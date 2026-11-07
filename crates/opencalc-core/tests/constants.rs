//! Named constants in worksheets.

use opencalc_core::eval::{evaluate, RegionId};
use opencalc_core::parser::parse;

/// The value of the last region, as the evaluator's debug text.
fn last(sources: &[&str]) -> Result<f64, String> {
    let regions: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| (RegionId(i as u64 + 1), parse(s)))
        .collect();
    let out = evaluate(&regions);
    let id = regions.last().unwrap().0;
    match &out.values[&id] {
        Ok(v) => match v {
            opencalc_core::value::Value::Scalar(s) => Ok(s.z.re),
            other => Err(format!("{other:?}")),
        },
        Err(e) => Err(e.to_string()),
    }
}

#[test]
fn pi_and_e_work_in_expressions() {
    assert_eq!(last(&["pi"]).unwrap(), std::f64::consts::PI);
    assert_eq!(last(&["\u{3c0}"]).unwrap(), std::f64::consts::PI);
    assert_eq!(last(&["e"]).unwrap(), std::f64::consts::E);
    let area = last(&["r := 2 m", "A := pi * r^2"]).unwrap();
    assert!((area - 4.0 * std::f64::consts::PI).abs() < 1e-12);
    assert!((last(&["e^1"]).unwrap() - std::f64::consts::E).abs() < 1e-12);
    // A number in scientific notation is still a number.
    assert_eq!(last(&["1e3"]).unwrap(), 1000.0);
}

#[test]
fn physical_constants_carry_their_units() {
    let force = last(&["m := 2 kg", "W := m * g_n"]).unwrap();
    assert!((force - 19.6133).abs() < 1e-9, "2 kg x g_n is 19.6133 N");
    assert_eq!(last(&["c_0"]).unwrap(), 299_792_458.0);
}

#[test]
fn a_definition_in_the_worksheet_wins_over_a_constant() {
    assert_eq!(last(&["pi := 3", "pi"]).unwrap(), 3.0);
    assert_eq!(last(&["e := 5 m", "e"]).unwrap(), 5.0);
    // And a unit wins too: `g` is grams, not gravity.
    assert!(last(&["g"]).is_ok());
    assert!((last(&["g"]).unwrap() - 0.001).abs() < 1e-15);
}

#[test]
fn an_unknown_name_is_still_undefined() {
    assert!(last(&["zzz"]).unwrap_err().contains("zzz"));
}
