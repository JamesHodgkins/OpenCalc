//! `odesolve`, `col` and `row` through the whole pipeline.

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
fn the_time_and_state_names_are_local_to_the_call() {
    // If `y` or `t` were read from the worksheet this would be a cycle or use 100.
    let out = run(
        &["y := 100", "t := 200", "r := odesolve(y, t, y, 0, 1, 1)"],
        None,
    );
    assert!((number(&out, 3) - std::f64::consts::E).abs() < 1e-7);
    let out = run(&["y := odesolve(y, t, y, 0, 1, 1)"], None);
    assert!(
        (number(&out, 1) - std::f64::consts::E).abs() < 1e-7,
        "no false cycle"
    );
}

#[test]
fn other_worksheet_values_are_dependencies_of_the_equation_and_the_limits() {
    let out = run(
        &[
            "rate := 2",
            "end := 1",
            "r := odesolve(rate * y, t, y, 0, 1, end)",
        ],
        None,
    );
    assert!((number(&out, 3) - 2f64.exp()).abs() < 1e-7);
    // Changing the rate changes the answer: it really is read.
    let out = run(
        &[
            "rate := 3",
            "end := 1",
            "r := odesolve(rate * y, t, y, 0, 1, end)",
        ],
        None,
    );
    assert!((number(&out, 3) - 3f64.exp()).abs() < 1e-6);
}

#[test]
fn the_equation_may_call_user_functions() {
    let out = run(
        &[
            "drag(v) := 0.5 * v",
            "r := odesolve(-drag(y), t, y, 0, 8, 2)",
        ],
        None,
    );
    assert!((number(&out, 2) - 8.0 * (-1f64).exp()).abs() < 1e-7);
}

#[test]
fn a_solve_inside_a_function_uses_the_functions_parameters() {
    let out = run(
        &[
            "after(k, time) := odesolve(k * y, t, y, 0, 1, time)",
            "r := after(2, 0.5)",
        ],
        None,
    );
    assert!((number(&out, 2) - 1f64.exp()).abs() < 1e-7);
}

#[test]
fn col_and_row_count_from_origin_like_indexing() {
    let table = "table := odesolve(y, t, y, 0, 1, 1, 2)";
    // ORIGIN is 0 by default: column 1 is the state.
    let out = run(&[table, "last := col(table, 1)[2]"], None);
    assert!((number(&out, 2) - std::f64::consts::E).abs() < 1e-7);
    // With ORIGIN := 1 the first column is number 1 and the state is number 2.
    let out = run(
        &[
            "ORIGIN := 1",
            table,
            "first_time := col(table, 1)[1]",
            "last_state := col(table, 2)[3]",
        ],
        None,
    );
    assert_eq!(number(&out, 3), 0.0);
    assert!((number(&out, 4) - std::f64::consts::E).abs() < 1e-7);
    // Column 0 no longer exists once ORIGIN is 1.
    let out = run(&["ORIGIN := 1", table, "gone := col(table, 0)"], None);
    assert!(matches!(error(&out, 3), EvalError::Invalid(_)));
}

#[test]
fn a_stiff_equation_ends_in_an_error_quickly() {
    let out = run(&["r := odesolve(-10000000 * y, t, y, 0, 1, 1)"], None);
    assert!(matches!(error(&out, 1), EvalError::Invalid(m) if m.contains("stiff")));
}

#[test]
fn the_step_budget_is_shared_with_the_equation_it_runs() {
    // A sum inside the equation makes each evaluation expensive; the region's budget
    // runs out long before the solution is found.
    let out = run(
        &["r := odesolve(sum_over(1, k, 1, 200000) * y / 200000, t, y, 0, 1, 1)"],
        None,
    );
    assert!(
        matches!(error(&out, 1), EvalError::OutOfBudget(_)),
        "{:?}",
        error(&out, 1)
    );
}

#[test]
fn a_raised_cancel_flag_stops_a_solve() {
    let flag = AtomicBool::new(true);
    let out = run(&["r := odesolve(y, t, y, 0, 1, 1)"], Some(&flag));
    assert_eq!(error(&out, 1), &EvalError::Cancelled);
}

#[test]
fn a_domain_error_inside_the_equation_points_at_the_equation() {
    let out = run(&["r := odesolve(undefined_name * y, t, y, 0, 1, 1)"], None);
    assert!(matches!(error(&out, 1), EvalError::Undefined(_)));
    assert!(out.error_paths.contains_key(&RegionId(1)));
}

#[test]
fn nesting_a_solve_in_a_solve_works_and_cannot_overflow_the_stack() {
    // The outer equation integrates the inner one for each stage.
    let out = run(
        &["r := odesolve(odesolve(-y, s, y, 0, 1, 1) * y, t, y, 0, 1, 1)"],
        None,
    );
    // The inner `y` is its own variable: odesolve(-y, ..., 1) = e^-1, so y' = e^-1 y.
    assert!((number(&out, 1) - (-1f64).exp().exp()).abs() < 1e-6);
    let mut src = String::from("y");
    for _ in 0..12 {
        src = format!("odesolve({src}, t, y, 0, 1, 1)");
    }
    let out = run(&[&src], None);
    assert!(
        out.values.contains_key(&RegionId(1)),
        "returns, whatever the answer"
    );
}
