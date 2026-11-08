//! Help entries for the calculus and iteration forms. They are listed with the
//! other functions (Functions window, autocomplete, reserved names) but are
//! evaluated by `eval::calculus`, because their first argument is an expression to
//! evaluate repeatedly, not a value.

use super::Arity::{AtLeast, Exactly};
use super::FunctionDef;
use crate::eval::EvalError;

fn evaluated_elsewhere(_: &[crate::value::Value]) -> super::Result<crate::value::Value> {
    Err(EvalError::Internal(
        "a calculus form reached the function table".into(),
    ))
}

pub(super) const CALCULUS: &[FunctionDef] = &[
    function!(
        "sum_over",
        "sum_over(f, k, a, b)",
        "Sum of f for k = a, a + 1, ..., b (whole numbers)",
        Exactly(4),
        evaluated_elsewhere
    ),
    function!(
        "product_over",
        "product_over(f, k, a, b)",
        "Product of f for k = a, a + 1, ..., b (whole numbers)",
        Exactly(4),
        evaluated_elsewhere
    ),
    function!(
        "integrate",
        "integrate(f, x, a, b)",
        "Definite integral of f over x from a to b (numerical, relative accuracy 1e-10)",
        Exactly(4),
        evaluated_elsewhere
    ),
    function!(
        "derivative",
        "derivative(f, x, at)",
        "First derivative of f with respect to x at a point (numerical)",
        Exactly(3),
        evaluated_elsewhere
    ),
    function!(
        "limit",
        "limit(f, x, to)",
        "Two-sided limit of f as x approaches a point, never evaluating f there (numerical)",
        Exactly(3),
        evaluated_elsewhere
    ),
    function!(
        "root",
        "root(f, x, guess) or root(f, x, a, b)",
        "A real root of f: from a guess, stepping outward to a sign change; or inside a bracket [a, b] where f changes sign",
        AtLeast(3),
        evaluated_elsewhere
    ),
];
