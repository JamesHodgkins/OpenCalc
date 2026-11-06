//! Display formatting for results. Display precision and number-format control
//! (spec section 2.4) and user-chosen display units will grow here.

use opencalc_core::numeric::Complex64;
use opencalc_core::registry::UnitSystem;
use opencalc_core::units::{Dimension, Quantity};
use opencalc_core::value::Value;

/// Units offered for automatic display in each unit system, smallest first
/// within a dimension. Conversion is for display only; values stay in SI.
const SI: &[(&str, Dimension, f64)] = &[
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

const USCS: &[(&str, Dimension, f64)] = &[
    ("in", Dimension::LENGTH, 0.0254),
    ("ft", Dimension::LENGTH, 0.3048),
    ("in\u{00b2}", Dimension::AREA, 0.0254 * 0.0254),
    ("ft\u{00b2}", Dimension::AREA, 0.3048 * 0.3048),
    ("lb", Dimension::MASS, 0.453_592_37),
    ("s", Dimension::TIME, 1.0),
    ("lbf", Dimension::FORCE, 4.448_221_615_260_5),
    ("kip", Dimension::FORCE, 4_448.221_615_260_5),
    ("psi", Dimension::PRESSURE, 6_894.757_293_168_36),
    ("ksi", Dimension::PRESSURE, 6_894_757.293_168_36),
];

const CGS: &[(&str, Dimension, f64)] = &[
    ("cm", Dimension::LENGTH, 1e-2),
    ("cm\u{00b2}", Dimension::AREA, 1e-4),
    ("g", Dimension::MASS, 1e-3),
    ("s", Dimension::TIME, 1.0),
    ("dyn", Dimension::FORCE, 1e-5),
    ("Ba", Dimension::PRESSURE, 0.1),
];

/// A system's display units for a dimension. A dimension the system has no unit
/// for (say, a volume in CGS) falls back to the SI table, then to SI base units.
fn candidates(system: UnitSystem, dim: Dimension) -> Vec<(&'static str, f64)> {
    let table = match system {
        UnitSystem::Si => SI,
        UnitSystem::Uscs => USCS,
        UnitSystem::Cgs => CGS,
    };
    let pick = |t: &'static [(&'static str, Dimension, f64)]| -> Vec<(&'static str, f64)> {
        t.iter()
            .filter(|(_, d, _)| *d == dim)
            .map(|(l, _, s)| (*l, *s))
            .collect()
    };
    let own = pick(table);
    if own.is_empty() {
        pick(SI)
    } else {
        own
    }
}

/// Relative slack when deciding whether a value reaches a unit. Arithmetic like
/// 10 kN / (0.1 m)^2 gives 999999.9999999998 Pa, which is 1 MPa to anyone reading it.
const ROUNDING: f64 = 1e-9;

/// The unit to show a magnitude of dimension `dim` in: the largest unit that
/// gives a value of at least 1 (so 5 N stays "5 N" and 10 000 N becomes "10 kN"),
/// or the smallest unit if the value is below 1 in all of them. Returns the
/// label and the scale to divide SI values by.
fn display_unit(system: UnitSystem, dim: Dimension, magnitude: f64) -> (String, f64) {
    if dim.is_dimensionless() {
        return (String::new(), 1.0);
    }
    let options = candidates(system, dim);
    let chosen = options
        .iter()
        .rfind(|(_, scale)| magnitude >= scale * (1.0 - ROUNDING))
        .or_else(|| options.iter().min_by(|a, b| a.1.total_cmp(&b.1)));
    match chosen {
        Some((label, scale)) => ((*label).to_owned(), *scale),
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

pub fn format_quantity(q: Quantity, system: UnitSystem) -> String {
    let (label, scale) = display_unit(system, q.dim, q.si.abs());
    with_unit(format!("{:.*}", digits(q.dim), q.si / scale), &label)
}

/// A part smaller than this fraction of the largest part shown alongside it is
/// displayed as zero. It is rounding noise, not a quantity: `exp(i*pi)` is
/// `-1 + 1.2e-16i` in floating point and should read `-1`. Display only; the
/// stored value is untouched. It is relative to the neighbours because an
/// absolute limit means nothing for a quantity in arbitrary units, so a lone real
/// number is never snapped.
const ZERO_RELATIVE: f64 = 1e-12;

fn snapped(x: f64, peak: f64) -> f64 {
    if x.abs() < ZERO_RELATIVE * peak {
        0.0
    } else {
        x
    }
}

/// A complex number as `a + bi`, with a sign that never doubles up. `peak` is the
/// largest magnitude it is shown alongside (itself, or its matrix).
fn complex_text(z: Complex64, scale: f64, digits: usize, peak: f64) -> String {
    let (re, im) = (snapped(z.re, peak) / scale, snapped(z.im, peak) / scale);
    if im == 0.0 {
        return format!("{re:.digits$}");
    }
    let sign = if im < 0.0 { '-' } else { '+' };
    format!("{re:.digits$} {sign} {:.digits$}i", im.abs())
}

/// Show any value. A complex scalar is `(3 + 4i) m`; a matrix lists its rows
/// separated by semicolons (`2x2 [1, 2; 3, 4]`) until matrices get typeset.
pub fn format_value(v: &Value, system: UnitSystem) -> String {
    const SHOWN: usize = 24;
    match v {
        Value::Scalar(s) => match s.as_real() {
            Some(q) => format_quantity(q, system),
            None => {
                let peak = s.z.re.abs().max(s.z.im.abs());
                let (label, scale) = display_unit(system, s.dim, peak);
                let text = complex_text(s.z, scale, digits(s.dim), peak);
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
            let (label, scale) = display_unit(system, m.dim, peak);
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
                    row.push(complex_text(m.at(r, c), scale, digits(m.dim), peak));
                    shown += 1;
                }
                rows.push(row.join(", "));
            }
            let size = format!("{}\u{00d7}{}", m.rows(), m.cols());
            with_unit(format!("{size} [{}]", rows.join("; ")), &label)
        }
        Value::Function { params } => format!("function of {}", params.join(", ")),
        Value::Series(r) => {
            let peak = r
                .elements()
                .iter()
                .fold(0.0_f64, |p, z| p.max(z.re.abs()).max(z.im.abs()));
            let (label, scale) = display_unit(system, r.dim, peak);
            let mut items: Vec<String> = r
                .elements()
                .iter()
                .take(SHOWN)
                .map(|z| complex_text(*z, scale, digits(r.dim), peak))
                .collect();
            if r.len() > SHOWN {
                items.push("\u{2026}".to_owned());
            }
            let count = if r.len() == 1 {
                "1 value".to_owned()
            } else {
                format!("{} values", r.len())
            };
            with_unit(format!("{count} [{}]", items.join(", ")), &label)
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
        format_quantity(Quantity::from_unit(v, unit).unwrap(), UnitSystem::Si)
    }

    fn shown(src: &str) -> String {
        use opencalc_core::eval::{evaluate, RegionId};
        let regions = vec![(RegionId(1), opencalc_core::parser::parse(src))];
        let out = evaluate(&regions);
        format_value(out.values[&RegionId(1)].as_ref().unwrap(), UnitSystem::Si)
    }

    fn shown_in(system: UnitSystem, v: f64, unit: &str) -> String {
        format_quantity(Quantity::from_unit(v, unit).unwrap(), system)
    }

    #[test]
    fn uscs_shows_feet_inches_and_psi() {
        let u = UnitSystem::Uscs;
        assert_eq!(shown_in(u, 1.0, "m"), "3.2808 ft");
        assert_eq!(shown_in(u, 5.0, "mm"), "0.1969 in");
        assert_eq!(shown_in(u, 1.0, "MPa"), "145.0377 psi");
        assert_eq!(shown_in(u, 100.0, "MPa"), "14.5038 ksi");
        assert_eq!(shown_in(u, 10.0, "kN"), "2.2481 kip");
        assert_eq!(shown_in(u, 1.0, "kg"), "2.2046 lb");
    }

    #[test]
    fn cgs_shows_centimetres_grams_and_dynes() {
        let c = UnitSystem::Cgs;
        assert_eq!(shown_in(c, 2.0, "m"), "200.0000 cm");
        assert_eq!(shown_in(c, 1.0, "N"), "100000.0000 dyn");
        assert_eq!(shown_in(c, 2.0, "kg"), "2000.0000 g");
    }

    #[test]
    fn a_dimension_with_no_unit_in_the_system_falls_back_to_si() {
        // Time has "s" everywhere; an unlisted dimension shows SI base units.
        let volume = Quantity::from_unit(2.0, "m")
            .unwrap()
            .pow(Quantity::dimensionless(3.0))
            .unwrap();
        for system in UnitSystem::ALL {
            assert_eq!(format_quantity(volume, system), "8.0000 m^3");
        }
    }

    #[test]
    fn complex_numbers_show_both_parts() {
        assert_eq!(shown("3 + 4 * i"), "3.000000 + 4.000000i");
        assert_eq!(shown("3 - 4 * i"), "3.000000 - 4.000000i");
        assert_eq!(shown("sqrt(0 - 4)"), "0.000000 + 2.000000i");
    }

    #[test]
    fn rounding_noise_next_to_a_real_part_is_not_shown() {
        // exp(i pi) is -1 + 1.22e-16 i in floating point.
        assert_eq!(shown("exp(i * 3.141592653589793)"), "-1.000000");
        // A genuinely small imaginary part survives.
        assert_eq!(shown("1 + 0.001 * i"), "1.000000 + 0.001000i");
        // Only noise relative to the neighbouring part is dropped.
        assert_eq!(shown("1e-20 + i"), "0.000000 + 1.000000i");
    }

    #[test]
    fn a_function_definition_reads_as_a_function_of_its_parameters() {
        assert_eq!(shown("f(x, y) := x * y"), "function of x, y");
    }

    #[test]
    fn a_list_shows_its_count_and_first_values() {
        assert_eq!(
            shown("0..3"),
            "4 values [0.000000, 1.000000, 2.000000, 3.000000]"
        );
        assert_eq!(shown("5..5"), "1 value [5.000000]");
        assert_eq!(
            shown("0 m, 1 m..2 m"),
            "3 values [0.0000, 1.0000, 2.0000] m"
        );
        let long = shown("0..99");
        assert!(long.starts_with("100 values [0.000000, 1.000000") && long.contains('\u{2026}'));
        assert!(long.len() < 300, "{long}");
    }

    #[test]
    fn matrix_noise_is_judged_against_the_largest_element() {
        assert_eq!(
            shown("mat(1, 2, 1, 1e-15)"),
            "1\u{00d7}2 [1.000000, 0.000000]"
        );
        // A lone small number is never snapped: it may be a real small quantity.
        assert_eq!(shown("1e-15"), "0.000000");
        assert_eq!(shown("2e-15 m"), "0.0000 mm");
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
        assert_eq!(format_quantity(stress, UnitSystem::Si), "1.0000 MPa");
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
        assert_eq!(
            format_quantity(small, UnitSystem::Si),
            "100.0000 mm\u{00b2}"
        );
        let big = Quantity::from_unit(3.0, "m")
            .unwrap()
            .pow(Quantity::dimensionless(2.0))
            .unwrap();
        assert_eq!(format_quantity(big, UnitSystem::Si), "9.0000 m\u{00b2}");
    }

    #[test]
    fn dimensionless_has_no_unit() {
        assert_eq!(
            format_quantity(Quantity::dimensionless(1.5), UnitSystem::Si),
            "1.500000"
        );
    }

    #[test]
    fn unknown_dimensions_show_si_base_units_not_a_debug_dump() {
        // 6 kN / 2 m = 3000 N/m has no display entry.
        let q = Quantity::from_unit(6.0, "kN")
            .unwrap()
            .over(Quantity::from_unit(2.0, "m").unwrap())
            .unwrap();
        assert_eq!(format_quantity(q, UnitSystem::Si), "3000.0000 kg*s^-2");
    }
}
