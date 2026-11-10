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

// ---- the Given / Find block ----

#[test]
fn a_given_block_is_the_find_call_it_stands_for() {
    let out = run(
        &[
            "x := 1",
            "y := 1",
            "r := given\n    x + y == 10\n    x - y == 2\nfind(x, y)",
            "s := find(x, y, x + y == 10, x - y == 2)",
            "r[0]",
            "r[1]",
            "s[0]",
        ],
        None,
    );
    assert!((number(&out, 5) - 6.0).abs() < 1e-7);
    assert!((number(&out, 6) - 4.0).abs() < 1e-7);
    assert_eq!(
        number(&out, 5),
        number(&out, 7),
        "the same numbers, exactly"
    );
}

#[test]
fn a_given_block_may_minimize_and_may_have_no_constraints() {
    let out = run(
        &[
            "a := 1",
            "m := given\nminimize((a - 3)^2, a)",
            "m =",
            "n := given\n    a >= 5\nminimize((a - 3)^2, a)",
            "n =",
        ],
        None,
    );
    assert!((number(&out, 3) - 3.0).abs() < 1e-4);
    assert!((number(&out, 5) - 5.0).abs() < 1e-4);
}

#[test]
fn a_given_blocks_guess_is_a_dependency_and_its_unknown_is_not_a_result_name() {
    // `a` is defined below the block; the block waits for it.
    let out = run(
        &["r := given\n    a ^ 2 == 9\nfind(a)", "a := 2", "r ="],
        None,
    );
    assert!((number(&out, 3) - 3.0).abs() < 1e-7);
}

#[test]
fn a_given_block_without_its_closing_line_is_an_error_that_says_so() {
    for src in [
        "r := given\n    a == 1",
        "r := given\n    a == 1\n    find(a)",
        "r := given\n    a == 1\nsolve(a)",
        "r := given\n    a == 1\nfind(a) + 1",
    ] {
        match opencalc_core::parser::parse(src) {
            opencalc_core::expr::Expr::Error(e) => {
                assert!(
                    e.message.contains("'given' block ends with"),
                    "{src:?}: {}",
                    e.message
                )
            }
            other => panic!("{src:?} parsed as {other:?}"),
        }
    }
}

#[test]
fn a_given_block_prints_and_reads_back_and_has_consistent_spans() {
    let src = "r := given\n    a ^ 2 == 9\n    a > 0\nfind(a)";
    let e = opencalc_core::parser::parse(src);
    assert!(!matches!(e, opencalc_core::expr::Expr::Error(_)), "{e:?}");
    assert_eq!(e.to_string(), src);
    let (_, spans) = opencalc_core::parser::parse_with_spans(src);
    let chars: Vec<char> = src.chars().collect();
    // The block is the value (child 0); its children are the two constraints and
    // then `a` (the closing line's argument), in the order of the text.
    let block = spans.at_path(&[0]).expect("the block");
    assert_eq!(block.kids.len(), 3);
    let text = |s: &opencalc_core::parser::SpanNode| -> String {
        chars[s.inner.start..s.inner.end].iter().collect()
    };
    assert_eq!(text(&block.kids[0]), "a ^ 2 == 9");
    assert_eq!(text(&block.kids[1]), "a > 0");
    assert_eq!(text(&block.kids[2]), "a");
}

#[test]
fn an_error_in_a_constraint_is_located_in_that_line() {
    let out = run(&["a := 1", "r := given\n    a == nothing\nfind(a)"], None);
    // The definition's value is child 0; the block's children are the constraint
    // (0) and then the unknown `a` (1); the bad name is the constraint's right side (1).
    let path = &out.error_paths[&RegionId(2)];
    assert_eq!(path[..3], [0, 0, 1], "{path:?}");
}

// ---- guesses inside the block, and naming the results ----

#[test]
fn a_guess_written_in_the_block_is_local_to_it() {
    // No worksheet `x` or `y` at all: the block carries its own starting values.
    let out = run(
        &[
            "r := given\n    x := 1\n    y := 1\n    x + y == 10\n    x - y == 2\nfind(x, y)",
            "r[0]",
            "r[1]",
        ],
        None,
    );
    assert!((number(&out, 2) - 6.0).abs() < 1e-7);
    assert!((number(&out, 3) - 4.0).abs() < 1e-7);
}

#[test]
fn naming_the_results_defines_each_name_with_its_own_unit() {
    let out = run(
        &[
            "[force; moment] := given\n    force := 1 N\n    moment := 1 N*m\n    force == 5 N\n    moment == 2 * force * 1 m\nfind(force, moment)",
            "force =",
            "moment =",
            "twice := 2 * force",
        ],
        None,
    );
    assert!((number(&out, 2) - 5.0).abs() < 1e-7);
    assert!((number(&out, 3) - 10.0).abs() < 1e-6);
    assert!(
        (number(&out, 4) - 10.0).abs() < 1e-7,
        "usable by other regions"
    );
    // Each keeps its own dimension: a force and a moment.
    let force = out.values[&RegionId(2)]
        .as_ref()
        .unwrap()
        .as_real()
        .unwrap()
        .dim;
    let moment = out.values[&RegionId(3)]
        .as_ref()
        .unwrap()
        .as_real()
        .unwrap()
        .dim;
    assert_ne!(force, moment);
}

#[test]
fn naming_works_for_any_vector_and_checks_the_count() {
    let out = run(
        &[
            "[a; b] := [1 m; 2 s]",
            "a =",
            "b =",
            "[c; d; e] := [1; 2]",
            "c =",
        ],
        None,
    );
    assert_eq!(number(&out, 2), 1.0);
    assert_eq!(number(&out, 3), 2.0);
    let message = out.values[&RegionId(4)].as_ref().unwrap_err().to_string();
    assert!(
        message.contains("3 name(s) need a vector of 3"),
        "{message}"
    );
    assert!(
        out.values[&RegionId(5)].is_err(),
        "a name that failed to define is an error"
    );
}

#[test]
fn naming_a_result_after_a_guess_in_the_worksheet_is_still_circular() {
    // The guess is the worksheet's own `x`, which this region also defines.
    let out = run(&["[x; y] := find(x, y, x + y == 3, x - y == 1)"], None);
    assert!(matches!(
        out.values[&RegionId(1)].as_ref().unwrap_err(),
        opencalc_core::eval::EvalError::Cycle
    ));
}

#[test]
fn guess_lines_and_names_round_trip_and_have_spans() {
    let src = "[x; y] := given\n    x := 1\n    y := 2\n    x + y == 10\nfind(x, y)";
    let e = opencalc_core::parser::parse(src);
    assert!(!matches!(e, opencalc_core::expr::Expr::Error(_)), "{e:?}");
    assert_eq!(e.to_string(), src);
    let (_, spans) = opencalc_core::parser::parse_with_spans(src);
    let chars: Vec<char> = src.chars().collect();
    let text = |s: &opencalc_core::parser::SpanNode| -> String {
        chars[s.inner.start..s.inner.end].iter().collect()
    };
    let block = spans.at_path(&[0]).expect("the block");
    assert_eq!(text(&block.kids[0]), "x := 1");
    assert_eq!(text(&block.kids[2]), "x + y == 10");
    // The same name twice is an error.
    assert!(matches!(
        opencalc_core::parser::parse("[x; x] := [1; 2]"),
        opencalc_core::expr::Expr::Error(_)
    ));
}

#[test]
fn an_error_in_a_guess_or_a_constraint_after_it_is_located() {
    let out = run(
        &["r := given\n    a := nothing\n    a == 1\nfind(a)", "r :="],
        None,
    );
    assert_eq!(
        out.error_paths[&RegionId(1)][..3],
        [0, 0, 0],
        "the guess's value"
    );
    let out = run(&["r := given\n    a := 1\n    a == nothing\nfind(a)"], None);
    assert_eq!(
        out.error_paths[&RegionId(1)][..2],
        [0, 1],
        "the constraint after the guess"
    );
}
