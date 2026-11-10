//! The lenient parse (editor display tree): it never fails, agrees with `parse` on
//! complete input, keeps spans consistent, and only ever fills in what is missing.

mod common;

use common::{known_sources, problem};
use opencalc_core::expr::Expr;
use opencalc_core::parser::{parse, parse_lenient};
use proptest::prelude::*;

/// Parse leniently and check the spans; return the tree.
fn checked(src: &str) -> Expr {
    let chars: Vec<char> = src.chars().collect();
    let (expr, spans) = parse_lenient(src);
    assert!(
        !matches!(expr, Expr::Error(_)),
        "{src:?} gave an error node"
    );
    if let Some(p) = problem(&expr, &spans, &chars) {
        panic!("{src:?}: {p}");
    }
    expr
}

fn holes(e: &Expr) -> usize {
    usize::from(matches!(e, Expr::Hole)) + e.children().into_iter().map(holes).sum::<usize>()
}

fn has_garbage(e: &Expr) -> bool {
    matches!(e, Expr::Garbage { .. }) || e.children().into_iter().any(has_garbage)
}

#[test]
fn complete_input_gives_the_same_tree_as_the_strict_parse() {
    let mut same = 0;
    for src in known_sources() {
        let strict = parse(&src);
        let lenient = checked(&src);
        if !matches!(strict, Expr::Error(_)) {
            assert_eq!(strict, lenient, "{src:?}");
            same += 1;
        }
    }
    assert!(same > 350, "only {same} sources compared");
}

#[test]
fn every_prefix_of_every_known_source_parses_to_something() {
    for src in known_sources() {
        let chars: Vec<char> = src.chars().collect();
        for n in 0..=chars.len() {
            let prefix: String = chars[..n].iter().collect();
            checked(&prefix);
        }
    }
}

#[test]
fn a_prefix_of_a_valid_source_never_invents_extra_structure() {
    // Whatever the prefix of a valid source turns into, a prefix that is itself
    // valid keeps exactly the strict tree: filling in never reorders what is there.
    for src in known_sources() {
        if matches!(parse(&src), Expr::Error(_)) {
            continue;
        }
        let chars: Vec<char> = src.chars().collect();
        for n in 0..=chars.len() {
            let prefix: String = chars[..n].iter().collect();
            let strict = parse(&prefix);
            if !matches!(strict, Expr::Error(_)) {
                assert_eq!(strict, checked(&prefix), "{prefix:?}");
            }
        }
    }
}

#[test]
fn what_is_missing_becomes_a_hole() {
    let cases = [
        ("", 1),
        ("1 +", 1),
        ("* 2", 1),
        ("x :=", 1),
        ("(1 +", 1),
        ("f(1,", 1),
        ("-", 1),
        ("[1, 2; 3", 1),
        ("0, 2", 1),
        ("a +  * b", 1),
        ("1 + 2", 0),
    ];
    for (src, want) in cases {
        assert_eq!(holes(&checked(src)), want, "{src:?}");
    }
}

#[test]
fn unclosed_brackets_are_closed_at_the_end() {
    for src in ["(1 + 2", "f(1, 2", "[1, 2", "v[3", "sqrt(4", "if(1, 2, 3"] {
        let e = checked(src);
        assert!(!has_garbage(&e), "{src:?} -> {e}");
    }
    assert_eq!(checked("(1 + 2"), parse("(1 + 2)"));
    assert_eq!(checked("sqrt(4"), parse("sqrt(4)"));
}

#[test]
fn text_that_cannot_be_used_is_kept_as_typed() {
    let Expr::Garbage { head, text } = checked("1 + 2 $ 3") else {
        panic!("expected garbage");
    };
    assert_eq!(text, "$ 3");
    assert_eq!(head.map(|h| h.to_string()).as_deref(), Some("1 + 2"));
    let Expr::Garbage { head, text } = checked("2 + 3 = 4") else {
        panic!("expected garbage");
    };
    assert_eq!(text, "= 4");
    assert!(head.is_some());
    // A bad token in operand position is garbage of its own.
    let e = checked("1 + $");
    let Expr::Binary { rhs, .. } = e else {
        panic!("{e}");
    };
    assert_eq!(
        *rhs,
        Expr::Garbage {
            head: None,
            text: "$".into()
        }
    );
    // A bad number keeps its text, in the operand position it was typed in.
    let e = checked("1.2.3 + 1");
    let Expr::Binary { lhs, .. } = e else {
        panic!("{e}");
    };
    assert_eq!(
        *lhs,
        Expr::Garbage {
            head: None,
            text: "1.2.3".into()
        }
    );
}

#[test]
fn the_strict_parse_never_makes_display_nodes() {
    fn clean(e: &Expr) -> bool {
        !matches!(e, Expr::Hole | Expr::Garbage { .. }) && e.children().into_iter().all(clean)
    }
    for src in ["1 +", "(1", "x := ", "1 $ 2", ""] {
        assert!(clean(&parse(src)), "{src:?}");
    }
}

#[test]
fn display_nodes_are_never_evaluated_into_values() {
    use opencalc_core::eval::{evaluate, RegionId};
    for src in ["1 +", "1 $ 2", ""] {
        let (expr, _) = parse_lenient(src);
        let out = evaluate(&[(RegionId(1), expr)]);
        assert!(out.values[&RegionId(1)].is_err(), "{src:?}");
    }
}

#[test]
fn over_long_and_unicode_input_is_kept_whole_or_parsed() {
    let long = "1+".repeat(5000);
    assert!(matches!(checked(&long), Expr::Garbage { head: None, .. }));
    assert!(!has_garbage(&checked("\u{3b1} + \u{3b2}")));
    assert!(has_garbage(&checked("\u{3b1} \u{20ac} 2")));
}

fn token() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "1", "2.5", "1.2.3", "x", "y_1", "m", "kN", "+", "-", "*", "/", "^", "(", ")", "[", "]",
        ",", ";", "..", ":=", "=", "==", "<", "sin(", "sqrt(", "if(", "f(", "\u{3c3}", "$",
        "\u{20ac}", " ", "  ",
    ])
}

proptest! {
    /// Any text at all gives a tree with consistent spans, and when the strict parse
    /// succeeds the two trees are equal.
    #[test]
    fn random_text_never_panics_and_agrees_when_valid(tokens in prop::collection::vec(token(), 0..40)) {
        let src = tokens.concat();
        let chars: Vec<char> = src.chars().collect();
        let (expr, spans) = parse_lenient(&src);
        prop_assert!(!matches!(expr, Expr::Error(_)));
        if let Some(p) = problem(&expr, &spans, &chars) {
            prop_assert!(false, "{:?}: {}", src, p);
        }
        let strict = parse(&src);
        if !matches!(strict, Expr::Error(_)) {
            prop_assert_eq!(strict, expr);
        }
    }

    #[test]
    fn arbitrary_characters_never_panic(src in "\\PC{0,60}") {
        let (expr, _) = parse_lenient(&src);
        prop_assert!(!matches!(expr, Expr::Error(_)));
    }
}

#[test]
fn a_greek_name_being_entered_is_one_piece() {
    for src in [r"\alpha", r"\", r"\sig", r"x+\alpha", r"2\beta"] {
        checked(src);
    }
    assert_eq!(
        checked(r"\alpha"),
        Expr::Garbage {
            head: None,
            text: r"\alpha".into()
        }
    );
    // The strict parse still refuses it.
    assert!(matches!(parse(r"\alpha"), Expr::Error(_)));
}

/// Programs being typed: whatever has been written so far, the lenient parse gives
/// a tree with consistent spans (bad lines kept as text, a new line as a hole).
const PROGRAMS: &[&str] = &[
    "f(n) := program\n    s \u{2190} 0\n    for i in 1..n\n        s \u{2190} s + i\n    return s",
    "g := program\n    if x > 1\n        a \u{2190} 1\n    else if x < 0\n        a \u{2190} 2\n    else\n        a \u{2190} 3\n    a",
    "h := program\n    try\n        v[0] \u{2190} 1 / 0\n    on error e\n        v[1] \u{2190} e\n    while 1\n        break\n    return v",
    "k := program\n    x \u{2190} 1\n  y \u{2190} 2\n        z \u{2190} 3\n    x +",
];

#[test]
fn every_prefix_of_a_program_has_consistent_spans() {
    for src in PROGRAMS {
        let chars: Vec<char> = src.chars().collect();
        for n in 0..=chars.len() {
            let prefix: String = chars[..n].iter().collect();
            checked(&prefix);
        }
    }
}

#[test]
fn a_new_line_in_a_program_is_a_hole_where_it_will_be_typed() {
    let src = "f := program\n    s \u{2190} 0\n    ";
    let e = checked(src);
    assert_eq!(holes(&e), 1, "{e:?}");
    let (_, spans) = parse_lenient(src);
    let len = src.chars().count();
    // The hole sits at the very end, after the indent.
    let last_statement = spans.at_path(&[0, 1]).expect("the hole");
    assert_eq!(
        (last_statement.inner.start, last_statement.inner.end),
        (len, len)
    );
}

#[test]
fn a_line_that_does_not_parse_is_kept_as_typed_and_the_rest_still_parses() {
    let src = "f := program\n    for i in\n        s \u{2190} 1\n    else\n    return 2";
    let e = checked(src);
    assert!(has_garbage(&e), "{e:?}");
    // The lines after the bad ones are still statements.
    assert!(e.to_string().contains("return 2"), "{e}");
}

#[test]
fn complete_programs_parse_the_same_leniently_as_strictly() {
    for src in PROGRAMS {
        let strict = parse(src);
        if !matches!(strict, Expr::Error(_)) {
            assert_eq!(strict, checked(src), "{src:?}");
        }
    }
}

#[test]
fn every_prefix_of_a_given_block_has_consistent_spans() {
    for src in [
        "r := given\n    x + y == 10\n    x - y == 2\nfind(x, y)",
        "given\n    a ^ 2 == 9\nfind(a)",
        "m := given\n    a >= 5\nminimize((a - 3)^2, a)",
        "p := program\n    r \u{2190} given\n        a == 1\n    find(a)\n    r",
    ] {
        let chars: Vec<char> = src.chars().collect();
        for n in 0..=chars.len() {
            let prefix: String = chars[..n].iter().collect();
            checked(&prefix);
        }
        assert_eq!(parse(src), checked(src), "{src:?}");
    }
}

#[test]
fn the_closing_line_of_a_given_block_being_typed_is_a_box() {
    let e = checked("r := given\n    a == 1\n");
    assert_eq!(holes(&e), 1, "{e:?}");
}
