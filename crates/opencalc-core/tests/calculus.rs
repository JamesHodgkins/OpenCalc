//! Calculus and iteration forms through the whole pipeline: parsing, the
//! dependency graph, the step budget and cancellation.

use std::sync::atomic::{AtomicBool, Ordering};

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
fn a_bound_variable_is_not_a_dependency_of_the_worksheet() {
    // If `x` were read from the worksheet, this would be a circular reference.
    let out = run(&["x := integrate(x, x, 0, 1)"], None);
    assert!((number(&out, 1) - 0.5).abs() < 1e-12);
    // Nor does a region that defines `k` change a sum over `k`.
    let out = run(&["k := 1000", "total := sum_over(k, k, 1, 4)"], None);
    assert_eq!(number(&out, 2), 10.0);
}

#[test]
fn the_bounds_are_read_from_the_worksheet() {
    let out = run(&["top := 4", "a := sum_over(k, k, 1, top)"], None);
    assert_eq!(number(&out, 2), 10.0);
    // A bound that reads the bound variable's name reads the worksheet's, not the loop's.
    let out = run(&["k := 3", "b := sum_over(k, k, 1, k)"], None);
    assert_eq!(number(&out, 2), 6.0);
}

#[test]
fn an_inner_variable_shadows_an_outer_one_of_the_same_name() {
    // The inner x is the inner integration variable: it integrates 1 over [0, 1]
    // for each outer x, then the outer integrates that constant over [0, 2].
    let out = run(&["integrate(integrate(x, x, 0, 1), x, 0, 2)"], None);
    assert!((number(&out, 1) - 1.0).abs() < 1e-10);
}

#[test]
fn a_function_body_sees_the_bound_variable_only_through_its_argument() {
    let out = run(
        &["g(t) := t * 2", "total := integrate(g(x), x, 0, 3)"],
        None,
    );
    assert!((number(&out, 2) - 9.0).abs() < 1e-9);
    // The function's own body cannot see `x`: it is the caller's bound variable.
    let out = run(
        &["h(t) := t + x", "total := integrate(h(x), x, 0, 1)"],
        None,
    );
    assert!(matches!(error(&out, 2), EvalError::InFunction { .. }));
}

#[test]
fn a_form_inside_a_function_uses_the_functions_parameters() {
    let out = run(
        &["area(top) := integrate(x, x, 0, top)", "r := area(4)"],
        None,
    );
    assert!((number(&out, 2) - 8.0).abs() < 1e-9);
}

#[test]
fn the_step_budget_stops_a_runaway_sum() {
    let out = run(&["sum_over(1, k, 1, 5000000)"], None);
    assert!(matches!(error(&out, 1), EvalError::OutOfBudget(_)));
}

#[test]
fn nested_forms_share_one_budget() {
    // 2000 x 2000 inner evaluations is far over the budget.
    let out = run(&["sum_over(sum_over(1, j, 1, 2000), k, 1, 2000)"], None);
    assert!(matches!(error(&out, 1), EvalError::OutOfBudget(_)));
}

#[test]
fn each_region_has_its_own_budget() {
    let big = "sum_over(1, k, 1, 400000)";
    let out = run(&[big, big, big], None);
    for n in 1..=3 {
        assert_eq!(number(&out, n), 400_000.0);
    }
}

#[test]
fn a_raised_flag_cancels_repeated_work_but_not_plain_arithmetic() {
    let flag = AtomicBool::new(true);
    let out = run(&["2 + 3", "sum_over(k, k, 1, 10)"], Some(&flag));
    assert_eq!(number(&out, 1), 5.0);
    assert_eq!(error(&out, 2), &EvalError::Cancelled);
    flag.store(false, Ordering::Relaxed);
    let out = run(&["sum_over(k, k, 1, 10)"], Some(&flag));
    assert_eq!(number(&out, 1), 55.0);
}

#[test]
fn a_pathological_integrand_stops_within_the_budget() {
    // Wildly oscillating near 0: the integral is refused, in bounded work.
    let out = run(&["integrate(sin(1 / x), x, 0, 1)"], None);
    assert!(error_or_number(&out, 1));
}

fn error_or_number(out: &Output, n: u64) -> bool {
    // Either a converged value or a clear error: never a panic or a hang.
    out.values[&RegionId(n)].is_ok()
        || matches!(error(out, n), EvalError::Invalid(_) | EvalError::NotFinite)
}

#[test]
fn deeply_nested_forms_do_not_overflow_the_stack() {
    let mut src = String::from("1");
    for _ in 0..24 {
        src = format!("integrate({src}, x, 0, 1)");
    }
    let out = run(&[&src], None);
    // Whether it finishes or runs out of budget, it returns.
    assert!(out.values.contains_key(&RegionId(1)));
}
