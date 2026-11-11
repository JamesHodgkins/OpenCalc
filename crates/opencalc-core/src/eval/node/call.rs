//! Evaluating `given` blocks and calls of user-defined functions.

use super::{child, eval, Evaluated, Located};
use crate::eval::{Cx, EvalError, UserFn, MAX_CALL_DEPTH};
use crate::expr::Expr;
use crate::value::Value;

/// A `given` block is the solver call it lowers to. The call's arguments are the
/// closing line's, then the constraints; the block's children are the other way
/// round (the order of the text), so the step into the call is renumbered.
#[inline(never)]
pub(super) fn given(block: &crate::expr::GivenBlock, cx: &Cx) -> Evaluated {
    // The guesses written in the block are local to it: they are bound for the
    // call, which reads the unknowns' starting values by name.
    let base = cx.locals.borrow().len();
    let result = given_call(block, cx);
    cx.locals.borrow_mut().truncate(base);
    result
}

fn given_call(block: &crate::expr::GivenBlock, cx: &Cx) -> Evaluated {
    for (line, name, value) in block.guesses() {
        // (The guess line is an assignment, whose one child is the value.)
        let guess = eval(value, cx).map_err(|l| l.under(0).under(line))?;
        cx.locals.borrow_mut().push((name.to_owned(), guess));
    }
    let lines = block.constraint_lines();
    eval(&block.lowered(), cx).map_err(|mut l| {
        if let Some(step) = l.rev_path.last_mut() {
            let k = usize::from(*step);
            let child = match k.checked_sub(block.args.len()) {
                None => block.constraints.len() + k,
                Some(c) => lines.get(c).copied().unwrap_or(c),
            };
            *step = u16::try_from(child).unwrap_or(u16::MAX);
        }
        l
    })
}

/// Call a user-defined function: evaluate the arguments where the call is, bind
/// them to the parameters, and evaluate the body with only those bound.
#[inline(never)]
pub(super) fn call_user(name: &str, f: &UserFn, args: &[Expr], cx: &Cx) -> Evaluated {
    if f.circular {
        return Err(Located::here(EvalError::Cycle));
    }
    // Each call is a step of the region's budget, so a recursion that does not
    // end (or only ends after too much work) stops on the budget.
    cx.meter.charge().map_err(Located::here)?;
    let fewest = f.params.len() - f.defaults.len();
    if args.len() < fewest || args.len() > f.params.len() {
        return Err(Located::here(if f.defaults.is_empty() {
            EvalError::Arity(name.to_owned(), f.params.len())
        } else {
            EvalError::Invalid(format!(
                "function '{name}' takes {fewest} to {} arguments",
                f.params.len()
            ))
        }));
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
    if args.len() < f.params.len() {
        add_defaults(name, f, args.len(), &mut locals, cx)?;
    }
    let inner = cx.scoped(locals, cx.depth + 1);
    // An error in the body is reported at the call, with the function's name.
    eval(f.body, &inner).map_err(|l| {
        Located::here(EvalError::InFunction {
            name: name.to_owned(),
            error: Box::new(l.error),
        })
    })
}

/// Bind the parameters after the first `given` to their defaults. They are
/// worked out in the worksheet (not where the function is called, and not with the
/// other parameters). Kept out of `call_user` so its stack frame stays small: a
/// test sizes `MAX_CALL_DEPTH` from it.
#[inline(never)]
fn add_defaults(
    name: &str,
    f: &UserFn,
    given: usize,
    locals: &mut Vec<(String, Value)>,
    cx: &Cx,
) -> Result<(), Located> {
    let outer = cx.scoped(Vec::new(), cx.depth + 1);
    let first = f.params.len() - f.defaults.len();
    for (param, default) in f.params[given..].iter().zip(&f.defaults[given - first..]) {
        let value = eval(default, &outer).map_err(|l| {
            Located::here(EvalError::InFunction {
                name: name.to_owned(),
                error: Box::new(EvalError::Invalid(format!(
                    "the default of '{param}': {}",
                    l.error
                ))),
            })
        })?;
        locals.push((param.clone(), value));
    }
    Ok(())
}
