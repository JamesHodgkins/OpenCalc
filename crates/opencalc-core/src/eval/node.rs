//! Evaluating one expression: the recursion over the tree, with the location of
//! any error. Kept apart from the dependency graph in `eval`.

use super::{Cx, EvalError, UserFn, MAX_CALL_DEPTH};
use crate::expr::{BinaryOp, Expr, UnaryOp, ORIGIN};
use crate::functions;
use crate::value::{Comparison, Matrix, Series, Value, ValueError};

/// An evaluation error and where in the expression it happened. The path is
/// built from the inside out as the error propagates, so it is stored reversed.
pub(super) struct Located {
    pub(super) error: EvalError,
    pub(super) rev_path: Vec<u16>,
}

impl Located {
    pub(super) fn here(error: impl Into<EvalError>) -> Located {
        Located {
            error: error.into(),
            rev_path: Vec::new(),
        }
    }

    /// The error came from child number `index` of the current node.
    fn under(mut self, index: usize) -> Located {
        // Expressions have at most `MAX_SYMBOLS` symbols, far below `u16::MAX`.
        self.rev_path.push(u16::try_from(index).unwrap_or(u16::MAX));
        self
    }
}

impl<E: Into<EvalError>> From<E> for Located {
    fn from(error: E) -> Located {
        Located::here(error)
    }
}

pub(super) type Evaluated = Result<Value, Located>;

pub(super) fn eval_region(expr: &Expr, cx: &Cx) -> Evaluated {
    match expr {
        Expr::FunctionDef { name, params, .. } => {
            if name == "if" || functions::lookup(name).is_some() {
                return Err(Located::here(EvalError::Invalid(format!(
                    "'{name}' is a built-in function and cannot be redefined"
                ))));
            }
            Ok(Value::Function {
                params: params.clone(),
            })
        }
        Expr::Assign { value, .. } => eval(value, cx).map_err(|l| l.under(0)),
        other => eval(other, cx),
    }
}

/// Evaluate one expression. The single choke point that rejects non-finite
/// results, so they cannot reach a displayed value.
fn eval(expr: &Expr, cx: &Cx) -> Evaluated {
    let v = eval_node(expr, cx)?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(Located::here(EvalError::NotFinite))
    }
}

/// Evaluate child `index` of the current node. Inlined even in debug builds: an
/// extra stack frame per nesting level eats the stack margin (see the parser tests).
#[inline(always)]
fn child(expr: &Expr, index: usize, cx: &Cx) -> Evaluated {
    eval(expr, cx).map_err(|l| l.under(index))
}

/// A thin dispatcher: every arm with real work is its own function, so the frame
/// this recursion pays for per nesting level stays small. (In a debug build a
/// frame holds the locals of all arms, and the parser tests bound the stack use.)
fn eval_node(expr: &Expr, cx: &Cx) -> Evaluated {
    match expr {
        Expr::Number(v) => Ok(Value::dimensionless(*v)),
        Expr::Quantity { value, unit } => Ok(cx.units.quantity(*value, unit)?.into()),
        Expr::Name(n) => name(n, cx),
        Expr::Unary {
            op: UnaryOp::Neg,
            arg,
        } => negate(arg, cx),
        Expr::Binary { op, lhs, rhs } => operate(*op, lhs, rhs, cx),
        Expr::Fraction { num, den } => operate(BinaryOp::Div, num, den, cx),
        Expr::Root { radicand } => root(radicand, cx),
        Expr::Call { name, args } => apply(name, args, cx),
        Expr::Matrix { rows, cols, cells } => literal(*rows, *cols, cells, cx),
        Expr::Index { base, indices } => index(base, indices, cx),
        Expr::Range { start, step, end } => range(start, step.as_deref(), end, cx),
        Expr::Conditional {
            cond,
            then,
            otherwise,
        } => conditional(cond, then, otherwise, cx),
        Expr::Assign { value, .. } => child(value, 0, cx),
        Expr::FunctionDef { .. } => Err(Located::here(EvalError::Invalid(
            "a function can only be defined at the top of a region".into(),
        ))),
        Expr::Error(e) => Err(Located::here(EvalError::Invalid(e.message.clone()))),
        // Display-tree nodes (`parse_lenient`); the evaluator is only given `parse`.
        Expr::Hole => Err(Located::here(EvalError::Invalid(
            "the expression is incomplete".into(),
        ))),
        Expr::Garbage { text, .. } => Err(Located::here(EvalError::Invalid(format!(
            "unexpected '{text}'"
        )))),
    }
}

/// `[a, b; c, d]`: every cell must be a scalar, and all share one dimension.
#[inline(never)]
fn literal(rows: usize, cols: usize, cells: &[Expr], cx: &Cx) -> Evaluated {
    let mut items = Vec::with_capacity(cells.len());
    for (i, cell) in cells.iter().enumerate() {
        match child(cell, i, cx)? {
            Value::Scalar(s) => items.push(s),
            Value::Matrix(_) | Value::Series(_) | Value::Function { .. } => {
                return Err(Located::here(ValueError::Shape(
                    "a matrix cell must be a single number".into(),
                ))
                .under(i))
            }
        }
    }
    Ok(Value::Matrix(Matrix::from_scalars(rows, cols, &items)?))
}

/// Call a user-defined function: evaluate the arguments where the call is, bind
/// them to the parameters, and evaluate the body with only those bound.
#[inline(never)]
fn call_user(name: &str, f: &UserFn, args: &[Expr], cx: &Cx) -> Evaluated {
    if f.circular {
        return Err(Located::here(EvalError::Cycle));
    }
    if args.len() != f.params.len() {
        return Err(Located::here(EvalError::Arity(
            name.to_owned(),
            f.params.len(),
        )));
    }
    if cx.depth >= MAX_CALL_DEPTH {
        return Err(Located::here(EvalError::Invalid(
            "functions call each other too deeply".into(),
        )));
    }
    let mut locals = Vec::with_capacity(args.len());
    for (i, (param, arg)) in f.params.iter().zip(args).enumerate() {
        locals.push((param.clone(), child(arg, i, cx)?));
    }
    let inner = Cx {
        env: cx.env,
        fns: cx.fns,
        units: cx.units,
        locals,
        depth: cx.depth + 1,
    };
    // An error in the body is reported at the call, with the function's name.
    eval(f.body, &inner).map_err(|l| {
        Located::here(EvalError::InFunction {
            name: name.to_owned(),
            error: Box::new(l.error),
        })
    })
}

/// `a..b` or `a, b..c`: a list of values.
#[inline(never)]
fn range(start: &Expr, step: Option<&Expr>, end: &Expr, cx: &Cx) -> Evaluated {
    let scalar = |value: Value, index: usize| match value {
        Value::Scalar(s) => Ok(s),
        _ => Err(Located::here(ValueError::Shape(
            "the parts of a range must be single numbers".into(),
        ))
        .under(index)),
    };
    let a = scalar(child(start, 0, cx)?, 0)?;
    let next = match step {
        Some(e) => Some(scalar(child(e, 1, cx)?, 1)?),
        None => None,
    };
    let end_at = if step.is_some() { 2 } else { 1 };
    let b = scalar(child(end, end_at, cx)?, end_at)?;
    Ok(Value::Series(Series::range(a, next, b)?))
}

/// Where indexing starts: the worksheet's `ORIGIN` variable, 0 if it has none.
fn origin(cx: &Cx) -> Result<i64, Located> {
    match cx.env.get(ORIGIN) {
        None => Ok(0),
        Some(Err(e)) => Err(Located::here(e.clone())),
        Some(Ok(v)) => whole_number(v).ok_or_else(|| {
            Located::here(EvalError::Invalid("ORIGIN must be a whole number".into()))
        }),
    }
}

/// A dimensionless real whole number, if the value is one.
fn whole_number(v: &Value) -> Option<i64> {
    let q = v.as_real()?;
    let small = q.si.abs() <= 1e9;
    (q.dim.is_dimensionless() && q.si.fract() == 0.0 && small).then_some(q.si as i64)
}

#[inline(never)]
fn index(base: &Expr, indices: &[Expr], cx: &Cx) -> Evaluated {
    let target = child(base, 0, cx)?;
    let first = origin(cx).map_err(|l| l.under(0))?;
    let mut at = Vec::with_capacity(indices.len());
    for (k, expr) in indices.iter().enumerate() {
        let value = child(expr, k + 1, cx)?;
        let n = whole_number(&value).ok_or_else(|| {
            Located::here(EvalError::Invalid("an index must be a whole number".into())).under(k + 1)
        })?;
        let zero_based = usize::try_from(n - first).map_err(|_| {
            Located::here(ValueError::Shape("index out of range".into())).under(k + 1)
        })?;
        at.push(zero_based);
    }
    Ok(target.element(&at)?)
}

/// Only the chosen branch is evaluated, so `if(x == 0, 0, 1 / x)` is safe.
#[inline(never)]
fn conditional(cond: &Expr, then: &Expr, otherwise: &Expr, cx: &Cx) -> Evaluated {
    let test = child(cond, 0, cx)?;
    let flag = test
        .as_real()
        .filter(|q| q.dim.is_dimensionless())
        .ok_or_else(|| {
            Located::here(EvalError::Invalid(
                "the condition of 'if' must be a real number without units".into(),
            ))
            .under(0)
        })?;
    if flag.si != 0.0 {
        child(then, 1, cx)
    } else {
        child(otherwise, 2, cx)
    }
}

/// A defined variable shadows a unit of the same name, and a unit shadows the
/// imaginary unit `i`.
#[inline(never)]
fn name(n: &str, cx: &Cx) -> Evaluated {
    if let Some((_, value)) = cx.locals.iter().find(|(p, _)| p == n) {
        return Ok(value.clone());
    }
    match cx.env.get(n) {
        Some(r) => r.clone().map_err(Located::here),
        None => cx
            .units
            .quantity(1.0, n)
            .map(Value::from)
            .or_else(|_| constant(n))
            .map_err(|_| Located::here(EvalError::Undefined(n.to_owned()))),
    }
}

#[inline(never)]
fn negate(arg: &Expr, cx: &Cx) -> Evaluated {
    Ok(child(arg, 0, cx)?.negated())
}

#[inline(never)]
fn operate(op: BinaryOp, lhs: &Expr, rhs: &Expr, cx: &Cx) -> Evaluated {
    Ok(binary(op, &child(lhs, 0, cx)?, &child(rhs, 1, cx)?)?)
}

#[inline(never)]
fn root(radicand: &Expr, cx: &Cx) -> Evaluated {
    Ok(functions::call("sqrt", &[child(radicand, 0, cx)?])?)
}

#[inline(never)]
fn apply(name: &str, args: &[Expr], cx: &Cx) -> Evaluated {
    if name == "if" {
        // The parser makes a Conditional from exactly three arguments.
        return Err(Located::here(EvalError::Arity("if".into(), 3)));
    }
    if functions::lookup(name).is_none() {
        if let Some(f) = cx.fns.get(name) {
            return call_user(name, f, args, cx);
        }
    }
    let values = args
        .iter()
        .enumerate()
        .map(|(i, a)| child(a, i, cx))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(functions::call(name, &values)?)
}

/// Built-in named constants, used when no variable or unit has the name.
fn constant(name: &str) -> Result<Value, ()> {
    crate::constants::constant(name).ok_or(())
}

fn binary(op: BinaryOp, a: &Value, b: &Value) -> Result<Value, ValueError> {
    Ok(match op {
        BinaryOp::Add => a.add(b)?,
        BinaryOp::Sub => a.sub(b)?,
        BinaryOp::Mul => a.mul(b)?,
        BinaryOp::Div => a.div(b)?,
        BinaryOp::Pow => a.pow(b)?,
        BinaryOp::Eq => a.compare(b, Comparison::Eq)?,
        BinaryOp::Ne => a.compare(b, Comparison::Ne)?,
        BinaryOp::Lt => a.compare(b, Comparison::Lt)?,
        BinaryOp::Gt => a.compare(b, Comparison::Gt)?,
        BinaryOp::Le => a.compare(b, Comparison::Le)?,
        BinaryOp::Ge => a.compare(b, Comparison::Ge)?,
    })
}
