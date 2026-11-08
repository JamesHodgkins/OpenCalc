//! `find`, `minerr`, `minimize` and `maximize` through the whole pipeline.

use std::sync::atomic::AtomicBool;

use opencalc_core::eval::{evaluate_cancellable, EvalError, Output, RegionId};
use opencalc_core::parser::parse;
use opencalc_core::registry::UnitRegistry;

fn run(sources: &[&str], cancel: Option<&AtomicBool>) -> Output {
    let regions: Vec<_> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| (RegionId(i as u64 + 1), parse(s)))
        .collect();
    evaluate_cancellable(&regions, &[], &UnitRegistry::builtin(), cancel)
}

fn number(out: &Output, n: u64) -> f64 {
    out.values[&RegionId(n)]
        .as_ref()
        .unwrap_or_else(|e| panic!("region {n}: {e}"))
        .as_real()
        .unwrap()
        .si
}

fn error(out: &Output, n: u64) -> &EvalError {
    out.values[&RegionId(n)].as_ref().unwrap_err()
}

#[test]
fn the_guess_is_a_worksheet_value_and_the_result_is_a_new_name() {
    let out = run(
        &["a := 1", "r := find(a, a ^ 2 == 9)", "doubled := r * 2"],
        None,
    );
    assert!((number(&out, 2) - 3.0).abs() < 1e-8);
    assert!((number(&out, 3) - 6.0).abs() < 1e-7);
    // The guess region is a dependency, so changing it changes the answer found.
    let out = run(&["a := 0 - 1", "r := find(a, a ^ 2 == 9)"], None);
    assert!(
        (number(&out, 2) + 3.0).abs() < 1e-8,
        "the other root, from the other side"
    );
}

#[test]
fn unknowns_inside_the_call_are_not_the_worksheets_values() {
    // `a` is 100 in the worksheet but is the trial value inside the constraint.
    let out = run(&["a := 100", "r := find(a, a == 5)"], None);
    assert!((number(&out, 2) - 5.0).abs() < 1e-8);
}

#[test]
fn a_constraint_reads_other_worksheet_values_as_dependencies() {
    let out = run(
        &["target := 49", "a := 1", "r := find(a, a ^ 2 == target)"],
        None,
    );
    assert!((number(&out, 3) - 7.0).abs() < 1e-7);
}

#[test]
fn assigning_the_result_to_an_unknowns_own_name_is_a_circular_reference() {
    let out = run(&["a := find(a, a ^ 2 == 4)"], None);
    assert_eq!(error(&out, 1), &EvalError::Cycle);
}

#[test]
fn a_solve_inside_a_function_uses_the_functions_parameters() {
    let out = run(
        &[
            "a := 1",
            "root_of(c) := find(a, a ^ 2 == c)",
            "r := root_of(81)",
        ],
        None,
    );
    assert!((number(&out, 3) - 9.0).abs() < 1e-7);
}

#[test]
fn the_number_of_unknowns_and_constraints_is_limited() {
    let names: Vec<String> = (0..17).map(|i| format!("u{i}")).collect();
    let defs: Vec<String> = names.iter().map(|n| format!("{n} := 1")).collect();
    let call = format!("find({}, u0 == 1)", names.join(", "));
    let mut sources: Vec<&str> = defs.iter().map(String::as_str).collect();
    sources.push(&call);
    let out = run(&sources, None);
    assert!(matches!(error(&out, 18), EvalError::Invalid(m) if m.contains("at most")));

    // The limit of 64 constraints cannot be reached from typed input: a region holds
    // at most 128 symbols, so a call that long does not parse. It is a backstop.
}

#[test]
fn a_raised_cancel_flag_stops_a_solve() {
    let flag = AtomicBool::new(true);
    let out = run(&["a := 1", "r := find(a, a ^ 2 == 2)"], Some(&flag));
    assert_eq!(error(&out, 2), &EvalError::Cancelled);
}

#[test]
fn an_unsolvable_problem_is_an_error_after_bounded_work() {
    let out = run(&["a := 1", "r := find(a, sin(a * 1000) == 2)"], None);
    assert!(matches!(error(&out, 2), EvalError::Invalid(m) if m.contains("no solution")));
    let out = run(&["a := 1", "r := minimize(a, a)"], None);
    assert!(
        matches!(error(&out, 2), EvalError::Invalid(_)),
        "unbounded below"
    );
}

#[test]
fn a_domain_error_at_a_trial_point_does_not_end_the_search() {
    // sqrt of a negative number is complex, which a constraint refuses; the search
    // steps away from it and still finds the root of ln(a) = 1.
    let out = run(&["a := 0.5", "r := find(a, ln(a) == 1)"], None);
    assert!((number(&out, 2) - std::f64::consts::E).abs() < 1e-6);
    let out = run(&["a := 4", "r := find(a, sqrt(a - 1) == 2)"], None);
    assert!((number(&out, 2) - 5.0).abs() < 1e-6);
}

#[test]
fn a_solver_error_points_at_the_failing_part() {
    let out = run(&["a := 1", "r := find(a, a == 1, a + 1 m == 2)"], None);
    assert!(matches!(error(&out, 2), EvalError::Unit(_)));
    assert!(out.error_paths.contains_key(&RegionId(2)));
}

#[test]
fn minerr_always_returns_a_point_and_find_does_not() {
    let sources = [
        "a := 1",
        "best := minerr(a, a ^ 2 == 0 - 1)",
        "sure := find(a, a ^ 2 == 0 - 1)",
    ];
    let out = run(&sources, None);
    assert!(
        number(&out, 2).abs() < 1e-3,
        "the point nearest to a solution: 0"
    );
    assert!(matches!(error(&out, 3), EvalError::Invalid(m) if m.contains("no solution")));
}
