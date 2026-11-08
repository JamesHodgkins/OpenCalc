//! Display formatting for results. Display precision and number-format control
//! (spec section 2.4) and user-chosen display units will grow here.

use crate::number_format::number_in;
use opencalc_core::document::ResultFormat;
use opencalc_core::numeric::Complex64;
use opencalc_core::registry::UnitSystem;
use opencalc_core::units::{builtin_unit, Dimension, Quantity};
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
    ("m\u{00b3}", Dimension::VOLUME, 1.0),
    ("Hz", Dimension::FREQUENCY, 1.0),
    ("kHz", Dimension::FREQUENCY, 1e3),
    ("MHz", Dimension::FREQUENCY, 1e6),
    ("J", Dimension::ENERGY, 1.0),
    ("kJ", Dimension::ENERGY, 1e3),
    ("MJ", Dimension::ENERGY, 1e6),
    ("W", Dimension::POWER, 1.0),
    ("kW", Dimension::POWER, 1e3),
    ("MW", Dimension::POWER, 1e6),
    ("C", Dimension::CHARGE, 1.0),
    ("mV", Dimension::VOLTAGE, 1e-3),
    ("V", Dimension::VOLTAGE, 1.0),
    ("kV", Dimension::VOLTAGE, 1e3),
    ("ohm", Dimension::RESISTANCE, 1.0),
    ("kohm", Dimension::RESISTANCE, 1e3),
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
    ("in\u{00b3}", Dimension::VOLUME, 0.0254 * 0.0254 * 0.0254),
    ("ft\u{00b3}", Dimension::VOLUME, 0.3048 * 0.3048 * 0.3048),
];

const CGS: &[(&str, Dimension, f64)] = &[
    ("cm", Dimension::LENGTH, 1e-2),
    ("cm\u{00b2}", Dimension::AREA, 1e-4),
    ("g", Dimension::MASS, 1e-3),
    ("s", Dimension::TIME, 1.0),
    ("dyn", Dimension::FORCE, 1e-5),
    ("Ba", Dimension::PRESSURE, 0.1),
    ("cm\u{00b3}", Dimension::VOLUME, 1e-6),
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

/// [`display_unit`], unless the region asks for a unit of this dimension.
fn unit_in(
    system: UnitSystem,
    dim: Dimension,
    magnitude: f64,
    format: ResultFormat,
) -> (String, f64) {
    let asked = format
        .unit()
        .and_then(|u| builtin_unit(u.symbol()))
        .filter(|u| u.dimension == dim && !dim.is_dimensionless());
    match asked {
        Some(u) => (u.symbol, u.to_si_scale),
        None => display_unit(system, dim, magnitude),
    }
}

/// The unit label and scale (divide SI values by it) to show an axis in, for
/// data up to `peak` in the given dimension.
pub fn axis_unit(system: UnitSystem, dim: Dimension, peak: f64) -> (String, f64) {
    display_unit(system, dim, peak)
}

/// A number to three decimal places with trailing zeros dropped (`10`, `0.333`,
/// `-1.5`), as Mathcad shows results by default. Numbers too large or too small to
/// read plainly use scientific notation (`1.5e-7`, `2.5e12`).
#[cfg(test)]
pub fn number_text(x: f64) -> String {
    number_in(x, ResultFormat::default())
}

fn with_unit(text: String, label: &str) -> String {
    if label.is_empty() {
        text
    } else {
        format!("{text} {label}")
    }
}

pub fn format_quantity(q: Quantity, system: UnitSystem) -> String {
    format_quantity_with(q, system, ResultFormat::default())
}

fn format_quantity_with(q: Quantity, system: UnitSystem, format: ResultFormat) -> String {
    let (label, scale) = unit_in(system, q.dim, q.si.abs(), format);
    with_unit(number_in(q.si / scale, format), &label)
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
fn complex_text(z: Complex64, scale: f64, peak: f64, format: ResultFormat) -> String {
    let (re, im) = (snapped(z.re, peak) / scale, snapped(z.im, peak) / scale);
    if im == 0.0 {
        return number_in(re, format);
    }
    let sign = if im < 0.0 { '-' } else { '+' };
    format!(
        "{} {sign} {}i",
        number_in(re, format),
        number_in(im.abs(), format)
    )
}

/// Show any value. A complex scalar is `(3 + 4i) m`; a matrix lists its rows
/// separated by semicolons (`2x2 [1, 2; 3, 4]`) until matrices get typeset.
#[cfg(test)]
pub fn format_value(v: &Value, system: UnitSystem) -> String {
    format_value_with(v, system, ResultFormat::default())
}

/// [`format_value`] with the number format a region asks for.
pub fn format_value_with(v: &Value, system: UnitSystem, format: ResultFormat) -> String {
    // A list is shown on one line, so it gets fewer values than a matrix grid.
    const SHOWN_IN_LIST: usize = 8;
    const SHOWN: usize = 24;
    match v {
        Value::Scalar(s) => match s.as_real() {
            Some(q) => format_quantity_with(q, system, format),
            None => {
                let peak = s.z.re.abs().max(s.z.im.abs());
                let (label, scale) = unit_in(system, s.dim, peak, format);
                let text = complex_text(s.z, scale, peak, format);
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
            let (label, scale) = unit_in(system, m.dim, peak, format);
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
                    row.push(complex_text(m.at(r, c), scale, peak, format));
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
            let (label, scale) = unit_in(system, r.dim, peak, format);
            let mut items: Vec<String> = r
                .elements()
                .iter()
                .take(SHOWN_IN_LIST)
                .map(|z| complex_text(*z, scale, peak, format))
                .collect();
            if r.len() > SHOWN_IN_LIST {
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

    #[test]
    fn derived_units_show_by_name_not_as_base_units() {
        assert_eq!(show(1500.0, "W"), "1.5 kW");
        assert_eq!(show(2.0, "J"), "2 J");
        assert_eq!(show(5.0, "V"), "5 V");
        assert_eq!(show(50.0, "Hz"), "50 Hz");
        assert_eq!(show(4.0, "ohm"), "4 ohm");
        assert_eq!(shown("2 m * 3 m * 4 m"), "24 m\u{00b3}");
        assert_eq!(shown("10 N * 3 m"), "30 J");
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
        assert_eq!(shown_in(u, 1.0, "m"), "3.281 ft");
        assert_eq!(shown_in(u, 5.0, "mm"), "0.197 in");
        assert_eq!(shown_in(u, 1.0, "MPa"), "145.038 psi");
        assert_eq!(shown_in(u, 100.0, "MPa"), "14.504 ksi");
        assert_eq!(shown_in(u, 10.0, "kN"), "2.248 kip");
        assert_eq!(shown_in(u, 1.0, "kg"), "2.205 lb");
    }

    #[test]
    fn cgs_shows_centimetres_grams_and_dynes() {
        let c = UnitSystem::Cgs;
        assert_eq!(shown_in(c, 2.0, "m"), "200 cm");
        assert_eq!(shown_in(c, 1.0, "N"), "100000 dyn");
        assert_eq!(shown_in(c, 2.0, "kg"), "2000 g");
    }

    #[test]
    fn a_dimension_with_no_unit_in_the_system_falls_back_to_si() {
        // Time has "s" everywhere; an unlisted dimension shows SI base units.
        // Volume has a display unit in each system, so use length^4.
        let quartic = Quantity::from_unit(2.0, "m")
            .unwrap()
            .pow(Quantity::dimensionless(4.0))
            .unwrap();
        for system in UnitSystem::ALL {
            assert_eq!(format_quantity(quartic, system), "16 m^4");
        }
    }

    #[test]
    fn complex_numbers_show_both_parts() {
        assert_eq!(shown("3 + 4 * i"), "3 + 4i");
        assert_eq!(shown("3 - 4 * i"), "3 - 4i");
        assert_eq!(shown("sqrt(0 - 4)"), "0 + 2i");
    }

    #[test]
    fn rounding_noise_next_to_a_real_part_is_not_shown() {
        // exp(i pi) is -1 + 1.22e-16 i in floating point.
        assert_eq!(shown("exp(i * 3.141592653589793)"), "-1");
        // A genuinely small imaginary part survives.
        assert_eq!(shown("1 + 0.001 * i"), "1 + 0.001i");
        // Only noise relative to the neighbouring part is dropped.
        assert_eq!(shown("1e-20 + i"), "0 + 1i");
    }

    #[test]
    fn a_function_definition_reads_as_a_function_of_its_parameters() {
        assert_eq!(shown("f(x, y) := x * y"), "function of x, y");
    }

    #[test]
    fn a_list_shows_its_count_and_first_values() {
        assert_eq!(shown("0..3"), "4 values [0, 1, 2, 3]");
        assert_eq!(shown("5..5"), "1 value [5]");
        assert_eq!(shown("0 m, 1 m..2 m"), "3 values [0, 1, 2] m");
        let long = shown("0..99");
        assert!(long.starts_with("100 values [0, 1") && long.contains('\u{2026}'));
        assert!(long.len() < 120, "{long}");
    }

    #[test]
    fn matrix_noise_is_judged_against_the_largest_element() {
        assert_eq!(shown("mat(1, 2, 1, 1e-15)"), "1\u{00d7}2 [1, 0]");
        // A lone small number is never snapped: it may be a real small quantity.
        // ...and so it shows as it is, in scientific notation, not as a false zero.
        assert_eq!(shown("1e-15"), "1e-15");
        assert_eq!(shown("2e-15 m"), "2e-12 mm");
    }

    #[test]
    fn complex_quantities_share_one_unit() {
        assert_eq!(shown("(3 + 4 * i) * 1 kN"), "(3 + 4i) kN");
    }

    #[test]
    fn matrices_list_their_rows() {
        assert_eq!(shown("mat(2, 2, 1, 2, 3, 4)"), "2\u{00d7}2 [1, 2; 3, 4]");
        assert_eq!(shown("vec(1 kN, 2 kN)"), "2\u{00d7}1 [1; 2] kN");
    }

    #[test]
    fn a_large_matrix_is_cut_short() {
        let items = vec!["1"; 30].join(", ");
        let text = shown(&format!("mat(6, 5, {items})"));
        assert!(text.len() < 400 && text.contains('\u{2026}'), "{text}");
    }

    #[test]
    fn numbers_have_three_decimals_without_trailing_zeros() {
        for (x, want) in [
            (10.0, "10"),
            (0.5, "0.5"),
            (1.0 / 3.0, "0.333"),
            (2.0 / 3.0, "0.667"),
            (-1.5, "-1.5"),
            (0.9999, "1"),
            (1234.5678, "1234.568"),
            (0.0, "0"),
            (-0.0, "0"),
            (999_999_999.0, "999999999"),
        ] {
            assert_eq!(number_text(x), want, "{x}");
        }
    }

    #[test]
    fn very_large_and_very_small_numbers_use_scientific_notation() {
        for (x, want) in [
            (1.5e-7, "1.5e-7"),
            (2.5e12, "2.5e12"),
            (1e9, "1e9"),
            (0.0009, "9e-4"),
            (-3.14159e-5, "-3.142e-5"),
            (0.001, "0.001"),
        ] {
            assert_eq!(number_text(x), want, "{x}");
        }
    }

    #[test]
    fn small_values_keep_their_unit() {
        assert_eq!(show(5.0, "N"), "5 N");
        assert_eq!(show(100.0, "Pa"), "100 Pa");
        assert_eq!(show(0.5, "kg"), "500 g");
    }

    #[test]
    fn large_values_step_up_a_unit() {
        assert_eq!(show(10.0, "kN"), "10 kN");
        assert_eq!(show(2.5e6, "Pa"), "2.5 MPa");
        assert_eq!(show(3.0, "km"), "3 km");
    }

    #[test]
    fn lengths_pick_metres_or_millimetres_sensibly() {
        assert_eq!(show(2.0, "m"), "2 m");
        assert_eq!(show(0.1, "m"), "100 mm");
        assert_eq!(show(100.0, "mm"), "100 mm");
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
        assert_eq!(format_quantity(stress, UnitSystem::Si), "1 MPa");
        // But a genuinely smaller value still steps down.
        assert_eq!(show(999.0, "kPa"), "999 kPa");
    }

    #[test]
    fn negative_values_use_their_magnitude_to_choose() {
        assert_eq!(show(-10.0, "kN"), "-10 kN");
        assert_eq!(show(-5.0, "N"), "-5 N");
    }

    #[test]
    fn zero_uses_the_smallest_unit() {
        assert_eq!(show(0.0, "kN"), "0 N");
    }

    #[test]
    fn area_is_shown_in_square_millimetres_or_metres() {
        let small = Quantity::from_unit(0.01, "m")
            .unwrap()
            .pow(Quantity::dimensionless(2.0))
            .unwrap();
        assert_eq!(format_quantity(small, UnitSystem::Si), "100 mm\u{00b2}");
        let big = Quantity::from_unit(3.0, "m")
            .unwrap()
            .pow(Quantity::dimensionless(2.0))
            .unwrap();
        assert_eq!(format_quantity(big, UnitSystem::Si), "9 m\u{00b2}");
    }

    #[test]
    fn dimensionless_has_no_unit() {
        assert_eq!(
            format_quantity(Quantity::dimensionless(1.5), UnitSystem::Si),
            "1.5"
        );
    }

    #[test]
    fn unknown_dimensions_show_si_base_units_not_a_debug_dump() {
        // 6 kN / 2 m = 3000 N/m has no display entry.
        let q = Quantity::from_unit(6.0, "kN")
            .unwrap()
            .over(Quantity::from_unit(2.0, "m").unwrap())
            .unwrap();
        assert_eq!(format_quantity(q, UnitSystem::Si), "3000 kg*s^-2");
    }
}
