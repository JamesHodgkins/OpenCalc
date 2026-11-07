//! Showing one real number: how many decimals, and in which notation.
//!
//! Display only. The number is never changed, only the text it is shown as.

use opencalc_core::document::{Notation, ResultFormat};

/// Decimals shown when none are chosen (trailing zeros are then dropped).
const DEFAULT_DECIMALS: usize = 3;

/// `x` as text in the given format. With no decimals chosen, up to three are
/// shown and trailing zeros dropped (`10`, `0.333`); with a chosen count that many
/// are always shown (`10.00`).
pub fn number_in(x: f64, format: ResultFormat) -> String {
    if !x.is_finite() {
        return format!("{x}");
    }
    let fixed = format.decimals().map(usize::from);
    let places = fixed.unwrap_or(DEFAULT_DECIMALS);
    let tidy = |t: String| if fixed.is_none() { trim_zeros(&t) } else { t };
    match format.notation() {
        Notation::General => {
            let magnitude = x.abs();
            // Plain digits unless that would be unreadably long or lose the value.
            let low = 10f64.powi(-(places.max(DEFAULT_DECIMALS) as i32));
            if x == 0.0 {
                return tidy(format!("{:.places$}", 0.0));
            }
            if !(low..1e9).contains(&magnitude) {
                return scientific(x, places, fixed.is_none());
            }
            tidy(format!("{x:.places$}"))
        }
        Notation::Scientific => scientific(x, places, fixed.is_none()),
        Notation::Engineering => engineering(x, places, fixed.is_none()),
    }
}

fn trim_zeros(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text.to_owned()
    }
}

/// `1.5e4`: one digit before the point.
fn scientific(x: f64, places: usize, trim: bool) -> String {
    let text = format!("{x:.places$e}");
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
    let mantissa = if trim {
        trim_zeros(mantissa)
    } else {
        mantissa.to_owned()
    };
    format!("{mantissa}e{exponent}")
}

/// `15e3`: the exponent is a multiple of three, so the mantissa is 1 up to 1000.
fn engineering(x: f64, places: usize, trim: bool) -> String {
    if x == 0.0 {
        let zero = format!("{:.places$}", 0.0);
        return format!("{}e0", if trim { trim_zeros(&zero) } else { zero });
    }
    // Work from the shortest exact digits, "1.2345e4", so there is no log10 to
    // get wrong on the boundaries, and no 10^n to overflow.
    let exact = format!("{x:e}");
    let (digits, exponent) = exact.split_once('e').unwrap_or((&exact, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let shift = exponent.rem_euclid(3);
    let mut engineering_exponent = exponent - shift;
    let mut mantissa = format!("{:.places$}", shifted(digits, shift));
    // 999.9996 rounds to 1000.000, which belongs one step up as 1.000e3 more.
    if mantissa
        .parse::<f64>()
        .is_ok_and(|m: f64| m.abs() >= 1000.0)
    {
        engineering_exponent += 3;
        mantissa = format!("{:.places$}", shifted(digits, shift - 3));
    }
    let mantissa = if trim {
        trim_zeros(&mantissa)
    } else {
        mantissa
    };
    format!("{mantissa}e{engineering_exponent}")
}

/// The digits "1.2345" with the point moved `by` places: 1.2345e2 is 123.45.
fn shifted(digits: &str, by: i32) -> f64 {
    format!("{digits}e{by}").parse().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(notation: Notation, decimals: Option<u8>) -> ResultFormat {
        ResultFormat::new(notation, decimals)
    }

    #[test]
    fn the_default_is_up_to_three_decimals_without_trailing_zeros() {
        let d = ResultFormat::default();
        for (x, want) in [
            (10.0, "10"),
            (1.0 / 3.0, "0.333"),
            (-1.5, "-1.5"),
            (0.0, "0"),
            (1.5e-7, "1.5e-7"),
            (2.5e12, "2.5e12"),
            (0.0009, "9e-4"),
        ] {
            assert_eq!(number_in(x, d), want, "{x}");
        }
    }

    #[test]
    fn a_chosen_number_of_decimals_is_always_shown() {
        let f = fmt(Notation::General, Some(2));
        assert_eq!(number_in(10.0, f), "10.00");
        assert_eq!(number_in(1.0 / 3.0, f), "0.33");
        assert_eq!(number_in(2.0 / 3.0, f), "0.67");
        assert_eq!(number_in(0.0, f), "0.00");
        assert_eq!(number_in(-1234.5678, f), "-1234.57");
        assert_eq!(number_in(7.5, fmt(Notation::General, Some(0))), "8");
        assert_eq!(
            number_in(1.0 / 3.0, fmt(Notation::General, Some(6))),
            "0.333333"
        );
        // More decimals let a small number stay plain rather than turn scientific.
        assert_eq!(
            number_in(0.0000125, fmt(Notation::General, Some(7))),
            "0.0000125"
        );
        assert_eq!(
            number_in(0.0000125, fmt(Notation::General, Some(2))),
            "1.25e-5"
        );
    }

    #[test]
    fn scientific_has_one_digit_before_the_point() {
        let s = fmt(Notation::Scientific, None);
        assert_eq!(number_in(15000.0, s), "1.5e4");
        assert_eq!(number_in(0.00123, s), "1.23e-3");
        assert_eq!(number_in(-2.0, s), "-2e0");
        assert_eq!(number_in(0.0, s), "0e0");
        assert_eq!(
            number_in(15000.0, fmt(Notation::Scientific, Some(3))),
            "1.500e4"
        );
        assert_eq!(
            number_in(9999.6, fmt(Notation::Scientific, Some(1))),
            "1.0e4"
        );
    }

    #[test]
    fn engineering_exponents_are_multiples_of_three() {
        let e = fmt(Notation::Engineering, None);
        for (x, want) in [
            (15000.0, "15e3"),
            (1.0, "1e0"),
            (999.0, "999e0"),
            (1000.0, "1e3"),
            (0.00123, "1.23e-3"),
            (0.000_000_47, "470e-9"),
            (-12_345_678.0, "-12.346e6"),
            (0.0, "0e0"),
        ] {
            assert_eq!(number_in(x, e), want, "{x}");
        }
        // Rounding up to 1000 moves to the next step instead of showing 1000e3.
        assert_eq!(
            number_in(999_999.0, fmt(Notation::Engineering, Some(1))),
            "1.0e6"
        );
        assert_eq!(
            number_in(15000.0, fmt(Notation::Engineering, Some(2))),
            "15.00e3"
        );
    }

    #[test]
    fn extreme_and_odd_numbers_do_not_panic() {
        for notation in [
            Notation::General,
            Notation::Scientific,
            Notation::Engineering,
        ] {
            for decimals in [None, Some(0), Some(15)] {
                for x in [
                    f64::MAX,
                    f64::MIN_POSITIVE,
                    5e-324,
                    -5e-324,
                    f64::NAN,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    1e308,
                ] {
                    let _ = number_in(x, fmt(notation, decimals));
                }
            }
        }
        assert_eq!(
            number_in(5e-324, fmt(Notation::Engineering, None)),
            "5e-324"
        );
    }
}
