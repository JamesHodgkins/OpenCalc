//! Defining a unit from a quantity: `hand` is `4 in`, `ksf` is `1000 lbf / ft^2`.
//!
//! The user writes what the unit equals, as an ordinary expression with units; the
//! dimension and the scale to SI follow from evaluating it. So nobody has to work
//! out a conversion factor by hand, and a unit can be defined from others already
//! defined.

use crate::eval::{evaluate_with, RegionId};
use crate::expr::Expr;
use crate::parser::parse;
use crate::registry::{RegistryError, UnitRegistry};
use crate::units::UnitDef;
use crate::value::Value;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DefineError {
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("type what the unit equals, such as 4 in")]
    Empty,
    #[error("{0}")]
    Parse(String),
    #[error("{0}")]
    Eval(String),
    #[error("the definition must be a single real quantity, such as 4 in")]
    NotAQuantity,
    #[error("the definition must be greater than zero")]
    NotPositive,
}

/// The unit `symbol` that equals `definition`. `existing` are the worksheet's
/// custom units; the one at `replacing`, if any, is being redefined, so it is
/// neither a name clash nor available to the definition.
pub fn define_unit(
    symbol: &str,
    definition: &str,
    existing: &[UnitDef],
    replacing: Option<usize>,
) -> Result<UnitDef, DefineError> {
    let symbol = symbol.trim();
    let others: Vec<UnitDef> = existing
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != replacing)
        .map(|(_, d)| d.clone())
        .collect();
    if definition.trim().is_empty() {
        return Err(DefineError::Empty);
    }
    let expr = parse(definition);
    match &expr {
        Expr::Error(e) => return Err(DefineError::Parse(e.message.clone())),
        Expr::Assign { .. } | Expr::FunctionDef { .. } => return Err(DefineError::NotAQuantity),
        _ => {}
    }
    // The unit being defined is not available to its own definition.
    let registry = UnitRegistry::new(others.clone())?;
    let id = RegionId(1);
    let out = evaluate_with(&[(id, expr)], &registry);
    let value = match out.values.get(&id) {
        Some(Ok(v)) => v,
        Some(Err(e)) => return Err(DefineError::Eval(e.to_string())),
        None => return Err(DefineError::NotAQuantity),
    };
    let Value::Scalar(scalar) = value else {
        return Err(DefineError::NotAQuantity);
    };
    let q = scalar.as_real().ok_or(DefineError::NotAQuantity)?;
    if !q.si.is_finite() || q.si <= 0.0 {
        return Err(DefineError::NotPositive);
    }
    let def = UnitDef {
        symbol: symbol.to_owned(),
        dimension: q.dim,
        to_si_scale: q.si,
        definition: definition.trim().to_owned(),
    };
    // Checks the name: valid, not built in, not already used.
    let mut all = others;
    all.push(def.clone());
    UnitRegistry::new(all)?;
    Ok(def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Dimension;

    fn ok(symbol: &str, def: &str, existing: &[UnitDef]) -> UnitDef {
        define_unit(symbol, def, existing, None).unwrap()
    }

    fn err(symbol: &str, def: &str, existing: &[UnitDef]) -> DefineError {
        define_unit(symbol, def, existing, None).unwrap_err()
    }

    #[test]
    fn the_scale_and_dimension_follow_from_the_quantity() {
        let hand = ok("hand", "4 in", &[]);
        assert_eq!(hand.dimension, Dimension::LENGTH);
        assert!((hand.to_si_scale - 0.1016).abs() < 1e-12);
        let ksf = ok("ksf", "1000 lbf / ft^2", &[]);
        assert_eq!(ksf.dimension, Dimension::PRESSURE);
        assert!(
            (ksf.to_si_scale - 47_880.258_980_3).abs() < 1e-3,
            "{}",
            ksf.to_si_scale
        );
        let percent = ok("pct", "0.01", &[]);
        assert!(percent.dimension.is_dimensionless());
    }

    #[test]
    fn a_unit_can_be_built_from_another_custom_unit() {
        let hand = ok("hand", "4 in", &[]);
        let two = ok("dhand", "2 hand", std::slice::from_ref(&hand));
        assert!((two.to_si_scale - 0.2032).abs() < 1e-12);
    }

    #[test]
    fn bad_definitions_say_what_is_wrong() {
        assert_eq!(err("u", "", &[]), DefineError::Empty);
        assert!(matches!(err("u", "4 +", &[]), DefineError::Parse(_)));
        assert!(matches!(err("u", "4 wibble", &[]), DefineError::Eval(_)));
        assert_eq!(err("u", "0 m", &[]), DefineError::NotPositive);
        assert_eq!(err("u", "-3 m", &[]), DefineError::NotPositive);
        assert_eq!(err("u", "x := 3 m", &[]), DefineError::NotAQuantity);
        assert_eq!(err("u", "[1, 2]", &[]), DefineError::NotAQuantity);
        assert_eq!(
            err("u", "i", &[]),
            DefineError::NotAQuantity,
            "a complex value"
        );
    }

    #[test]
    fn bad_names_are_the_registrys_errors() {
        assert!(matches!(
            err("m", "2 m", &[]),
            DefineError::Registry(RegistryError::Reserved(_))
        ));
        assert!(matches!(
            err("2x", "2 m", &[]),
            DefineError::Registry(RegistryError::BadSymbol(_))
        ));
        assert!(matches!(
            err("", "2 m", &[]),
            DefineError::Registry(RegistryError::BadSymbol(_))
        ));
        let hand = ok("hand", "4 in", &[]);
        assert!(matches!(
            err("hand", "5 in", std::slice::from_ref(&hand)),
            DefineError::Registry(RegistryError::Duplicate(_))
        ));
    }

    #[test]
    fn redefining_replaces_the_unit_and_a_unit_cannot_use_itself() {
        let hand = ok("hand", "4 in", &[]);
        let existing = std::slice::from_ref(&hand);
        let again = define_unit("hand", "5 in", existing, Some(0)).unwrap();
        assert!((again.to_si_scale - 0.127).abs() < 1e-12);
        assert!(matches!(
            define_unit("hand", "2 hand", existing, Some(0)),
            Err(DefineError::Eval(_))
        ));
        // Surrounding spaces in the name do not matter.
        assert_eq!(ok("  span ", "9 in", &[]).symbol, "span");
    }
}
