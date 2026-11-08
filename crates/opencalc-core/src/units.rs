//! Dimensions, quantities and a small built-in unit table (spec section 3.4).
//!
//! All arithmetic on values goes through [`Quantity`]. Values are stored in SI;
//! a unit symbol only matters when parsing input and formatting output.

use serde::{Deserialize, Serialize};

mod table;

pub use table::{builtin_symbols, builtin_unit, offset_symbols, offset_unit};

/// Integer exponents over the seven SI base dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Dimension {
    pub length: i8,
    pub mass: i8,
    pub time: i8,
    pub current: i8,
    pub temperature: i8,
    pub amount: i8,
    pub luminosity: i8,
}

/// A dimension in words, as error messages show it: `length`, `mass·length / time^2`.
/// No dimension at all reads as `a plain number`.
impl std::fmt::Display for Dimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const NAMES: [&str; 7] = [
            "length",
            "mass",
            "time",
            "current",
            "temperature",
            "amount",
            "luminosity",
        ];
        let part = |sign: i8| {
            let items: Vec<String> = self
                .exps()
                .iter()
                .zip(NAMES)
                .filter(|(e, _)| e.signum() == sign)
                .map(|(e, name)| match e.abs() {
                    1 => name.to_owned(),
                    n => format!("{name}^{n}"),
                })
                .collect();
            items.join("\u{b7}")
        };
        let (top, bottom) = (part(1), part(-1));
        match (top.is_empty(), bottom.is_empty()) {
            (true, true) => f.write_str("a plain number"),
            (false, true) => f.write_str(&top),
            (true, false) => write!(f, "1 / {bottom}"),
            (false, false) => write!(f, "{top} / {bottom}"),
        }
    }
}

impl Dimension {
    pub const NONE: Dimension = Dimension::new([0; 7]);
    pub const LENGTH: Dimension = Dimension::new([1, 0, 0, 0, 0, 0, 0]);
    pub const MASS: Dimension = Dimension::new([0, 1, 0, 0, 0, 0, 0]);
    pub const TIME: Dimension = Dimension::new([0, 0, 1, 0, 0, 0, 0]);
    /// Mass * length / time^2.
    pub const FORCE: Dimension = Dimension::new([1, 1, -2, 0, 0, 0, 0]);
    /// Force / length^2.
    pub const PRESSURE: Dimension = Dimension::new([-1, 1, -2, 0, 0, 0, 0]);
    pub const VELOCITY: Dimension = Dimension::new([1, 0, -1, 0, 0, 0, 0]);
    pub const ACCELERATION: Dimension = Dimension::new([1, 0, -2, 0, 0, 0, 0]);
    pub const AREA: Dimension = Dimension::new([2, 0, 0, 0, 0, 0, 0]);
    pub const VOLUME: Dimension = Dimension::new([3, 0, 0, 0, 0, 0, 0]);
    pub const CURRENT: Dimension = Dimension::new([0, 0, 0, 1, 0, 0, 0]);
    pub const TEMPERATURE: Dimension = Dimension::new([0, 0, 0, 0, 1, 0, 0]);
    pub const AMOUNT: Dimension = Dimension::new([0, 0, 0, 0, 0, 1, 0]);
    pub const LUMINOSITY: Dimension = Dimension::new([0, 0, 0, 0, 0, 0, 1]);
    /// Time^-1.
    pub const FREQUENCY: Dimension = Dimension::new([0, 0, -1, 0, 0, 0, 0]);
    /// Mass * length^2 / time^2.
    pub const ENERGY: Dimension = Dimension::new([2, 1, -2, 0, 0, 0, 0]);
    /// Mass * length^2 / time^3.
    pub const POWER: Dimension = Dimension::new([2, 1, -3, 0, 0, 0, 0]);
    /// Current * time.
    pub const CHARGE: Dimension = Dimension::new([0, 0, 1, 1, 0, 0, 0]);
    /// Power / current.
    pub const VOLTAGE: Dimension = Dimension::new([2, 1, -3, -1, 0, 0, 0]);
    /// Voltage / current.
    pub const RESISTANCE: Dimension = Dimension::new([2, 1, -3, -2, 0, 0, 0]);

    const fn new(e: [i8; 7]) -> Self {
        Dimension {
            length: e[0],
            mass: e[1],
            time: e[2],
            current: e[3],
            temperature: e[4],
            amount: e[5],
            luminosity: e[6],
        }
    }

    fn exps(self) -> [i8; 7] {
        [
            self.length,
            self.mass,
            self.time,
            self.current,
            self.temperature,
            self.amount,
            self.luminosity,
        ]
    }

    pub fn is_dimensionless(self) -> bool {
        self == Dimension::NONE
    }

    /// Dimension of a product: exponents add. `None` if an exponent overflows.
    pub fn checked_times(self, other: Dimension) -> Option<Dimension> {
        self.combine(other, i8::checked_add)
    }

    /// Dimension of a quotient: exponents subtract. `None` on overflow.
    pub fn checked_over(self, other: Dimension) -> Option<Dimension> {
        self.combine(other, i8::checked_sub)
    }

    fn combine(self, other: Dimension, op: fn(i8, i8) -> Option<i8>) -> Option<Dimension> {
        let (a, b) = (self.exps(), other.exps());
        let mut out = [0i8; 7];
        for i in 0..7 {
            out[i] = op(a[i], b[i])?;
        }
        Some(Dimension::new(out))
    }

    /// Square root of a dimension, if every exponent is even.
    pub fn halved(self) -> Option<Dimension> {
        let a = self.exps();
        if a.iter().any(|e| e % 2 != 0) {
            return None;
        }
        Some(Dimension::new(std::array::from_fn(|i| a[i] / 2)))
    }

    /// Dimension raised to an integer power. `None` if an exponent overflows.
    pub fn checked_powi(self, n: i8) -> Option<Dimension> {
        let a = self.exps();
        let mut out = [0i8; 7];
        for i in 0..7 {
            out[i] = a[i].checked_mul(n)?;
        }
        Some(Dimension::new(out))
    }
}

/// A unit definition: `value_in_unit * to_si_scale = value_in_si`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitDef {
    pub symbol: String,
    pub dimension: Dimension,
    pub to_si_scale: f64,
    /// What the user typed to define a custom unit, such as `4 in`. Empty for a
    /// built-in unit or a file that did not record it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub definition: String,
}

/// Errors from quantity arithmetic.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum UnitError {
    #[error("unit mismatch: cannot combine {0} with {1}")]
    Mismatch(Dimension, Dimension),
    #[error("function '{0}' needs a dimensionless argument")]
    NotDimensionless(String),
    #[error("unknown unit '{0}'")]
    UnknownUnit(String),
    #[error("unit exponent is too large")]
    DimensionOverflow,
}

/// A real value in SI with its dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    pub si: f64,
    pub dim: Dimension,
}

impl Quantity {
    pub fn dimensionless(v: f64) -> Self {
        Quantity {
            si: v,
            dim: Dimension::NONE,
        }
    }

    /// Build from a value expressed in the unit named `symbol`.
    pub fn from_unit(value: f64, symbol: &str) -> Result<Self, UnitError> {
        if let Some((scale, offset)) = offset_unit(symbol) {
            return Ok(Quantity {
                si: value * scale + offset,
                dim: Dimension::TEMPERATURE,
            });
        }
        let u = builtin_unit(symbol).ok_or_else(|| UnitError::UnknownUnit(symbol.to_owned()))?;
        Ok(Quantity {
            si: value * u.to_si_scale,
            dim: u.dimension,
        })
    }

    /// The value expressed in the unit named `symbol`, if dimensions match.
    pub fn in_unit(self, symbol: &str) -> Result<f64, UnitError> {
        if let Some((scale, offset)) = offset_unit(symbol) {
            if self.dim != Dimension::TEMPERATURE {
                return Err(UnitError::Mismatch(self.dim, Dimension::TEMPERATURE));
            }
            return Ok((self.si - offset) / scale);
        }
        let u = builtin_unit(symbol).ok_or_else(|| UnitError::UnknownUnit(symbol.to_owned()))?;
        if u.dimension != self.dim {
            return Err(UnitError::Mismatch(self.dim, u.dimension));
        }
        Ok(self.si / u.to_si_scale)
    }

    pub fn try_add(self, o: Quantity) -> Result<Quantity, UnitError> {
        self.same_dim(o).map(|d| Quantity {
            si: self.si + o.si,
            dim: d,
        })
    }

    pub fn try_sub(self, o: Quantity) -> Result<Quantity, UnitError> {
        self.same_dim(o).map(|d| Quantity {
            si: self.si - o.si,
            dim: d,
        })
    }

    pub fn times(self, o: Quantity) -> Result<Quantity, UnitError> {
        let dim = self
            .dim
            .checked_times(o.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        Ok(Quantity {
            si: self.si * o.si,
            dim,
        })
    }

    pub fn over(self, o: Quantity) -> Result<Quantity, UnitError> {
        let dim = self
            .dim
            .checked_over(o.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        Ok(Quantity {
            si: self.si / o.si,
            dim,
        })
    }

    /// Power with an integer exponent keeps dimensions; other exponents need a
    /// dimensionless base.
    pub fn pow(self, e: Quantity) -> Result<Quantity, UnitError> {
        if !e.dim.is_dimensionless() {
            return Err(UnitError::NotDimensionless("^".into()));
        }
        if e.si.fract() == 0.0 && e.si.abs() <= f64::from(i8::MAX) {
            let dim = self
                .dim
                .checked_powi(e.si as i8)
                .ok_or(UnitError::DimensionOverflow)?;
            return Ok(Quantity {
                si: self.si.powf(e.si),
                dim,
            });
        }
        if !self.dim.is_dimensionless() {
            return Err(UnitError::NotDimensionless("^".into()));
        }
        Ok(Quantity::dimensionless(self.si.powf(e.si)))
    }

    pub fn negated(self) -> Quantity {
        Quantity {
            si: -self.si,
            dim: self.dim,
        }
    }

    fn same_dim(self, o: Quantity) -> Result<Dimension, UnitError> {
        if self.dim == o.dim {
            Ok(self.dim)
        } else {
            Err(UnitError::Mismatch(self.dim, o.dim))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_read_in_words_in_error_messages() {
        let say = |d: Dimension| d.to_string();
        assert_eq!(say(Dimension::NONE), "a plain number");
        assert_eq!(say(Dimension::LENGTH), "length");
        assert_eq!(say(Dimension::AREA), "length^2");
        assert_eq!(say(Dimension::FORCE), "length·mass / time^2");
        assert_eq!(say(Dimension::PRESSURE), "mass / length·time^2");
        let one_over_time = Dimension::TIME.checked_powi(-1).expect("an exponent");
        assert_eq!(say(one_over_time), "1 / time");
        let message = UnitError::Mismatch(Dimension::LENGTH, Dimension::TIME).to_string();
        assert_eq!(message, "unit mismatch: cannot combine length with time");
    }

    #[test]
    fn force_is_mass_length_over_time_squared() {
        let n = builtin_unit("N").unwrap();
        assert_eq!(n.dimension.mass, 1);
        assert_eq!(n.dimension.length, 1);
        assert_eq!(n.dimension.time, -2);
    }

    #[test]
    fn conversion_round_trips() {
        let q = Quantity::from_unit(2.5, "ft").unwrap();
        assert!((q.in_unit("ft").unwrap() - 2.5).abs() < 1e-12);
        assert!((q.in_unit("in").unwrap() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn adding_length_to_mass_is_an_error() {
        let a = Quantity::from_unit(1.0, "m").unwrap();
        let b = Quantity::from_unit(1.0, "kg").unwrap();
        assert!(matches!(a.try_add(b), Err(UnitError::Mismatch(..))));
    }

    #[test]
    fn force_over_area_converts_to_pressure() {
        let f = Quantity::from_unit(10.0, "kN").unwrap();
        let a = Quantity::from_unit(0.01, "m")
            .unwrap()
            .pow(Quantity::dimensionless(2.0))
            .unwrap();
        let p = f.over(a).unwrap();
        assert!((p.in_unit("MPa").unwrap() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn fractional_power_of_dimensioned_base_is_an_error() {
        let a = Quantity::from_unit(4.0, "m").unwrap();
        assert!(a.pow(Quantity::dimensionless(0.5)).is_err());
    }

    #[test]
    fn dimension_exponent_overflow_is_an_error_not_a_panic() {
        let m = Quantity::from_unit(1.0, "m").unwrap();
        let area = m.times(m).unwrap();
        // (m^2)^100 needs an exponent of 200, which does not fit in an i8.
        assert_eq!(
            area.pow(Quantity::dimensionless(100.0)),
            Err(UnitError::DimensionOverflow)
        );
        let big = m.pow(Quantity::dimensionless(100.0)).unwrap();
        assert_eq!(big.times(big), Err(UnitError::DimensionOverflow));
        assert_eq!(
            big.over(Quantity::dimensionless(1.0)).map(|q| q.dim.length),
            Ok(100)
        );
        // Dividing by the inverse overflows the other way.
        let inv = m.pow(Quantity::dimensionless(-100.0)).unwrap();
        assert_eq!(big.over(inv), Err(UnitError::DimensionOverflow));
    }

    #[test]
    fn checked_dimension_arithmetic_matches_plain_arithmetic_in_range() {
        let f = Dimension::FORCE;
        assert_eq!(f.checked_over(Dimension::AREA), Some(Dimension::PRESSURE));
        assert_eq!(
            Dimension::LENGTH.checked_powi(127).map(|d| d.length),
            Some(127)
        );
        assert_eq!(Dimension::LENGTH.checked_powi(0), Some(Dimension::NONE));
        assert_eq!(
            Dimension::LENGTH
                .checked_powi(i8::MAX)
                .and_then(|d| d.checked_powi(2)),
            None
        );
    }

    #[test]
    fn named_dimension_constants_are_consistent() {
        assert_eq!(Dimension::AREA, Dimension::LENGTH.checked_powi(2).unwrap());
        assert_eq!(
            Dimension::PRESSURE,
            Dimension::FORCE.checked_over(Dimension::AREA).unwrap()
        );
        let force_from_base = Dimension::MASS
            .checked_times(Dimension::LENGTH)
            .and_then(|d| d.checked_over(Dimension::TIME.checked_powi(2)?));
        assert_eq!(force_from_base, Some(Dimension::FORCE));
    }

    #[test]
    fn the_unit_table_has_unique_symbols_and_sane_scales() {
        let symbols: Vec<&str> = builtin_symbols().collect();
        let mut sorted = symbols.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), symbols.len(), "duplicate unit symbol");
        for s in symbols {
            let u = builtin_unit(s).expect(s);
            assert!(u.to_si_scale.is_finite() && u.to_si_scale > 0.0, "{s}");
            // Angles are the only built-in units that are plain numbers.
            assert!(
                !u.dimension.is_dimensionless() || ["rad", "deg"].contains(&s),
                "{s} has no dimension"
            );
        }
        assert!(builtin_unit("furlong").is_none());
    }

    /// Relationships that are definitions or exact by convention, stated
    /// independently of the table's own numbers.
    #[test]
    fn unit_scales_agree_with_their_exact_definitions() {
        let si = |v: f64, u: &str| Quantity::from_unit(v, u).unwrap().si;
        assert_eq!(si(1.0, "in"), 0.0254, "the international inch");
        assert!(
            (si(1.0, "ft") - 12.0 * si(1.0, "in")).abs() < 1e-15,
            "12 in = 1 ft"
        );
        assert_eq!(si(1.0, "lb"), 0.453_592_37, "the avoirdupois pound");
        assert_eq!(si(1.0, "kN"), 1000.0 * si(1.0, "N"));
        assert_eq!(si(1.0, "MPa"), 1000.0 * si(1.0, "kPa"));
        assert_eq!(si(1.0, "kPa"), 1000.0 * si(1.0, "Pa"));
        assert_eq!(si(1000.0, "mm"), si(1.0, "m"));
        assert_eq!(si(100.0, "cm"), si(1.0, "m"));
        assert_eq!(si(1.0, "km"), 1000.0 * si(1.0, "m"));
        assert_eq!(si(1000.0, "g"), si(1.0, "kg"));
    }

    /// The units added in roadmap Phase 1.1, against definitions stated apart
    /// from the table.
    #[test]
    fn the_added_units_agree_with_their_definitions() {
        let si = |v: f64, u: &str| Quantity::from_unit(v, u).unwrap().si;
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * b.abs();
        assert!(close(si(3.0, "yd"), si(9.0, "ft")), "1 yd = 3 ft");
        assert!(
            close(si(1.0, "mi"), 5280.0 * si(1.0, "ft")),
            "1 mi = 5280 ft"
        );
        assert!(close(si(16.0, "oz"), si(1.0, "lb")), "16 oz = 1 lb");
        assert_eq!(si(1.0, "hr"), 60.0 * si(1.0, "min"));
        assert_eq!(si(1.0, "day"), 24.0 * si(1.0, "hr"));
        assert!(close(si(1.0, "kWh"), si(1.0, "kW") * si(1.0, "hr")));
        assert!(close(si(1.0, "L"), si(1000.0, "mL")));
        assert!(close(
            si(1.0, "gal"),
            231.0 * si(1.0, "in") * si(1.0, "in") * si(1.0, "in")
        ));
        assert!(close(
            si(1.0, "psf"),
            si(1.0, "lbf") / (si(1.0, "ft") * si(1.0, "ft"))
        ));
        assert!(
            close(si(1.0, "hp"), 550.0 * si(1.0, "lbf") * si(1.0, "ft")),
            "1 hp = 550 ft lbf/s"
        );
        assert_eq!(si(1.0, "atm"), 101_325.0);
    }

    #[test]
    fn celsius_and_fahrenheit_convert_through_kelvin() {
        let k = |v: f64, u: &str| Quantity::from_unit(v, u).unwrap().si;
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(k(0.0, "degC"), 273.15), "freezing point of water");
        assert!(close(k(100.0, "degC"), 373.15), "boiling point of water");
        assert!(close(k(32.0, "degF"), 273.15));
        assert!(close(k(212.0, "degF"), 373.15));
        assert!(
            close(k(-40.0, "degF"), k(-40.0, "degC")),
            "-40 is the same on both"
        );
        assert!(close(k(-273.15, "degC"), 0.0), "absolute zero");
        assert!(close(k(0.0, "degR"), 0.0) && close(k(491.67, "degR"), 273.15));
    }

    #[test]
    fn temperature_units_convert_back_and_refuse_other_dimensions() {
        let q = Quantity::from_unit(25.0, "degC").unwrap();
        assert!((q.in_unit("degC").unwrap() - 25.0).abs() < 1e-12);
        assert!((q.in_unit("degF").unwrap() - 77.0).abs() < 1e-9);
        assert!((q.in_unit("K").unwrap() - 298.15).abs() < 1e-12);
        let length = Quantity::from_unit(1.0, "m").unwrap();
        assert!(length.in_unit("degC").is_err());
    }

    #[test]
    fn delta_units_are_steps_without_an_offset() {
        let k = |v: f64, u: &str| Quantity::from_unit(v, u).unwrap().si;
        assert_eq!(k(10.0, "deltaC"), 10.0);
        assert!((k(9.0, "deltaF") - 5.0).abs() < 1e-12);
        assert!((k(9.0, "degR") - 5.0).abs() < 1e-12);
        assert!(offset_unit("deltaC").is_none() && offset_unit("K").is_none());
    }

    #[test]
    fn angles_are_plain_numbers_in_radians() {
        let pi = std::f64::consts::PI;
        let q = |v: f64, u: &str| Quantity::from_unit(v, u).unwrap();
        assert_eq!(q(1.0, "rad"), Quantity::dimensionless(1.0));
        assert!((q(180.0, "deg").si - pi).abs() < 1e-15);
        assert!(q(90.0, "deg").dim.is_dimensionless());
        assert!((q(pi, "rad").in_unit("deg").unwrap() - 180.0).abs() < 1e-12);
        // A length is not an angle, and an angle is not a length.
        assert!(q(1.0, "m").in_unit("deg").is_err());
        assert!(q(1.0, "deg").in_unit("m").is_err());
    }

    /// Derived units must be the products and quotients of base units.
    #[test]
    fn derived_unit_dimensions_follow_from_the_base_units() {
        let mul = |a: Quantity, b: Quantity| a.times(b).unwrap();
        let div = |a: Quantity, b: Quantity| a.over(b).unwrap();
        let q = |u: &str| Quantity::from_unit(1.0, u).unwrap();
        assert_eq!(mul(q("N"), q("m")).dim, q("J").dim, "J = N m");
        assert_eq!(div(q("J"), q("s")).dim, q("W").dim, "W = J / s");
        assert_eq!(div(q("W"), q("A")).dim, q("V").dim, "V = W / A");
        assert_eq!(div(q("V"), q("A")).dim, q("ohm").dim, "ohm = V / A");
        assert_eq!(mul(q("A"), q("s")).dim, q("C").dim, "C = A s");
        assert_eq!(div(q("m"), q("m")).dim, Dimension::NONE);
        assert_eq!(div(Quantity::dimensionless(1.0), q("s")).dim, q("Hz").dim);
        assert_eq!(
            mul(mul(q("m"), q("m")), q("m")).dim,
            q("L").dim,
            "L is a volume"
        );
    }

    #[test]
    fn mixing_unit_dimension_in_conversion_fails() {
        let a = Quantity::from_unit(1.0, "m").unwrap();
        assert!(a.in_unit("kg").is_err());
    }
}
