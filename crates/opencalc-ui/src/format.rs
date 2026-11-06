//! Display formatting for results. Display precision and number-format control
//! (spec section 2.4) and user-chosen display units will grow here.

use opencalc_core::units::{Dimension, Quantity};

/// Units offered for automatic display, smallest first within a dimension.
/// Conversion is for display only; values stay in SI.
const DISPLAY: &[(&str, Dimension, f64)] = &[
    ("mm", Dimension::LENGTH, 1e-3),
    ("m", Dimension::LENGTH, 1.0),
    ("km", Dimension::LENGTH, 1e3),
    ("mm\u{00b2}", Dimension::AREA, 1e-6),
    ("m\u{00b2}", Dimension::AREA, 1.0),
    ("g", Dimension::MASS, 1e-3),
    ("kg", Dimension::MASS, 1.0),
    ("s", Dimension::TIME, 1.0),
    ("N", Dimension::FORCE, 1.0),
    ("kN", Dimension::FORCE, 1e3),
    ("Pa", Dimension::PRESSURE, 1.0),
    ("kPa", Dimension::PRESSURE, 1e3),
    ("MPa", Dimension::PRESSURE, 1e6),
];

/// Show a quantity in the largest unit of its dimension that gives a value of
/// at least 1 (so 5 N stays "5 N" and 10 000 N becomes "10 kN"), or the
/// smallest unit if the value is below 1 in all of them.
pub fn format_quantity(q: Quantity) -> String {
    if q.dim.is_dimensionless() {
        return format!("{:.6}", q.si);
    }
    let candidates = DISPLAY.iter().filter(|(_, d, _)| *d == q.dim);
    let chosen = candidates
        .clone()
        .rfind(|(_, _, scale)| q.si.abs() >= *scale)
        .or_else(|| candidates.min_by(|a, b| a.2.total_cmp(&b.2)));
    match chosen {
        Some((label, _, scale)) => format!("{:.4} {label}", q.si / scale),
        None => format!("{:.4} {}", q.si, si_base(q.dim)),
    }
}

/// A dimension as SI base units, such as `kg*m^-1*s^-2`.
fn si_base(d: Dimension) -> String {
    let parts = [
        ("m", d.length),
        ("kg", d.mass),
        ("s", d.time),
        ("A", d.current),
        ("K", d.temperature),
        ("mol", d.amount),
        ("cd", d.luminosity),
    ];
    let shown: Vec<String> = parts
        .iter()
        .filter(|(_, e)| *e != 0)
        .map(|(u, e)| {
            if *e == 1 {
                (*u).to_owned()
            } else {
                format!("{u}^{e}")
            }
        })
        .collect();
    shown.join("*")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(v: f64, unit: &str) -> String {
        format_quantity(Quantity::from_unit(v, unit).unwrap())
    }

    #[test]
    fn small_values_keep_their_unit() {
        assert_eq!(show(5.0, "N"), "5.0000 N");
        assert_eq!(show(100.0, "Pa"), "100.0000 Pa");
        assert_eq!(show(0.5, "kg"), "500.0000 g");
    }

    #[test]
    fn large_values_step_up_a_unit() {
        assert_eq!(show(10.0, "kN"), "10.0000 kN");
        assert_eq!(show(2.5e6, "Pa"), "2.5000 MPa");
        assert_eq!(show(3.0, "km"), "3.0000 km");
    }

    #[test]
    fn lengths_pick_metres_or_millimetres_sensibly() {
        assert_eq!(show(2.0, "m"), "2.0000 m");
        assert_eq!(show(0.1, "m"), "100.0000 mm");
        assert_eq!(show(100.0, "mm"), "100.0000 mm");
    }

    #[test]
    fn negative_values_use_their_magnitude_to_choose() {
        assert_eq!(show(-10.0, "kN"), "-10.0000 kN");
        assert_eq!(show(-5.0, "N"), "-5.0000 N");
    }

    #[test]
    fn zero_uses_the_smallest_unit() {
        assert_eq!(show(0.0, "kN"), "0.0000 N");
    }

    #[test]
    fn area_is_shown_in_square_millimetres_or_metres() {
        let small = Quantity::from_unit(0.01, "m")
            .unwrap()
            .pow(Quantity::dimensionless(2.0))
            .unwrap();
        assert_eq!(format_quantity(small), "100.0000 mm\u{00b2}");
        let big = Quantity::from_unit(3.0, "m")
            .unwrap()
            .pow(Quantity::dimensionless(2.0))
            .unwrap();
        assert_eq!(format_quantity(big), "9.0000 m\u{00b2}");
    }

    #[test]
    fn dimensionless_has_no_unit() {
        assert_eq!(format_quantity(Quantity::dimensionless(1.5)), "1.500000");
    }

    #[test]
    fn unknown_dimensions_show_si_base_units_not_a_debug_dump() {
        // 6 kN / 2 m = 3000 N/m has no display entry.
        let q = Quantity::from_unit(6.0, "kN")
            .unwrap()
            .over(Quantity::from_unit(2.0, "m").unwrap())
            .unwrap();
        assert_eq!(format_quantity(q), "3000.0000 kg*s^-2");
    }
}
