//! `find`, `minerr`, `minimize` and `maximize` (design: `docs/SOLVE_BLOCKS.md`).
//!
//! The unknowns are named in the call; their starting values (the guesses) are the
//! worksheet values of the same names, read outside the call. Inside the
//! constraints and the objective the names are bound to the current trial point,
//! as for `integrate`. Everything is calculated on SI values; the constraints'
//! dimensions are checked, never scaled. Each unknown keeps the dimension of its own
//! guess, so a call may solve for a force and a moment together.
//!
//! * `find` and `minerr` run Levenberg-Marquardt on the equations and on the
//!   violations of the inequalities. `find` then checks every constraint and
//!   gives an error rather than a point that fails one; `minerr` returns the best
//!   point reached, as Mathcad documents for `minerr`.
//! * `minimize` and `maximize` run BFGS; constraints are handled by an augmented
//!   Lagrangian, then checked the same way as for `find`.

use super::super::node::{eval, Evaluated, Located};
use super::super::{Cx, EvalError};
use super::invalid;
use crate::expr::{solver_parts, BinaryOp, Expr};
use crate::numeric;
use crate::units::{Dimension, UnitError};
use crate::value::{Matrix, Scalar, Value};

mod bfgs;
mod lm;

/// Most unknowns and constraints in one call.
const MAX_UNKNOWNS: usize = 16;
const MAX_CONSTRAINTS: usize = 64;
/// How closely a constraint must hold, relative to the size of its two sides, for
/// `find` and `minerr`; and for the optimising forms, which reach constraints
/// through a penalty method and so a little less tightly.
const FIND_TOL: f64 = 1e-9;
const OPTIMISE_TOL: f64 = 1e-7;
/// Outer rounds of the augmented Lagrangian.
const MAX_OUTER: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Kind {
    /// `a == b`
    Eq,
    /// `a <= b` (and `a < b`, `a > b` and `a >= b` with the sides swapped)
    Le,
}

/// One constraint evaluated at a point.
#[derive(Clone, Debug)]
pub(super) struct Term {
    /// Left side minus right side, in SI.
    pub value: f64,
    /// The larger of the two sides' sizes, which tolerances are relative to.
    pub scale: f64,
    pub kind: Kind,
}

impl Term {
    /// How far the constraint is from holding: 0 when it holds.
    pub fn violation(&self) -> f64 {
        match self.kind {
            Kind::Eq => self.value.abs(),
            Kind::Le => self.value.max(0.0),
        }
    }
}

/// The smallest tolerance a constraint is given, from its size at the start.
pub(super) fn floors(at_start: &[Term]) -> Vec<f64> {
    at_start
        .iter()
        .map(|t| 1e-14 * if t.scale > 0.0 { t.scale } else { 1.0 })
        .collect()
}

pub(super) fn satisfied(terms: &[Term], floors: &[f64], rel: f64) -> bool {
    terms
        .iter()
        .zip(floors)
        .all(|(t, floor)| t.violation() <= rel * t.scale + floor)
}

struct Constraint<'e> {
    kind: Kind,
    lhs: &'e Expr,
    rhs: &'e Expr,
    /// Index of the constraint among the call's arguments (for error paths).
    arg: usize,
    /// The sides were swapped to turn `>` into `<=`.
    swapped: bool,
}

fn constraint(arg: usize, e: &Expr) -> Result<Constraint<'_>, Located> {
    let bad = || invalid("a constraint must be a comparison: ==, <, <=, > or >=").under(arg);
    let Expr::Binary { op, lhs, rhs } = e else {
        return Err(bad());
    };
    let (kind, a, b, swapped) = match op {
        BinaryOp::Eq => (Kind::Eq, lhs, rhs, false),
        BinaryOp::Le | BinaryOp::Lt => (Kind::Le, lhs, rhs, false),
        BinaryOp::Ge | BinaryOp::Gt => (Kind::Le, rhs, lhs, true),
        _ => return Err(bad()),
    };
    Ok(Constraint {
        kind,
        lhs: a,
        rhs: b,
        arg,
        swapped,
    })
}

/// The problem: the bound unknowns, the objective and the constraints.
struct Problem<'e, 'a> {
    inner: Cx<'a>,
    base: usize,
    /// The dimension of each unknown, from its guess.
    dims: Vec<Dimension>,
    objective: Option<&'e Expr>,
    constraints: Vec<Constraint<'e>>,
}

impl Problem<'_, '_> {
    fn set(&mut self, x: &[f64]) {
        for (i, v) in x.iter().enumerate() {
            self.inner.locals.get_mut()[self.base + i].1 =
                Value::Scalar(Scalar::new(numeric::real(*v), self.dims[i]));
        }
    }

    /// A real value of `expr` at the current point, with its dimension.
    fn real(&self, expr: &Expr, arg: usize, side: usize) -> Result<(f64, Dimension), Located> {
        self.inner.meter.charge().map_err(Located::here)?;
        match eval(expr, &self.inner).map_err(|l| l.under(side).under(arg))? {
            Value::Scalar(s) if numeric::is_real(s.z) => Ok((s.z.re, s.dim)),
            _ => Err(invalid("this must be a real number").under(side).under(arg)),
        }
    }

    fn terms(&mut self, x: &[f64]) -> Result<Vec<Term>, Located> {
        self.set(x);
        let mut out = Vec::with_capacity(self.constraints.len());
        for c in &self.constraints {
            let (l_side, r_side) = if c.swapped { (1, 0) } else { (0, 1) };
            let (a, da) = self.real(c.lhs, c.arg, l_side)?;
            let (b, db) = self.real(c.rhs, c.arg, r_side)?;
            if da != db {
                return Err(Located::here(UnitError::Mismatch(da, db)).under(c.arg));
            }
            out.push(Term {
                value: a - b,
                scale: a.abs().max(b.abs()),
                kind: c.kind,
            });
        }
        Ok(out)
    }

    fn objective(&mut self, x: &[f64]) -> Result<f64, Located> {
        self.set(x);
        let Some(f) = self.objective else {
            return Err(invalid("there is no objective"));
        };
        self.inner.meter.charge().map_err(Located::here)?;
        // The objective is argument 0 of the call.
        match eval(f, &self.inner).map_err(|l| l.under(0))? {
            Value::Scalar(s) if numeric::is_real(s.z) => Ok(s.z.re),
            _ => Err(invalid("the objective must be a real number").under(0)),
        }
    }
}

pub(super) fn evaluate(name: &str, args: &[Expr], cx: &Cx) -> Evaluated {
    let parts = solver_parts(name, args).map_err(invalid)?;
    if parts.unknowns.len() > MAX_UNKNOWNS || parts.constraints.len() > MAX_CONSTRAINTS {
        return Err(invalid(format!(
            "{name} takes at most {MAX_UNKNOWNS} unknowns and {MAX_CONSTRAINTS} constraints"
        )));
    }
    let names: Vec<&str> = parts.unknowns.iter().map(|(_, n)| *n).collect();
    if let Some(dup) = names
        .iter()
        .enumerate()
        .find_map(|(i, n)| names[..i].contains(n).then_some(n))
    {
        return Err(invalid(format!("'{dup}' is listed twice")));
    }
    let (x0, dims) = guesses(&parts.unknowns, cx)?;
    let constraints = parts
        .constraints
        .iter()
        .map(|(arg, e)| constraint(*arg, e))
        .collect::<Result<Vec<_>, _>>()?;
    let mut locals = cx.locals.borrow().clone();
    let base = locals.len();
    locals.extend(
        names
            .iter()
            .map(|n| ((*n).to_owned(), Value::dimensionless(0.0))),
    );
    let mut problem = Problem {
        inner: cx.scoped(locals, cx.depth),
        base,
        dims: dims.clone(),
        objective: parts.objective,
        constraints,
    };
    let x = match name {
        "find" => find(&mut problem, &x0, true)?,
        "minerr" => find(&mut problem, &x0, false)?,
        "minimize" => optimise(&mut problem, &x0, false)?,
        _ => optimise(&mut problem, &x0, true)?,
    };
    answer(&x, &dims)
}

/// The starting values: the worksheet values of the unknowns' names.
fn guesses(unknowns: &[(usize, &str)], cx: &Cx) -> Result<(Vec<f64>, Vec<Dimension>), Located> {
    let mut x0 = Vec::with_capacity(unknowns.len());
    let mut dims = Vec::with_capacity(unknowns.len());
    for (arg, n) in unknowns {
        let v = eval(&Expr::Name((*n).to_owned()), cx).map_err(|l| match l.error {
            EvalError::Undefined(_) => {
                invalid(format!("give '{n}' a starting value, such as {n} := 1")).under(*arg)
            }
            _ => l.under(*arg),
        })?;
        let Value::Scalar(s) = v else {
            return Err(
                invalid(format!("the starting value of '{n}' must be a number")).under(*arg),
            );
        };
        if !numeric::is_real(s.z) {
            return Err(invalid("complex unknowns are not supported yet").under(*arg));
        }
        x0.push(s.z.re);
        dims.push(s.dim);
    }
    Ok((x0, dims))
}

fn answer(x: &[f64], dims: &[Dimension]) -> Evaluated {
    if let ([one], [dim]) = (x, dims) {
        return Ok(Value::Scalar(Scalar::new(numeric::real(*one), *dim)));
    }
    let items: Vec<Scalar> = x
        .iter()
        .zip(dims)
        .map(|(v, dim)| Scalar::new(numeric::real(*v), *dim))
        .collect();
    Ok(Value::Matrix(
        Matrix::column(&items).map_err(Located::here)?,
    ))
}

fn largest(terms: &[Term]) -> f64 {
    terms.iter().map(Term::violation).fold(0.0, f64::max)
}

/// `find` (`strict`) or `minerr`.
fn find(p: &mut Problem, x0: &[f64], strict: bool) -> Result<Vec<f64>, Located> {
    let out = lm::solve(&mut |x| p.terms(x), x0, FIND_TOL)?;
    if out.x.iter().any(|v| !v.is_finite()) {
        return Err(Located::here(EvalError::NotFinite));
    }
    if strict && !out.satisfied {
        return Err(invalid(format!(
            "no solution found: the nearest point leaves a residual of {:.1e} (in SI units); \
             try other starting values, or minerr for the best point",
            largest(&out.terms)
        )));
    }
    Ok(out.x)
}

/// `minimize`, or with `maximize` the same on the negated objective.
fn optimise(p: &mut Problem, x0: &[f64], maximize: bool) -> Result<Vec<f64>, Located> {
    let sign = if maximize { -1.0 } else { 1.0 };
    if p.constraints.is_empty() {
        let r = bfgs::minimise(&mut |x| Ok(sign * p.objective(x)?), x0)?;
        if !r.converged {
            return Err(invalid(
                "the search for a minimum did not converge; try another starting point",
            ));
        }
        return Ok(r.x);
    }
    let start = p.terms(x0)?;
    let floors = floors(&start);
    let mut lambda = vec![0.0; start.len()];
    let (mut mu, mut previous) = (10.0, f64::INFINITY);
    let mut x = x0.to_vec();
    for _ in 0..MAX_OUTER {
        let multipliers = lambda.clone();
        let m = mu;
        let r = bfgs::minimise(
            &mut |x| {
                let f = sign * p.objective(x)?;
                let t = p.terms(x)?;
                Ok(f + bfgs::penalty(&t, &multipliers, m))
            },
            &x,
        )?;
        x = r.x;
        let t = p.terms(&x)?;
        if satisfied(&t, &floors, OPTIMISE_TOL) {
            return Ok(x);
        }
        for (l, term) in lambda.iter_mut().zip(&t) {
            *l = match term.kind {
                Kind::Eq => *l + mu * term.value,
                Kind::Le => (*l + mu * term.value).max(0.0),
            };
        }
        let worst = largest(&t);
        if worst > 0.25 * previous {
            mu = (mu * 10.0).min(1e10);
        }
        previous = worst;
    }
    Err(invalid(
        "no point satisfying the constraints was found; try another starting point",
    ))
}
