//! Named constants available in every worksheet: `pi`, `e`, standard gravity and
//! the speed of light. A variable or unit of the same name takes precedence, so a
//! worksheet that defines its own `e` or `g_n` is not disturbed.

use std::f64::consts::{E, PI};

use crate::units::{Dimension, Quantity};
use crate::value::Value;

pub struct Constant {
    pub name: &'static str,
    pub value: f64,
    pub dimension: Dimension,
    /// A short description for the suggestion list.
    pub summary: &'static str,
}

pub const CONSTANTS: &[Constant] = &[
    Constant {
        name: "pi",
        value: PI,
        dimension: Dimension::NONE,
        summary: "the ratio of a circle's circumference to its diameter",
    },
    Constant {
        name: "\u{3c0}",
        value: PI,
        dimension: Dimension::NONE,
        summary: "pi",
    },
    Constant {
        name: "e",
        value: E,
        dimension: Dimension::NONE,
        summary: "the base of natural logarithms",
    },
    Constant {
        name: "g_n",
        value: 9.806_65,
        dimension: Dimension::ACCELERATION,
        summary: "standard gravity, 9.80665 m/s^2",
    },
    Constant {
        name: "c_0",
        value: 299_792_458.0,
        dimension: Dimension::VELOCITY,
        summary: "speed of light in a vacuum, 299792458 m/s",
    },
];

/// The value of the constant called `name`, if there is one.
pub fn constant(name: &str) -> Option<Value> {
    if name == "i" {
        return Some(Value::complex(0.0, 1.0));
    }
    CONSTANTS.iter().find(|c| c.name == name).map(|c| {
        Quantity {
            si: c.value,
            dim: c.dimension,
        }
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_have_their_values_and_dimensions() {
        let pi = constant("pi").unwrap();
        assert_eq!(pi, constant("\u{3c0}").unwrap());
        assert_eq!(pi, Value::dimensionless(PI));
        assert_eq!(constant("i"), Some(Value::complex(0.0, 1.0)));
        assert!(constant("zzz").is_none());
        let g = Quantity::from_unit(1.0, "m")
            .unwrap()
            .over(
                Quantity::from_unit(1.0, "s")
                    .unwrap()
                    .pow(Quantity::dimensionless(2.0))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            constant("g_n"),
            Some(
                Quantity {
                    si: 9.80665,
                    dim: g.dim
                }
                .into()
            )
        );
    }

    #[test]
    fn names_are_unique_and_are_valid_names() {
        for (k, c) in CONSTANTS.iter().enumerate() {
            assert!(CONSTANTS[..k].iter().all(|o| o.name != c.name));
            assert!(c.name.chars().all(|ch| ch.is_alphanumeric() || ch == '_'));
        }
    }
}
