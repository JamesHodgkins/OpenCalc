//! What a math region shows beside what was typed.
//!
//! As in SMath, a *definition* (`x := 5`) shows only the definition, and
//! a bare expression shows nothing: a result appears only on an evaluation region
//! (`x =`, or `2 + 3 =`). Errors are always shown, wherever they happen, so a failing
//! definition is not silent.

use opencalc_core::document::RegionId;
use opencalc_core::eval::{EvalError, Warning};
use opencalc_core::expr::{AssignKind, Expr};
use opencalc_core::value::Value;

pub enum Beside<'a> {
    Nothing,
    /// An evaluation region whose answer has not arrived yet.
    Pending,
    Value(&'a Value),
    Failed(&'a EvalError),
}

/// Whether the expression is an evaluation region (ends in `=`).
pub fn is_evaluation(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Assign {
            kind: AssignKind::Evaluate,
            ..
        }
    )
}

pub fn beside<'a>(expr: &Expr, result: Option<&'a Result<Value, EvalError>>) -> Beside<'a> {
    match result {
        Some(Err(e)) => Beside::Failed(e),
        // A function definition's "value" is only the function itself.
        Some(Ok(Value::Function { .. })) => Beside::Nothing,
        Some(Ok(v)) if is_evaluation(expr) => Beside::Value(v),
        None if is_evaluation(expr) => Beside::Pending,
        _ => Beside::Nothing,
    }
}

/// The name this region defines if another region defines it too (the last one in
/// layout order wins, so every such region is flagged).
pub fn redefined(warnings: &[Warning], id: RegionId) -> Option<&str> {
    warnings.iter().find_map(|w| match w {
        Warning::Redefinition { name, regions } => regions.contains(&id).then_some(name.as_str()),
        Warning::UndefinedEvaluation { .. } => None,
    })
}

/// The name this region evaluates with `=` although nothing defines it: `:=` was
/// probably meant.
pub fn undefined_evaluation(warnings: &[Warning], id: RegionId) -> Option<&str> {
    warnings.iter().find_map(|w| match w {
        Warning::UndefinedEvaluation { name, region } if *region == id => Some(name.as_str()),
        _ => None,
    })
}

/// The part of the expression to typeset before the `=` of an evaluation region.
/// `x =` and `2 + 3 =` show just `x` and `2 + 3`; anything else shows as typed.
pub fn typeset_part(expr: &Expr) -> &Expr {
    match expr {
        Expr::Assign {
            kind: AssignKind::Evaluate,
            target,
            value,
        } if target.is_empty() || matches!(&**value, Expr::Name(n) if n == target) => value,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::parser::parse;

    fn ok() -> Result<Value, EvalError> {
        Ok(Value::dimensionless(5.0))
    }

    fn shown(src: &str, result: Option<&Result<Value, EvalError>>) -> &'static str {
        match beside(&parse(src), result) {
            Beside::Nothing => "nothing",
            Beside::Pending => "pending",
            Beside::Value(_) => "value",
            Beside::Failed(_) => "error",
        }
    }

    #[test]
    fn only_evaluation_regions_show_a_value() {
        let r = ok();
        assert_eq!(shown("x =", Some(&r)), "value");
        assert_eq!(shown("2 + 3 =", Some(&r)), "value");
        assert_eq!(shown("x := 5", Some(&r)), "nothing");
        assert_eq!(
            shown("2 + 3", Some(&r)),
            "nothing",
            "a bare expression shows nothing"
        );
        assert_eq!(shown("f(x) := x ^ 2", Some(&r)), "nothing");
        assert_eq!(shown("v := [1, 2]", Some(&r)), "nothing");
    }

    #[test]
    fn an_evaluation_waiting_for_the_worker_shows_pending_but_nothing_else_does() {
        assert_eq!(shown("x =", None), "pending");
        assert_eq!(shown("x := 5", None), "nothing");
        assert_eq!(shown("2 + 3", None), "nothing");
    }

    #[test]
    fn errors_show_on_every_kind_of_region() {
        let e: Result<Value, EvalError> = Err(EvalError::NotFinite);
        for src in ["x =", "x := 1 / 0", "1 / 0", "f(x) := x"] {
            assert_eq!(shown(src, Some(&e)), "error", "{src}");
        }
    }

    #[test]
    fn every_region_defining_a_repeated_name_is_flagged_and_others_are_not() {
        use opencalc_core::eval::evaluate;
        let regions = vec![
            (RegionId(1), parse("x := 1")),
            (RegionId(2), parse("y := 2")),
            (RegionId(3), parse("x := 3")),
        ];
        let out = evaluate(&regions);
        assert_eq!(redefined(&out.warnings, RegionId(1)), Some("x"));
        assert_eq!(redefined(&out.warnings, RegionId(3)), Some("x"));
        assert_eq!(redefined(&out.warnings, RegionId(2)), None);
    }

    #[test]
    fn a_function_definition_never_shows_its_function_value() {
        let f: Result<Value, EvalError> = Ok(Value::Function {
            params: vec!["x".into()],
        });
        assert_eq!(shown("f(x) := x", Some(&f)), "nothing");
    }

    #[test]
    fn the_typeset_part_leaves_out_the_repeated_name() {
        assert_eq!(typeset_part(&parse("x =")).to_string(), "x");
        assert_eq!(typeset_part(&parse("2 + 3 =")).to_string(), "2 + 3");
        // As typed otherwise.
        assert_eq!(typeset_part(&parse("y = x + 1")).to_string(), "y = x + 1");
        assert_eq!(typeset_part(&parse("x := 5")).to_string(), "x := 5");
    }
}
