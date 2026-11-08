//! The built-in unit tables: scale-only units and the two offset units.

use super::{Dimension, UnitDef};

/// The built-in units: symbol, dimension, and the factor that turns a value in
/// the unit into SI. One table, so a unit cannot be added in one place and
/// missed in another. Offset scales (degC, degF) come later (see TECH_DEBT).
const BUILTIN: &[(&str, Dimension, f64)] = &[
    ("m", Dimension::LENGTH, 1.0),
    ("mm", Dimension::LENGTH, 1e-3),
    ("cm", Dimension::LENGTH, 1e-2),
    ("km", Dimension::LENGTH, 1e3),
    ("in", Dimension::LENGTH, 0.0254),
    ("ft", Dimension::LENGTH, 0.3048),
    ("kg", Dimension::MASS, 1.0),
    ("g", Dimension::MASS, 1e-3),
    ("lb", Dimension::MASS, 0.453_592_37),
    ("s", Dimension::TIME, 1.0),
    ("N", Dimension::FORCE, 1.0),
    ("kN", Dimension::FORCE, 1e3),
    ("Pa", Dimension::PRESSURE, 1.0),
    ("kPa", Dimension::PRESSURE, 1e3),
    ("MPa", Dimension::PRESSURE, 1e6),
    // US customary and CGS, so results can be shown in them (see `UnitSystem`).
    ("lbf", Dimension::FORCE, 4.448_221_615_260_5),
    ("kip", Dimension::FORCE, 4_448.221_615_260_5),
    ("psi", Dimension::PRESSURE, 6_894.757_293_168_36),
    ("ksi", Dimension::PRESSURE, 6_894_757.293_168_36),
    ("dyn", Dimension::FORCE, 1e-5),
    ("Ba", Dimension::PRESSURE, 0.1),
    // More length, mass and time.
    ("um", Dimension::LENGTH, 1e-6),
    ("nm", Dimension::LENGTH, 1e-9),
    ("yd", Dimension::LENGTH, 0.9144),
    ("mi", Dimension::LENGTH, 1_609.344),
    ("mg", Dimension::MASS, 1e-6),
    ("oz", Dimension::MASS, 0.028_349_523_125),
    ("tonne", Dimension::MASS, 1e3),
    ("ms", Dimension::TIME, 1e-3),
    ("min", Dimension::TIME, 60.0),
    ("hr", Dimension::TIME, 3_600.0),
    ("day", Dimension::TIME, 86_400.0),
    ("Hz", Dimension::FREQUENCY, 1.0),
    ("kHz", Dimension::FREQUENCY, 1e3),
    ("MHz", Dimension::FREQUENCY, 1e6),
    // The other SI base dimensions. Kelvin has no offset; degC and degF are not
    // built in (see TECH_DEBT TD-09).
    ("A", Dimension::CURRENT, 1.0),
    ("mA", Dimension::CURRENT, 1e-3),
    ("K", Dimension::TEMPERATURE, 1.0),
    ("mol", Dimension::AMOUNT, 1.0),
    ("cd", Dimension::LUMINOSITY, 1.0),
    // Derived units.
    ("J", Dimension::ENERGY, 1.0),
    ("kJ", Dimension::ENERGY, 1e3),
    ("MJ", Dimension::ENERGY, 1e6),
    ("kWh", Dimension::ENERGY, 3.6e6),
    ("cal", Dimension::ENERGY, 4.184),
    ("BTU", Dimension::ENERGY, 1_055.055_852_62),
    ("W", Dimension::POWER, 1.0),
    ("kW", Dimension::POWER, 1e3),
    ("MW", Dimension::POWER, 1e6),
    ("hp", Dimension::POWER, 745.699_871_582_270_2),
    ("C", Dimension::CHARGE, 1.0),
    ("V", Dimension::VOLTAGE, 1.0),
    ("kV", Dimension::VOLTAGE, 1e3),
    ("mV", Dimension::VOLTAGE, 1e-3),
    ("ohm", Dimension::RESISTANCE, 1.0),
    ("kohm", Dimension::RESISTANCE, 1e3),
    ("bar", Dimension::PRESSURE, 1e5),
    ("atm", Dimension::PRESSURE, 101_325.0),
    ("psf", Dimension::PRESSURE, 47.880_258_980_335_84),
    // Temperature steps with no offset: kelvin and the delta units (a rise of
    // one degree Celsius is one kelvin; one Fahrenheit or Rankine degree is 5/9).
    ("deltaC", Dimension::TEMPERATURE, 1.0),
    ("deltaF", Dimension::TEMPERATURE, 5.0 / 9.0),
    ("degR", Dimension::TEMPERATURE, 5.0 / 9.0),
    // Angles are plain numbers (radians), so `sin(30 deg)` works and an inverse
    // function gives radians: divide by `deg` to read the answer in degrees. A
    // value in `deg` is stored as its radian number and displays without a unit.
    ("rad", Dimension::NONE, 1.0),
    ("deg", Dimension::NONE, std::f64::consts::PI / 180.0),
    ("L", Dimension::VOLUME, 1e-3),
    ("mL", Dimension::VOLUME, 1e-6),
    ("gal", Dimension::VOLUME, 0.003_785_411_784),
];

/// Units whose zero is not kelvin's: `kelvin = value * scale + offset`. They name
/// a point on a scale, not a step, so they are accepted only directly after a
/// number (`20 degC`) and never in a product, quotient or power. Use `deltaC` or
/// `deltaF` for a temperature difference or a rate such as `W / (m * deltaC)`.
/// Everything is calculated in kelvin, so `20 degC + 10 degC` is not refused.
const OFFSET: &[(&str, f64, f64)] = &[
    ("degC", 1.0, 273.15),
    ("degF", 5.0 / 9.0, 459.67 * 5.0 / 9.0),
];

/// `(scale, offset)` of an offset unit such as `degC`.
pub fn offset_unit(symbol: &str) -> Option<(f64, f64)> {
    OFFSET
        .iter()
        .find(|(s, _, _)| *s == symbol)
        .map(|(_, scale, offset)| (*scale, *offset))
}

/// The offset unit symbols, for listing and completion.
pub fn offset_symbols() -> impl Iterator<Item = &'static str> {
    OFFSET.iter().map(|(s, _, _)| *s)
}

/// Look up a built-in unit by symbol.
pub fn builtin_unit(symbol: &str) -> Option<UnitDef> {
    BUILTIN
        .iter()
        .find(|(s, _, _)| *s == symbol)
        .map(|(s, dimension, scale)| UnitDef {
            symbol: (*s).to_owned(),
            dimension: *dimension,
            to_si_scale: *scale,
            definition: String::new(),
        })
}

/// Every built-in unit symbol, for tests and for listing units in the UI.
pub fn builtin_symbols() -> impl Iterator<Item = &'static str> {
    BUILTIN.iter().map(|(s, _, _)| *s)
}
