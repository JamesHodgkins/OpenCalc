//! Display formatting for results. Display precision and number-format control
//! (spec section 2.4) and user-chosen display units will grow here.

use opencalc_core::numeric::Complex64;
use opencalc_core::units::{Dimension, Quantity};
use opencalc_core::value::Value;

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

/// Relative slack when deciding whether a value reaches a unit. Arithmetic like
/// 10 kN / (0.1 m)^2 gives 999999.9999999998 Pa, which is 1 MPa to anyone reading it.
const ROUNDING: f64 = 1e-9;

/// The unit to show a magnitude of dimension `dim` in: the largest unit that
/// gives a value of at least 1 (so 5 N stays "5 N" and 10 000 N becomes "10 kN"),
/// or the smallest unit if the value is below 1 in all of them. Returns the
/// label and the scale to divide SI values by.
fn display_unit(dim: Dimension, magnitude: f64) -> (String, f64) {
    if dim.is_dimensionless() {
        return (String::new(), 1.0);
    }
    let candidates = DISPLAY.iter().filter(|(_, d, _)| *d == dim);
    let chosen = candidates
        .clone()
        .rfind(|(_, _, scale)| magnitude >= scale * (1.0 - ROUNDING))
        .or_else(|| candidates.min_by(|a, b| a.2.total_cmp(&b.2)));
    match chosen {
        Some((label, _, scale)) => ((*label).to_owned(), *scale),
        None => (si_base(dim), 1.0),
    }
}

fn digits(dim: Dimension) -> usize {
    if dim.is_dimensionless() {
        6
    } else {
        4
    }
}

fn with_unit(text: String, label: &str) -> String {
    if label.is_empty() {
        text
    } else {
        format!("{text} {label}")
    }
}

pub fn format_quantity(q: Quantity) -> String {
    let (label, scale) = display_unit(q.dim, q.si.abs());
    with_unit(format!("{:.*}", digits(q.dim), q.si / scale), &label)
}

/// A complex number as `a + bi`, with a sign that never doubles up.
fn complex_text(z: Complex64, scale: f64, digits: usize) -> String {
    let (re, im) = (z.re / scale, z.im / scale);
    if z.im == 0.0 {
        return format!("{re:.digits$}");
    }
    let sign = if im < 0.0 { '-' } else { '+' };
    format!("{re:.digits$} {sign} {:.digits$}i", im.abs())
}

/// Show any value. A complex scalar is `(3 + 4i) m`; a matrix lists its rows
/// separated by semicolons (`2x2 [1, 2; 3, 4]`) until matrices get typeset.
pub fn format_value(v: &Value) -> String {
    const SHOWN: usize = 24;
    match v {
        Value::Scalar(s) => match s.as_real() {
            Some(q) => format_quantity(q),
            None => {
                let (label, scale) = display_unit(s.dim, s.z.re.abs().max(s.z.im.abs()));
                let text = complex_text(s.z, scale, digits(s.dim));
                if label.is_empty() {
                    text
                } else {
                    format!("({text}) {label}")
                }
            }
        },
        Value::Matrix(m) => {
            let peak = m
                .elements()
                .iter()
                .fold(0.0_f64, |p, z| p.max(z.re.abs()).max(z.im.abs()));
            let (label, scale) = display_unit(m.dim, peak);
            let mut rows = Vec::new();
            let mut shown = 0;
            'rows: for r in 0..m.rows() {
                let mut row = Vec::new();
                for c in 0..m.cols() {
                    if shown == SHOWN {
                        row.push("\u{2026}".to_owned());
                        rows.push(row.join(", "));
                        break 'rows;
                    }
                    row.push(complex_text(m.at(r, c), scale, digits(m.dim)));
                    shown += 1;
                }
                rows.push(row.join(", "));
            }
            let size = format!("{}\u{00d7}{}", m.rows(), m.cols());
            with_unit(format!("{size} [{}]", rows.join("; ")), &label)
        }
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

    fn shown(src: &str) -> String {
        use opencalc_core::eval::{evaluate, RegionId};
        let regions = vec![(RegionId(1), opencalc_core::parser::parse(src))];
        let out = evaluate(&regions);
        format_value(out.values[&RegionId(1)].as_ref().unwrap())
    }

    #[test]
    fn complex_numbers_show_both_parts() {
        assert_eq!(shown("3 + 4 * i"), "3.000000 + 4.000000i");
        assert_eq!(shown("3 - 4 * i"), "3.000000 - 4.000000i");
        assert_eq!(shown("sqrt(0 - 4)"), "0.000000 + 2.000000i");
    }

    #[test]
    fn complex_quantities_share_one_unit() {
        assert_eq!(shown("(3 + 4 * i) * 1 kN"), "(3.0000 + 4.0000i) kN");
    }

    #[test]
    fn matrices_list_their_rows() {
        assert_eq!(
            shown("mat(2, 2, 1, 2, 3, 4)"),
            "2\u{00d7}2 [1.000000, 2.000000; 3.000000, 4.000000]"
        );
        assert_eq!(shown("vec(1 kN, 2 kN)"), "2\u{00d7}1 [1.0000; 2.0000] kN");
    }

    #[test]
    fn a_large_matrix_is_cut_short() {
        let items = vec!["1"; 30].join(", ");
        let text = shown(&format!("mat(6, 5, {items})"));
        assert!(text.len() < 400 && text.contains('\u{2026}'), "{text}");
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
    fn floating_point_error_does_not_push_a_value_into_the_smaller_unit() {
        // The worked example in the app: 10 kN over (100 mm)^2 is exactly 1 MPa.
        let force = Quantity::from_unit(10.0, "kN").unwrap();
        let side = Quantity::from_unit(100.0, "mm").unwrap();
        let area = side.pow(Quantity::dimensionless(2.0)).unwrap();
        let stress = force.over(area).unwrap();
        assert_ne!(
            stress.si, 1e6,
            "the point of this test: it is not exactly 1e6"
        );
        assert_eq!(format_quantity(stress), "1.0000 MPa");
        // But a genuinely smaller value still steps down.
        assert_eq!(show(999.0, "kPa"), "999.0000 kPa");
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
