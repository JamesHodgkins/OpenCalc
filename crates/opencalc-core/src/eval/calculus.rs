//! Forms that repeat an evaluation with a bound variable: `sum_over`,
//! `product_over`, `integrate`, `derivative` and `limit`.
//!
//! They are evaluated here, not in `functions`, because their first argument is an
//! expression to evaluate many times, not a value. The second argument is the
//! bound variable's name; it is local to the form and shadows any worksheet name,
//! unit or outer variable of the same name.
//!
//! `integrate`, `derivative` and `limit` are numerical methods with stated
//! tolerances. When the method cannot reach its tolerance (a pole, a jump, a
//! kink, noise) the form is an error, never a number it does not trust. Every
//! repeated evaluation is charged to the region's [`Meter`](super::meter::Meter).

use super::node::{eval, origin, Evaluated, Located};
use super::{Cx, EvalError};
use crate::expr::Expr;
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use crate::value::{Matrix, Scalar, Value};

mod ode;
mod quadrature;
mod ridders;
mod roots;
mod solve;

/// Most terms a sum or product may have.
const MAX_BOUND: f64 = 1e9;

pub(super) fn evaluate(name: &str, args: &[Expr], cx: &Cx) -> Evaluated {
    match name {
        "sum_over" => fold(name, args, cx, false),
        "product_over" => fold(name, args, cx, true),
        "integrate" => integrate(args, cx),
        "derivative" => derivative(args, cx),
        "limit" => limit(args, cx),
        "root" => root(args, cx),
        "odesolve" => ode::odesolve(args, cx),
        "col" | "row" => select(name, args, cx),
        "find" | "minerr" | "minimize" | "maximize" => solve::evaluate(name, args, cx),
        _ => Err(Located::here(EvalError::UnknownFunction(name.to_owned()))),
    }
}

fn invalid(message: impl Into<String>) -> Located {
    Located::here(EvalError::Invalid(message.into()))
}

/// The body and the bound variable's name; checks the argument count, which
/// must be one of `counts`.
fn target<'a>(
    name: &str,
    args: &'a [Expr],
    counts: &[usize],
) -> Result<(&'a Expr, &'a str), Located> {
    if !counts.contains(&args.len()) {
        return Err(match counts {
            [one] => Located::here(EvalError::Arity(name.to_owned(), *one)),
            _ => invalid(format!(
                "{name} takes {} or {} arguments",
                counts[0], counts[1]
            )),
        });
    }
    match &args[1] {
        Expr::Name(v) => Ok((&args[0], v)),
        _ => Err(invalid(format!(
            "the second argument of {name} must be the name of a variable"
        ))),
    }
}

/// A context with the variable bound (initially to 0). Outer locals stay visible;
/// the new one is last, and `name` looks from the last, so it shadows them.
fn bound<'a>(cx: &Cx<'a>, var: &str) -> Cx<'a> {
    let mut locals = cx.locals.clone();
    locals.push((var.to_owned(), Value::dimensionless(0.0)));
    Cx {
        env: cx.env,
        fns: cx.fns,
        units: cx.units,
        locals,
        depth: cx.depth,
        meter: cx.meter,
    }
}

/// Evaluate the body with the variable set to `value`, charging one step.
fn at(inner: &mut Cx, body: &Expr, value: Value) -> Evaluated {
    inner.meter.charge().map_err(Located::here)?;
    if let Some(last) = inner.locals.last_mut() {
        last.1 = value;
    }
    eval(body, inner).map_err(|l| l.under(0))
}

/// The body at a real `x` of dimension `dim` (SI value), as a number.
fn scalar_at(inner: &mut Cx, body: &Expr, x: f64, dim: Dimension) -> Result<Scalar, Located> {
    let v = Value::Scalar(Scalar::new(numeric::real(x), dim));
    match at(inner, body, v)? {
        Value::Scalar(s) => Ok(s),
        _ => Err(invalid("the expression must give a single number").under(0)),
    }
}

/// Argument `index` as a real number with its dimension.
fn real_arg(args: &[Expr], index: usize, cx: &Cx) -> Result<(f64, Dimension), Located> {
    match eval(&args[index], cx).map_err(|l| l.under(index))? {
        Value::Scalar(s) if numeric::is_real(s.z) => Ok((s.z.re, s.dim)),
        _ => Err(invalid("a bound must be a real number").under(index)),
    }
}

/// Argument `index` as a whole number, for the bounds of a sum or product.
fn whole_arg(args: &[Expr], index: usize, cx: &Cx) -> Result<i64, Located> {
    let (x, dim) = real_arg(args, index, cx)?;
    if dim != Dimension::NONE || x.fract() != 0.0 || x.abs() > MAX_BOUND {
        return Err(invalid("the bounds of a sum or product must be whole numbers").under(index));
    }
    Ok(x as i64)
}

/// `sum_over(f, k, a, b)` and `product_over(f, k, a, b)`: k from a to b in steps of 1.
fn fold(name: &str, args: &[Expr], cx: &Cx, product: bool) -> Evaluated {
    let (body, var) = target(name, args, &[4])?;
    let (from, to) = (whole_arg(args, 2, cx)?, whole_arg(args, 3, cx)?);
    if to < from {
        // An empty sum would be a bare 0 with no dimension to add to anything.
        return Err(invalid(format!(
            "{name}: the range {from} to {to} is empty"
        )));
    }
    let mut inner = bound(cx, var);
    let mut total: Option<Value> = None;
    for k in from..=to {
        let term = at(&mut inner, body, Value::dimensionless(k as f64))?;
        total = Some(match total {
            None => term,
            Some(acc) => if product {
                acc.mul(&term)
            } else {
                acc.add(&term)
            }
            .map_err(Located::here)?,
        });
    }
    total.ok_or_else(|| invalid("nothing to add"))
}

/// Keeps the body's dimension the same at every sample.
struct Sampler<'c, 'a> {
    inner: Cx<'a>,
    body: &'c Expr,
    x_dim: Dimension,
    f_dim: Option<Dimension>,
}

impl<'c, 'a> Sampler<'c, 'a> {
    fn new(cx: &Cx<'a>, body: &'c Expr, var: &str, x_dim: Dimension) -> Sampler<'c, 'a> {
        Sampler {
            inner: bound(cx, var),
            body,
            x_dim,
            f_dim: None,
        }
    }

    fn sample(&mut self, x: f64) -> Result<Complex64, Located> {
        let s = scalar_at(&mut self.inner, self.body, x, self.x_dim)?;
        match self.f_dim {
            None => self.f_dim = Some(s.dim),
            Some(d) if d != s.dim => {
                return Err(Located::here(UnitError::Mismatch(d, s.dim)).under(0));
            }
            Some(_) => {}
        }
        Ok(s.z)
    }

    fn f_dim(&self) -> Dimension {
        self.f_dim.unwrap_or(Dimension::NONE)
    }
}

/// `col(M, j)` and `row(M, i)`: a column or a row of a matrix, counting from `ORIGIN`
/// as indexing does. The elements keep their own units.
fn select(name: &str, args: &[Expr], cx: &Cx) -> Evaluated {
    if args.len() != 2 {
        return Err(Located::here(EvalError::Arity(name.to_owned(), 2)));
    }
    let Value::Matrix(m) = eval(&args[0], cx).map_err(|l| l.under(0))? else {
        return Err(invalid(format!("{name} needs a matrix")).under(0));
    };
    let (n, dim) = real_arg(args, 1, cx)?;
    if dim != Dimension::NONE || n.fract() != 0.0 || n.abs() > 1e9 {
        return Err(invalid(format!("the {name} number must be a whole number")).under(1));
    }
    let at = n as i64 - origin(cx)?;
    let (count, along) = if name == "col" {
        (m.cols(), m.rows())
    } else {
        (m.rows(), m.cols())
    };
    let Ok(at) = usize::try_from(at) else {
        return Err(invalid(format!("{name} {n} is outside the matrix")).under(1));
    };
    if at >= count {
        return Err(invalid(format!("{name} {n} is outside the matrix")).under(1));
    }
    let items = (0..along)
        .map(|k| {
            if name == "col" {
                m.element(&[k, at])
            } else {
                m.element(&[at, k])
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Located::here)?;
    let picked = if name == "col" {
        Matrix::column(&items)
    } else {
        Matrix::from_scalars(1, items.len(), &items)
    };
    Ok(Value::Matrix(picked.map_err(Located::here)?))
}

/// `root(f, x, guess)` or `root(f, x, a, b)`: a real root of f.
fn root(args: &[Expr], cx: &Cx) -> Evaluated {
    let (body, var) = target("root", args, &[3, 4])?;
    let (a, da) = real_arg(args, 2, cx)?;
    let mut s = Sampler::new(cx, body, var, da);
    let mut real = |x: f64| -> Result<f64, Located> {
        let z = s.sample(x)?;
        if z.im != 0.0 {
            return Err(invalid("root needs a function with real values").under(0));
        }
        Ok(z.re)
    };
    let x = if args.len() == 4 {
        let (b, db) = real_arg(args, 3, cx)?;
        if da != db {
            return Err(Located::here(UnitError::Mismatch(da, db)).under(3));
        }
        roots::bracketed(&mut real, a, b)?
    } else {
        roots::from_guess(&mut real, a)?
    };
    Ok(Value::Scalar(Scalar::new(numeric::real(x), da)))
}

fn scalar_result(z: Complex64, dim: Option<Dimension>) -> Result<Value, Located> {
    Ok(Value::Scalar(Scalar::new(
        z,
        dim.unwrap_or(Dimension::NONE),
    )))
}

/// `integrate(f, x, a, b)`: the definite integral by adaptive Gauss-Kronrod.
fn integrate(args: &[Expr], cx: &Cx) -> Evaluated {
    let (body, var) = target("integrate", args, &[4])?;
    let ((a, da), (b, db)) = (real_arg(args, 2, cx)?, real_arg(args, 3, cx)?);
    if da != db {
        return Err(Located::here(UnitError::Mismatch(da, db)).under(3));
    }
    let mut s = Sampler::new(cx, body, var, da);
    let (value, _) = quadrature::integrate(&mut |x| s.sample(x), a, b)?;
    let dim = s
        .f_dim()
        .checked_times(da)
        .ok_or(UnitError::DimensionOverflow)
        .map_err(Located::here)?;
    scalar_result(value, Some(dim))
}

/// `derivative(f, x, at)`: the first derivative by Ridders' method.
fn derivative(args: &[Expr], cx: &Cx) -> Evaluated {
    let (body, var) = target("derivative", args, &[3])?;
    let (x0, dx) = real_arg(args, 2, cx)?;
    let mut s = Sampler::new(cx, body, var, dx);
    let d = ridders::derivative(&mut |x| s.sample(x), x0)?;
    let dim = s
        .f_dim()
        .checked_over(dx)
        .ok_or(UnitError::DimensionOverflow)
        .map_err(Located::here)?;
    scalar_result(d, Some(dim))
}

/// `limit(f, x, to)`: the two-sided limit by Richardson extrapolation.
fn limit(args: &[Expr], cx: &Cx) -> Evaluated {
    let (body, var) = target("limit", args, &[3])?;
    let (x0, dx) = real_arg(args, 2, cx)?;
    let mut s = Sampler::new(cx, body, var, dx);
    let value = ridders::limit(&mut |x| s.sample(x), x0)?;
    scalar_result(value, s.f_dim)
}
