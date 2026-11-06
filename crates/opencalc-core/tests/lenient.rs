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
