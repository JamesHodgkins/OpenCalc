//! Running a program (docs/PROGRAMMING.md): local variables, branches, loops and
//! early return. Each statement and each pass of a loop is one step of the
//! region's budget, and checks for cancellation, so a runaway loop ends.

use super::node::{eval, origin, whole_number, Evaluated, Located};
use super::{Cx, EvalError};
use crate::expr::{assigned_names, Expr};
use crate::value::{Scalar, Value};

/// What a statement did besides computing.
enum Flow {
    Next,
    Break,
    Continue,
    Return(Value),
}

type Done = Result<Flow, Located>;

/// A local that the program assigns somewhere but has not yet is recorded under
/// this prefix, so reading it is an error rather than a quiet fall-back to a
/// worksheet variable of the same name (which the dependency analysis, treating
/// it as local, has not ordered).
const UNSET: &str = "\u{0}unset:";

/// Whether `n` is a local of a running program that has no value yet.
pub(super) fn unset(n: &str, cx: &Cx) -> bool {
    let locals = cx.locals.borrow();
    !locals.is_empty()
        && locals
            .iter()
            .any(|(p, _)| p.strip_prefix(UNSET).is_some_and(|rest| rest == n))
}

fn set(cx: &Cx, name: &str, value: Value) {
    let mut locals = cx.locals.borrow_mut();
    match locals.iter_mut().rev().find(|(p, _)| p == name) {
        Some(slot) => slot.1 = value,
        None => locals.push((name.to_owned(), value)),
    }
}

fn is_statement(e: &Expr) -> bool {
    matches!(
        e,
        Expr::LocalAssign { .. }
            | Expr::IndexAssign { .. }
            | Expr::IfBlock { .. }
            | Expr::For { .. }
            | Expr::While { .. }
            | Expr::Break
            | Expr::Continue
            | Expr::Return(_)
            | Expr::Try { .. }
    )
}

/// The value of a program: what `return` gives, else the value of its last line
/// when that is an expression.
pub(super) fn run(statements: &[Expr], cx: &Cx) -> Evaluated {
    let base = cx.locals.borrow().len();
    for name in assigned_names(statements) {
        let has_value = cx.locals.borrow().iter().any(|(p, _)| *p == name);
        if !has_value {
            let marker = format!("{UNSET}{name}");
            cx.locals
                .borrow_mut()
                .push((marker, Value::dimensionless(0.0)));
        }
    }
    let result = lines(statements, cx);
    cx.locals.borrow_mut().truncate(base);
    result
}

fn lines(statements: &[Expr], cx: &Cx) -> Evaluated {
    for (i, s) in statements.iter().enumerate() {
        cx.meter.charge().map_err(Located::here)?;
        if i + 1 == statements.len() && !is_statement(s) {
            return eval(s, cx).map_err(|l| l.under(i));
        }
        match exec(s, cx).map_err(|l| l.under(i))? {
            Flow::Next => {}
            Flow::Return(v) => return Ok(v),
            Flow::Break | Flow::Continue => {
                return Err(Located::here(EvalError::Invalid(
                    "'break' and 'continue' belong inside a loop".into(),
                ))
                .under(i))
            }
        }
    }
    Err(Located::here(EvalError::Invalid(
        "the program gives no value: end it with an expression or 'return'".into(),
    )))
}

/// The statements of a block.
fn block(b: &Expr, cx: &Cx) -> Done {
    let statements = match b {
        Expr::Program(statements) => statements.as_slice(),
        other => std::slice::from_ref(other),
    };
    for (i, s) in statements.iter().enumerate() {
        cx.meter.charge().map_err(Located::here)?;
        match exec(s, cx).map_err(|l| l.under(i))? {
            Flow::Next => {}
            other => return Ok(other),
        }
    }
    Ok(Flow::Next)
}

fn truth(cond: &Expr, cx: &Cx) -> Result<bool, Located> {
    let test = eval(cond, cx).map_err(|l| l.under(0))?;
    let flag = test
        .as_real()
        .filter(|q| q.dim.is_dimensionless())
        .ok_or_else(|| {
            Located::here(EvalError::Invalid(
                "a condition must be a real number without units".into(),
            ))
            .under(0)
        })?;
    Ok(flag.si != 0.0)
}

#[inline(never)]
fn exec(s: &Expr, cx: &Cx) -> Done {
    match s {
        Expr::LocalAssign { target, value } => {
            let v = eval(value, cx).map_err(|l| l.under(0))?;
            set(cx, target, v);
            Ok(Flow::Next)
        }
        Expr::IndexAssign {
            target,
            indices,
            value,
        } => {
            assign_element(target, indices, value, cx)?;
            Ok(Flow::Next)
        }
        Expr::IfBlock {
            cond,
            then,
            otherwise,
        } => {
            if truth(cond, cx)? {
                block(then, cx).map_err(|l| l.under(1))
            } else if let Some(o) = otherwise {
                block(o, cx).map_err(|l| l.under(2))
            } else {
                Ok(Flow::Next)
            }
        }
        Expr::For { var, over, body } => repeat_for(var, over, body, cx),
        Expr::While { cond, body } => loop {
            cx.meter.charge().map_err(Located::here)?;
            if !truth(cond, cx)? {
                return Ok(Flow::Next);
            }
            match block(body, cx).map_err(|l| l.under(1))? {
                Flow::Break => return Ok(Flow::Next),
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Next | Flow::Continue => {}
            }
        },
        Expr::Break => Ok(Flow::Break),
        Expr::Continue => Ok(Flow::Continue),
        Expr::Return(value) => Ok(Flow::Return(eval(value, cx).map_err(|l| l.under(0))?)),
        Expr::Try { body, handler } => match block(body, cx) {
            // Stopping the calculation is not a failure to be recovered from.
            Err(l) if !l.error.is_stop() => block(handler, cx).map_err(|l| l.under(1)),
            other => other.map_err(|l| l.under(0)),
        },
        other => eval(other, cx).map(|_| Flow::Next),
    }
}

#[inline(never)]
fn repeat_for(var: &str, over: &Expr, body: &Expr, cx: &Cx) -> Done {
    let values: Vec<Scalar> = match eval(over, cx).map_err(|l| l.under(0))? {
        Value::Scalar(s) => vec![s],
        Value::Series(s) => s.scalars().collect(),
        Value::Matrix(m) => m.scalars().collect(),
        Value::Function { .. } => {
            return Err(Located::here(EvalError::Invalid(
                "'for' goes through a range, a vector or a matrix".into(),
            ))
            .under(0))
        }
    };
    for item in values {
        cx.meter.charge().map_err(Located::here)?;
        set(cx, var, Value::Scalar(item));
        match block(body, cx).map_err(|l| l.under(1))? {
            Flow::Break => break,
            Flow::Return(v) => return Ok(Flow::Return(v)),
            Flow::Next | Flow::Continue => {}
        }
    }
    Ok(Flow::Next)
}

/// `target[i] ← value` or `target[i, j] ← value`. The target is a local of the
/// program (it need not exist yet); an element past the end grows it, with zeros of
/// the new element's dimension. Counts from `ORIGIN`, like indexing.
#[inline(never)]
fn assign_element(target: &str, indices: &[Expr], value: &Expr, cx: &Cx) -> Result<(), Located> {
    let first = origin(cx)?;
    let mut at = Vec::with_capacity(indices.len());
    for (k, expr) in indices.iter().enumerate() {
        let n = eval(expr, cx).map_err(|l| l.under(k))?;
        let zero_based = whole_number(&n)
            .and_then(|n| usize::try_from(n - first).ok())
            .ok_or_else(|| {
                Located::here(EvalError::Invalid(
                    "an index must be a whole number, counting from ORIGIN".into(),
                ))
                .under(k)
            })?;
        at.push(zero_based);
    }
    let last = indices.len();
    let Value::Scalar(new) = eval(value, cx).map_err(|l| l.under(last))? else {
        return Err(Located::here(EvalError::Invalid(
            "an element must be a single number".into(),
        ))
        .under(last));
    };
    let current = cx
        .locals
        .borrow()
        .iter()
        .rev()
        .find(|(p, _)| p == target)
        .map(|(_, v)| v.clone());
    let (mut rows, mut cols, mut cells) = match current {
        None => (0, 0, Vec::new()),
        Some(Value::Scalar(s)) => (1, 1, vec![s]),
        Some(Value::Series(s)) => (s.len(), 1, s.scalars().collect()),
        Some(Value::Matrix(m)) => (m.rows(), m.cols(), m.scalars().collect()),
        Some(Value::Function { .. }) => {
            return Err(Located::here(EvalError::Invalid(format!(
                "'{target}' is a function, not a vector"
            ))))
        }
    };
    let (r, c) = match at[..] {
        [i] if cols <= 1 => (i, 0),
        [i] if rows == 1 => (0, i),
        [_] => {
            return Err(Located::here(EvalError::Invalid(
                "a matrix needs a row and a column index, like M[i, j]".into(),
            )))
        }
        [r, c] => (r, c),
        _ => {
            return Err(Located::here(EvalError::Invalid(
                "an index has one or two numbers".into(),
            )))
        }
    };
    let (new_rows, new_cols) = (rows.max(r + 1), cols.max(c + 1));
    if (new_rows, new_cols) != (rows, cols) {
        let zero = Scalar::new(crate::numeric::real(0.0), new.dim);
        let mut grown = vec![zero; new_rows * new_cols];
        for i in 0..rows {
            for j in 0..cols {
                grown[i * new_cols + j] = cells[i * cols + j];
            }
        }
        (rows, cols, cells) = (new_rows, new_cols, grown);
    }
    cells[r * cols + c] = new;
    // Copying the elements costs in proportion to their number.
    for _ in 0..cells.len() / 64 {
        cx.meter.charge().map_err(Located::here)?;
    }
    let matrix = crate::value::Matrix::from_scalars(rows, cols, &cells).map_err(Located::here)?;
    set(cx, target, Value::Matrix(matrix));
    Ok(())
}
