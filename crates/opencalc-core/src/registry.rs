//! Units a worksheet knows: the built-in table plus the worksheet's own, and the
//! unit system it displays results in.

use serde::{Deserialize, Serialize};

use crate::units::{builtin_unit, Quantity, UnitDef, UnitError};

/// Which units results are shown in. Values are always stored in SI; this only
/// changes display (spec 2.3), so switching it never changes a calculation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitSystem {
    #[default]
    Si,
    /// US customary: feet, inches, pounds-force, psi.
    Uscs,
    /// Centimetre-gram-second.
    Cgs,
}

impl UnitSystem {
    pub const ALL: [UnitSystem; 3] = [UnitSystem::Si, UnitSystem::Uscs, UnitSystem::Cgs];

    pub fn label(self) -> &'static str {
        match self {
            UnitSystem::Si => "SI",
            UnitSystem::Uscs => "USCS",
            UnitSystem::Cgs => "CGS",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("'{0}' is not a valid unit name")]
    BadSymbol(String),
    #[error("'{0}' is already a built-in unit, a constant or a function")]
    Reserved(String),
    #[error("unit '{0}' is defined twice")]
    Duplicate(String),
    #[error("unit '{0}' needs a positive, finite scale")]
    BadScale(String),
}

/// The imaginary unit is the only constant; functions are in `functions`.
const CONSTANTS: &[&str] = &["i"];

fn reserved(symbol: &str) -> bool {
    CONSTANTS.contains(&symbol) || crate::functions::lookup(symbol).is_some()
}

const MAX_SYMBOL_CHARS: usize = 32;

fn valid_symbol(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && s.chars().count() <= MAX_SYMBOL_CHARS
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// The units one worksheet can use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitRegistry {
    custom: Vec<UnitDef>,
}

impl UnitRegistry {
    /// Built-in units only.
    pub fn builtin() -> UnitRegistry {
        UnitRegistry::default()
    }

    /// Built-in units plus `custom`, which are checked: a bad definition is an
    /// error here, not a surprise during evaluation.
    pub fn new(custom: Vec<UnitDef>) -> Result<UnitRegistry, RegistryError> {
        for (k, def) in custom.iter().enumerate() {
            let s = def.symbol.as_str();
            if !valid_symbol(s) {
                return Err(RegistryError::BadSymbol(s.to_owned()));
            }
            if builtin_unit(s).is_some() || reserved(s) {
                return Err(RegistryError::Reserved(s.to_owned()));
            }
            if custom[..k].iter().any(|d| d.symbol == def.symbol) {
                return Err(RegistryError::Duplicate(s.to_owned()));
            }
            if !(def.to_si_scale.is_finite() && def.to_si_scale > 0.0) {
                return Err(RegistryError::BadScale(s.to_owned()));
            }
        }
        Ok(UnitRegistry { custom })
    }

    pub fn custom(&self) -> &[UnitDef] {
        &self.custom
    }

    pub fn lookup(&self, symbol: &str) -> Option<UnitDef> {
        builtin_unit(symbol).or_else(|| self.custom.iter().find(|d| d.symbol == symbol).cloned())
    }

    /// `value` expressed in the unit `symbol`, as an SI quantity.
    pub fn quantity(&self, value: f64, symbol: &str) -> Result<Quantity, UnitError> {
        let u = self
            .lookup(symbol)
            .ok_or_else(|| UnitError::UnknownUnit(symbol.to_owned()))?;
        Ok(Quantity {
            si: value * u.to_si_scale,
            dim: u.dimension,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Dimension;

    fn def(symbol: &str, scale: f64) -> UnitDef {
        UnitDef {
            symbol: symbol.into(),
            dimension: Dimension::LENGTH,
            to_si_scale: scale,
        }
    }

    #[test]
    fn a_custom_unit_converts_like_a_built_in() {
        let r = UnitRegistry::new(vec![def("furlong", 201.168)]).unwrap();
        let q = r.quantity(2.0, "furlong").unwrap();
        assert!((q.si - 402.336).abs() < 1e-9 && q.dim == Dimension::LENGTH);
        assert!(r.quantity(1.0, "nope").is_err());
        assert!(UnitRegistry::builtin().quantity(1.0, "furlong").is_err());
    }

    #[test]
    fn bad_definitions_are_rejected() {
        let err = |d: Vec<UnitDef>| UnitRegistry::new(d).unwrap_err();
        assert!(matches!(
            err(vec![def("m", 2.0)]),
            RegistryError::Reserved(_)
        ));
        assert!(matches!(
            err(vec![def("i", 2.0)]),
            RegistryError::Reserved(_)
        ));
        assert!(matches!(
            err(vec![def("sin", 2.0)]),
            RegistryError::Reserved(_)
        ));
        assert!(matches!(
            err(vec![def("", 2.0)]),
            RegistryError::BadSymbol(_)
        ));
        assert!(matches!(
            err(vec![def("2x", 2.0)]),
            RegistryError::BadSymbol(_)
        ));
        assert!(matches!(
            err(vec![def("a b", 2.0)]),
            RegistryError::BadSymbol(_)
        ));
        assert!(matches!(
            err(vec![def("x", 0.0)]),
            RegistryError::BadScale(_)
        ));
        assert!(matches!(
            err(vec![def("x", -1.0)]),
            RegistryError::BadScale(_)
        ));
        assert!(matches!(
            err(vec![def("x", f64::NAN)]),
            RegistryError::BadScale(_)
        ));
        assert!(matches!(
            err(vec![def("x", 1.0), def("x", 2.0)]),
            RegistryError::Duplicate(_)
        ));
    }
}
