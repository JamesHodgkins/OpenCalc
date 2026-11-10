//! Programs: `program`, local variables, branches, loops, early return.

use std::sync::atomic::{AtomicBool, Ordering};

use opencalc_core::eval::{evaluate_cancellable, EvalError, Output, RegionId};
use opencalc_core::expr::Expr;
use opencalc_core::parser::{parse, parse_with_spans};
use opencalc_core::registry::UnitRegistry;

fn run(sources: &[&str]) -> Output {
    run_with(sources, None)
}

fn run_with(sources: &[&str], cancel: Option<&AtomicBool>) -> Output {
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

fn failure(out: &Output, n: u64) -> String {
    out.values[&RegionId(n)].as_ref().unwrap_err().to_string()
}

#[test]
fn a_program_sums_with_a_loop_and_returns_it() {
    let out = run(&[
        "total(n) := program\n    s ← 0\n    for i in 1..n\n        s ← s + i\n    return s",
        "total(10)",
    ]);
    assert_eq!(number(&out, 2), 55.0);
}

#[test]
fn the_last_line_is_the_value_when_nothing_returns() {
    let out = run(&["f(x) := program\n    y ← x * 2\n    y + 1", "f(5)"]);
    assert_eq!(number(&out, 2), 11.0);
}

#[test]
fn a_program_can_be_a_plain_definition_and_keeps_units() {
    let out = run(&[
        "w := program\n    a ← 3 m\n    b ← 4 m\n    sqrt(a^2 + b^2)",
        "w =",
    ]);
    assert!((number(&out, 2) - 5.0).abs() < 1e-12);
}

#[test]
fn if_else_and_else_if() {
    let src = "sign(x) := program\n    if x > 0\n        return 1\n    else if x < 0\n        return 0 - 1\n    else\n        return 0";
    let out = run(&[src, "sign(7)", "sign(0 - 3)", "sign(0)"]);
    assert_eq!(
        (number(&out, 2), number(&out, 3), number(&out, 4)),
        (1.0, -1.0, 0.0)
    );
}

#[test]
fn the_if_function_still_works_on_a_line_of_its_own() {
    let out = run(&[
        "f(x) := program\n    y ← 1\n    if(x > 0, 10, 20)",
        "f(1)",
        "f(0 - 1)",
    ]);
    assert_eq!((number(&out, 2), number(&out, 3)), (10.0, 20.0));
}

#[test]
fn while_break_and_continue() {
    let src = "f := program\n    n ← 0\n    t ← 0\n    while 1\n        n ← n + 1\n        if n > 10\n            break\n        if n - 2 * floor(n / 2) == 1\n            continue\n        t ← t + n\n    return t";
    let out = run(&[src, "f ="]);
    assert_eq!(number(&out, 2), 30.0); // 2 + 4 + 6 + 8 + 10
}

#[test]
fn return_leaves_nested_loops() {
    let src = "f := program\n    for i in 1..5\n        for j in 1..5\n            if i * j == 12\n                return i * 10 + j\n    return 0";
    let out = run(&[src, "f ="]);
    assert_eq!(number(&out, 2), 34.0);
}

#[test]
fn try_runs_the_handler_when_the_body_fails() {
    let src = "f(x) := program\n    try\n        r ← 1 / (x - x)\n    on error\n        r ← 0 - 1\n    return r";
    let out = run(&[src, "f(2)"]);
    assert_eq!(number(&out, 2), -1.0);
    let ok =
        "g(x) := program\n    try\n        r ← x + 1\n    on error\n        r ← 0\n    return r";
    assert_eq!(number(&run(&[ok, "g(2)"]), 2), 3.0);
}

#[test]
fn locals_are_the_programs_own_and_never_leak() {
    let out = run(&[
        "s := 100",
        "f := program\n    s ← 5\n    s + 1",
        "f =",
        "s =",
    ]);
    assert_eq!(number(&out, 3), 6.0);
    assert_eq!(number(&out, 4), 100.0);
    // A name used before the program gives it a value is an error, not the worksheet's.
    let out = run(&[
        "s := 100",
        "f := program\n    t ← s\n    s ← 5\n    t",
        "f =",
    ]);
    assert!(failure(&out, 3).contains("before it is given a value"));
}

#[test]
fn what_a_program_reads_is_a_dependency_and_what_it_assigns_is_not() {
    // `rate` is defined after the program that uses it, and `r` is local to the
    // function so the worksheet's `r` (which uses the function) is no cycle.
    let out = run(&[
        "r := area(2)",
        "area(x) := program\n    r ← x * x * rate\n    r",
        "rate := 3",
    ]);
    assert_eq!(number(&out, 1), 12.0);
}

#[test]
fn a_runaway_loop_ends_on_the_budget() {
    let out = run(&[
        "f := program\n    while 1\n        x ← 1\n    return 0",
        "f =",
    ]);
    assert!(matches!(
        out.values[&RegionId(2)].as_ref().unwrap_err(),
        EvalError::OutOfBudget(_)
    ));
}

#[test]
fn a_cancelled_loop_stops_and_try_does_not_swallow_it() {
    let flag = AtomicBool::new(true);
    let src = "f := program\n    try\n        while 1\n            x ← 1\n    on error\n        return 5\n    return 0";
    let out = run_with(&[src, "f ="], Some(&flag));
    flag.store(false, Ordering::Relaxed);
    assert!(matches!(
        out.values[&RegionId(2)].as_ref().unwrap_err(),
        EvalError::Cancelled
    ));
}

#[test]
fn a_try_does_not_swallow_the_budget_either() {
    let src = "f := program\n    try\n        while 1\n            x ← 1\n    on error\n        return 5\n    return 0";
    let out = run(&[src, "f ="]);
    assert!(matches!(
        out.values[&RegionId(2)].as_ref().unwrap_err(),
        EvalError::OutOfBudget(_)
    ));
}

#[test]
fn programs_that_give_no_value_or_misplace_break_are_errors() {
    let out = run(&["f := program\n    x ← 1", "f ="]);
    assert!(failure(&out, 2).contains("gives no value"));
    let out = run(&["f := program\n    break\n    1", "f ="]);
    assert!(failure(&out, 2).contains("inside a loop"));
}

#[test]
fn for_goes_through_vectors_and_keeps_each_unit() {
    let out = run(&[
        "v := [1 m; 2 m; 3 m]",
        "f := program\n    t ← 0 m\n    for x in v\n        t ← t + x\n    t",
        "f =",
    ]);
    assert_eq!(number(&out, 3), 6.0);
}

#[test]
fn bad_syntax_is_an_error_that_points_at_the_line() {
    for (src, part) in [
        (
            "f := program\n    x ← 1\n      y ← 2\n    y",
            "indented more",
        ),
        ("f := program\nx ← 1", "indented line"),
        ("f := program\n    else\n        1", "'else' has no 'if'"),
        ("f := program\n    try\n        1", "'on error'"),
        (
            "f := program\n    for 1 in 2\n        3",
            "name of a variable",
        ),
        ("f := program\n    1 ← 2\n    3", "only a name"),
    ] {
        let message = match parse(src) {
            Expr::Error(e) => e.message,
            other => panic!("{src:?} parsed as {other:?}"),
        };
        assert!(message.contains(part), "{src:?}: {message}");
    }
}

#[test]
fn printing_a_program_and_reading_it_back_gives_the_same_tree() {
    let sources = [
        "f(x, n := 2) := program\n    s ← 0\n    for i in 1..x\n        if i > n\n            s ← s + i\n        else if i == n\n            continue\n        else\n            s ← s - 1\n    while s > 100\n        s ← s / 2\n    try\n        t ← 1 / s\n    on error\n        t ← 0\n    return s + t",
        "g := program\n    1 + 2",
    ];
    for src in sources {
        let e = parse(src);
        assert!(!matches!(e, Expr::Error(_)), "{src}: {e:?}");
        assert_eq!(e.to_string(), src, "printing gives the source back");
        assert_eq!(parse(&e.to_string()), e);
    }
}

#[test]
fn every_node_has_a_span_and_the_tree_matches() {
    let src = "f(x) := program\n    s ← 0\n    for i in 1..x\n        s ← s + i\n    if s > 3\n        return s\n    else\n        return 0";
    let (e, spans) = parse_with_spans(src);
    assert!(!matches!(e, Expr::Error(_)), "{e:?}");
    let chars: Vec<char> = src.chars().collect();
    // Path [0] is the body (the program), [0, 0] its first statement.
    let first = spans.at_path(&[0, 0]).expect("first statement");
    let text: String = chars[first.inner.start..first.inner.end].iter().collect();
    assert_eq!(text, "s ← 0");
}

#[test]
fn an_error_inside_a_program_is_located_in_the_statement() {
    let out = run(&[
        "f := program\n    a ← 1\n    b ← a + undefined_name\n    b",
        "f =",
    ]);
    // The definition is region 1; its value is child 0, the program, whose second
    // line (index 1) holds the assignment whose value (child 0) has the bad name
    // as its right operand (child 1).
    assert_eq!(out.error_paths[&RegionId(1)], vec![0, 1, 0, 1]);
}

#[test]
fn programs_are_limited_in_nesting_and_each_line_in_length() {
    let mut deep = String::from("f := program");
    for level in 1..=14 {
        deep.push_str(&format!("\n{}if 1", "    ".repeat(level)));
    }
    deep.push_str(&format!("\n{}1", "    ".repeat(15)));
    match parse(&deep) {
        Expr::Error(e) => assert!(e.message.contains("nested too deeply"), "{}", e.message),
        other => panic!("{other:?}"),
    }
    let long_line = format!("f := program\n    {}", "1+".repeat(80) + "1");
    assert!(matches!(parse(&long_line), Expr::Error(_)));
}

#[test]
fn several_values_come_back_as_a_vector() {
    let out = run(&[
        "stats(a, b) := program\n    return [a + b; a * b]",
        "r := stats(3, 4)",
        "r[0]",
        "r[1]",
    ]);
    assert_eq!((number(&out, 3), number(&out, 4)), (7.0, 12.0));
}

#[test]
fn an_element_can_be_set_and_a_vector_grows_to_fit() {
    let src = "squares(n) := program\n    for i in 0..n\n        v[i] \u{2190} i * i\n    return v";
    let out = run(&[src, "r := squares(4)", "r[4]", "r[2]", "rows(r)"]);
    assert_eq!((number(&out, 3), number(&out, 4)), (16.0, 4.0));
    assert_eq!(number(&out, 5), 5.0);
}

#[test]
fn elements_keep_their_units_and_new_ones_start_at_zero_of_that_unit() {
    let src = "f := program\n    v[2] \u{2190} 3 m\n    return v";
    let out = run(&[src, "r := f", "r[2]", "r[0]"]);
    assert_eq!(number(&out, 3), 3.0);
    assert_eq!(number(&out, 4), 0.0);
}

#[test]
fn a_matrix_is_filled_by_row_and_column() {
    let src = "f := program\n    for i in 0..2\n        for j in 0..2\n            m[i, j] \u{2190} 10 * i + j\n    return m";
    let out = run(&[src, "r := f", "r[2, 1]", "r[1, 2]", "rows(r)", "cols(r)"]);
    assert_eq!((number(&out, 3), number(&out, 4)), (21.0, 12.0));
    assert_eq!((number(&out, 5), number(&out, 6)), (3.0, 3.0));
}

#[test]
fn setting_an_element_of_an_existing_vector_changes_only_that_element() {
    let src = "f := program\n    v \u{2190} [1; 2; 3]\n    v[1] \u{2190} 20\n    return v[0] + v[1] + v[2]";
    let out = run(&[src, "f ="]);
    assert_eq!(number(&out, 2), 24.0);
}

#[test]
fn bad_element_assignments_are_errors() {
    let out = run(&["f := program\n    v[0.5] \u{2190} 1\n    1", "f ="]);
    assert!(failure(&out, 2).contains("whole number"));
    let out = run(&[
        "f := program\n    m \u{2190} [1, 2; 3, 4]\n    m[1] \u{2190} 1\n    1",
        "f =",
    ]);
    assert!(failure(&out, 2).contains("row and a column"));
    let out = run(&["f := program\n    v[0] \u{2190} [1; 2]\n    1", "f ="]);
    assert!(failure(&out, 2).contains("single number"));
    match parse("f := program\n    f(1) \u{2190} 2\n    1") {
        Expr::Error(e) => assert!(e.message.contains("one element"), "{}", e.message),
        other => panic!("{other:?}"),
    }
}

#[test]
fn element_assignment_round_trips_and_has_spans() {
    let src = "g := program\n    m[1, 2] \u{2190} 5\n    m";
    let e = parse(src);
    assert!(!matches!(e, Expr::Error(_)), "{e:?}");
    assert_eq!(e.to_string(), src);
    let (_, spans) = parse_with_spans(src);
    let line = spans.at_path(&[0, 0]).expect("the assignment");
    let chars: Vec<char> = src.chars().collect();
    let text: String = chars[line.inner.start..line.inner.end].iter().collect();
    assert_eq!(text, "m[1, 2] \u{2190} 5");
    // Its children are the two indices and the value.
    assert_eq!(line.kids.len(), 3);
}

#[test]
fn a_function_may_call_itself() {
    let out = run(&[
        "fact(n) := if(n <= 1, 1, n * fact(n - 1))",
        "fact(10)",
        "fib(n) := if(n < 2, n, fib(n - 1) + fib(n - 2))",
        "fib(15)",
    ]);
    assert_eq!(number(&out, 2), 3_628_800.0);
    assert_eq!(number(&out, 4), 610.0);
}

#[test]
fn functions_may_call_each_other_in_a_cycle() {
    let out = run(&[
        "even(n) := if(n == 0, 1, odd(n - 1))",
        "odd(n) := if(n == 0, 0, even(n - 1))",
        "even(10)",
        "even(7)",
    ]);
    assert_eq!((number(&out, 3), number(&out, 4)), (1.0, 0.0));
}

#[test]
fn recursion_in_a_program() {
    let src = "gcd(a, b) := program\n    if b == 0\n        return a\n    return gcd(b, a - b * floor(a / b))";
    let out = run(&[src, "gcd(48, 18)"]);
    assert_eq!(number(&out, 2), 6.0);
}

#[test]
fn a_cycle_through_a_variable_is_still_circular() {
    let out = run(&["x := f(1)", "f(n) := n + x", "x ="]);
    assert!(matches!(
        out.values[&RegionId(3)].as_ref().unwrap_err(),
        EvalError::Cycle
    ));
}

#[test]
fn recursion_that_never_ends_stops_with_an_error_not_a_crash() {
    let out = run(&["f(n) := f(n + 1)", "f(0)"]);
    assert!(
        failure(&out, 2).contains("too deeply"),
        "{}",
        failure(&out, 2)
    );
}

#[test]
fn recursion_that_does_too_much_work_stops_on_the_budget() {
    // fib(32) alone is about seven million calls.
    let out = run(&["fib(n) := if(n < 2, n, fib(n - 1) + fib(n - 2))", "fib(32)"]);
    assert!(matches!(
        out.values[&RegionId(2)].as_ref().unwrap_err(),
        EvalError::InFunction { .. } | EvalError::OutOfBudget(_)
    ));
}

/// The worst shape a recursive function can have at every level: blocks nested as
/// deep as allowed, and the longest line allowed round the recursive call.
#[test]
fn the_deepest_recursion_of_the_deepest_functions_fits_in_the_evaluation_stack() {
    let minuses = "-".repeat(opencalc_core::parser::MAX_SYMBOLS - 20);
    let mut src = String::from("r(n) := program");
    for level in 1..=10 {
        src.push_str(&format!("\n{}if n > 0", "    ".repeat(level)));
    }
    src.push_str(&format!(
        "\n{}return {minuses}r(n - 1)\n    return 0",
        "    ".repeat(11)
    ));
    let e = parse(&src);
    assert!(!matches!(e, Expr::Error(_)), "{e:?}");
    let regions = vec![
        (RegionId(1), e),
        (RegionId(2), parse("r(60)")),
        (RegionId(3), parse("r(100000)")),
    ];
    let out = std::thread::Builder::new()
        .stack_size(opencalc_core::worker::EVAL_STACK / 2)
        .spawn(move || evaluate_cancellable(&regions, &[], &UnitRegistry::builtin(), None))
        .unwrap()
        .join()
        .expect("did not overflow the stack");
    // An even number of minus signs: the value is r(0) = 0 whatever the depth.
    assert_eq!(number(&out, 2), 0.0);
    assert!(failure(&out, 3).contains("too deeply"));
}

fn text(out: &Output, n: u64) -> String {
    match out.values[&RegionId(n)].as_ref().unwrap() {
        opencalc_core::value::Value::Text(s) => s.clone(),
        other => panic!("region {n} is {other:?}, not text"),
    }
}

#[test]
fn text_is_joined_with_plus_and_compared_with_equals() {
    let out = run(&[
        "greeting := \"Hello, \" + \"world\"",
        "greeting =",
        "same := greeting == \"Hello, world\"",
        "different := greeting != \"Hello, world\"",
        "same =",
        "different =",
    ]);
    assert_eq!(text(&out, 2), "Hello, world");
    assert_eq!((number(&out, 5), number(&out, 6)), (1.0, 0.0));
}

#[test]
fn text_does_not_do_arithmetic_or_ordering() {
    for src in [
        "\"a\" - \"b\"",
        "\"a\" * 2",
        "\"a\" + 1",
        "\"a\" < \"b\"",
        "\"a\" == 1",
        "sqrt(\"a\")",
        "[\"a\"]",
    ] {
        let out = run(&[src]);
        assert!(out.values[&RegionId(1)].is_err(), "{src}");
    }
}

#[test]
fn text_must_be_closed_and_not_too_long() {
    for src in ["\"open", &format!("\"{}\"", "x".repeat(300))] {
        assert!(matches!(parse(src), Expr::Error(_)), "{src}");
    }
    assert_eq!(parse("\"a b\"").to_string(), "\"a b\"");
}

#[test]
fn on_error_can_name_the_message() {
    let src = "f(x) := program\n    try\n        r ← 1 / (x - x)\n    on error e\n        return \"failed: \" + e\n    return \"fine\"";
    let out = run(&[src, "f(2)"]);
    let message = text(&out, 2);
    assert!(message.starts_with("failed: "), "{message}");
    assert!(message.len() > "failed: ".len());
    // The name is a local of the program, only given a value if there was an error.
    let ok = "g := program\n    try\n        r ← 1\n    on error e\n        r ← 2\n    e";
    assert!(failure(&run(&[ok, "g ="]), 2).contains("before it is given a value"));
    // The message can be compared.
    let cmp = "h := program\n    try\n        r ← missing_name\n    on error e\n        return e == \"x\"\n    return 2";
    assert_eq!(number(&run(&[cmp, "h ="]), 2), 0.0);
}

#[test]
fn on_error_with_a_name_round_trips() {
    let src = "f := program\n    try\n        x ← 1\n    on error msg\n        x ← 2\n    x";
    let e = parse(src);
    assert!(!matches!(e, Expr::Error(_)), "{e:?}");
    assert_eq!(e.to_string(), src);
}
