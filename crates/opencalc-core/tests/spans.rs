//! Source spans: every node of every parsed expression knows which characters it
//! came from. Checked on every source in the golden file and the corpus, on
//! hand-checked examples, and on random input.

use std::path::PathBuf;

use opencalc_core::expr::Expr;
use opencalc_core::parser::{parse, parse_with_spans, Span, SpanNode};
use proptest::prelude::*;

fn crate_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// Why `spans` does not describe `expr` over a source of `chars`, if it does not.
fn problem(expr: &Expr, spans: &SpanNode, chars: &[char]) -> Option<String> {
    let kids = expr.children();
    if kids.len() != spans.kids.len() {
        return Some(format!(
            "{} children but {} kid spans in {expr}",
            kids.len(),
            spans.kids.len()
        ));
    }
    if !spans.outer.contains_span(spans.inner) {
        return Some(format!(
            "inner {:?} outside outer {:?}",
            spans.inner, spans.outer
        ));
    }
    if spans.outer.end > chars.len() || spans.outer.start > spans.outer.end {
        return Some(format!("span {:?} is not inside the source", spans.outer));
    }
    let mut previous_end = spans.inner.start;
    for kid in &spans.kids {
        if !spans.inner.contains_span(kid.outer) {
            return Some(format!(
                "kid {:?} is not inside its parent's inner span {:?}",
                kid.outer, spans.inner
            ));
        }
        if kid.outer.start < previous_end {
            return Some(format!("kid {:?} overlaps the one before it", kid.outer));
        }
        previous_end = kid.outer.end;
    }
    kids.iter()
        .zip(&spans.kids)
        .find_map(|(e, s)| problem(e, s, chars))
}

/// Parse `src` and check the spans; return them for further checks.
fn checked(src: &str) -> (Expr, SpanNode) {
    let chars: Vec<char> = src.chars().collect();
    let (expr, spans) = parse_with_spans(src);
    assert_eq!(
        parse(src),
        expr,
        "parse and parse_with_spans disagree on {src:?}"
    );
    if let Some(p) = problem(&expr, &spans, &chars) {
        panic!("{src:?}: {p}");
    }
    if !matches!(expr, Expr::Error(_)) {
        // The root covers the text from the first to the last character that is
        // not white space.
        let first = chars.iter().position(|c| !c.is_whitespace()).unwrap();
        let last = chars.iter().rposition(|c| !c.is_whitespace()).unwrap() + 1;
        assert_eq!(spans.outer, Span::new(first, last), "root span of {src:?}");
    }
    (expr, spans)
}

fn text(src: &str, span: Span) -> String {
    src.chars().skip(span.start).take(span.len()).collect()
}

/// Every source the project already has: the golden inputs, the corpus lines and the
/// expressions its expectations are written as.
fn known_sources() -> Vec<String> {
    let mut out = Vec::new();
    let golden = std::fs::read_to_string(crate_path("tests/golden/parser.golden")).unwrap();
    out.extend(
        golden
            .replace("\r\n", "\n")
            .lines()
            .filter(|l| !l.starts_with("  "))
            .map(str::to_owned),
    );
    let dir = crate_path("../../tests/corpus");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "calc") {
            for line in std::fs::read_to_string(&path).unwrap().lines() {
                if let Some(rest) = line.strip_prefix("=> ") {
                    if let Some((_, expr)) = rest.split_once(" = ") {
                        out.push(expr.to_owned());
                    }
                } else if !(line.starts_with('#')
                    || line.starts_with("!>")
                    || line.starts_with('@'))
                {
                    out.push(line.to_owned());
                }
            }
        }
    }
    out.retain(|s| !s.trim().is_empty());
    out
}

#[test]
fn every_known_source_has_consistent_spans() {
    let sources = known_sources();
    assert!(sources.len() > 400, "only {} sources found", sources.len());
    let mut parsed = 0;
    for src in &sources {
        let (expr, _) = checked(src);
        if !matches!(expr, Expr::Error(_)) {
            parsed += 1;
        }
    }
    assert!(parsed > 350, "only {parsed} of {} parsed", sources.len());
}

type Wants<'a> = &'a [(&'a [u16], &'a str)];

#[test]
fn spans_land_on_the_right_characters() {
    // Source, then (path, the text that node must cover): checked by hand.
    let cases: &[(&str, Wants)] = &[
        ("1 + 2", &[(&[], "1 + 2"), (&[0], "1"), (&[1], "2")]),
        ("  1 +   2  ", &[(&[], "1 +   2"), (&[1], "2")]),
        (
            "(a + b) * c",
            &[
                (&[], "(a + b) * c"),
                (&[0], "(a + b)"),
                (&[0, 0], "a"),
                (&[0, 1], "b"),
                (&[1], "c"),
            ],
        ),
        ("2 ^ 3 ^ 2", &[(&[1], "3 ^ 2"), (&[1, 0], "3")]),
        ("-x", &[(&[], "-x"), (&[0], "x")]),
        ("5 kN", &[(&[], "5 kN")]),
        ("5 kN / 2 m", &[(&[0], "5 kN"), (&[1], "2 m")]),
        (
            "3 m ^ 2",
            &[
                (&[], "3 m ^ 2"),
                (&[0], "3"),
                (&[1], "m ^ 2"),
                (&[1, 0], "m"),
                (&[1, 1], "2"),
            ],
        ),
        ("2 x", &[(&[], "2 x"), (&[0], "2"), (&[1], "x")]),
        ("2(x + 1)", &[(&[1], "(x + 1)")]),
        ("sqrt(x ^ 2)", &[(&[], "sqrt(x ^ 2)"), (&[0], "x ^ 2")]),
        (
            "max(1, 22, 3)",
            &[
                (&[], "max(1, 22, 3)"),
                (&[0], "1"),
                (&[1], "22"),
                (&[2], "3"),
            ],
        ),
        ("x := 3", &[(&[], "x := 3"), (&[0], "3")]),
        ("x = ", &[(&[], "x ="), (&[0], "x")]),
        ("y = x + 1", &[(&[], "y = x + 1"), (&[0], "x + 1")]),
        (
            "f(a, b) := a * b",
            &[(&[], "f(a, b) := a * b"), (&[0], "a * b"), (&[0, 1], "b")],
        ),
        (
            "[1, 2; 3, 4]",
            &[(&[], "[1, 2; 3, 4]"), (&[0], "1"), (&[3], "4")],
        ),
        (
            "v[i + 1]",
            &[(&[], "v[i + 1]"), (&[0], "v"), (&[1], "i + 1")],
        ),
        (
            "M[0][1]",
            &[
                (&[], "M[0][1]"),
                (&[0], "M[0]"),
                (&[0, 0], "M"),
                (&[1], "1"),
            ],
        ),
        (
            "if(a > b, a, b)",
            &[(&[0], "a > b"), (&[1], "a"), (&[2], "b")],
        ),
        (
            "0, 2..10",
            &[(&[], "0, 2..10"), (&[0], "0"), (&[1], "2"), (&[2], "10")],
        ),
        ("a == b", &[(&[], "a == b")]),
        (
            "\u{3b1} + \u{3b2}",
            &[(&[], "\u{3b1} + \u{3b2}"), (&[1], "\u{3b2}")],
        ),
    ];
    for (src, wants) in cases {
        let (_, spans) = checked(src);
        for (path, want) in *wants {
            let node = spans
                .at_path(path)
                .unwrap_or_else(|| panic!("{src:?} has no node at {path:?}"));
            assert_eq!(text(src, node.outer), *want, "{src:?} at {path:?}");
        }
    }
}

#[test]
fn parentheses_belong_to_the_node_they_wrap() {
    let (_, s) = checked("(a + b) * c");
    let wrapped = s.at_path(&[0]).unwrap();
    assert_eq!(text("(a + b) * c", wrapped.outer), "(a + b)");
    assert_eq!(text("(a + b) * c", wrapped.inner), "a + b");
    // Without brackets the two are the same.
    let plain = s.at_path(&[1]).unwrap();
    assert_eq!(plain.outer, plain.inner);
    // Brackets nest: the outer span takes all of them, the inner none.
    let (_, s) = checked("((x))");
    assert_eq!(text("((x))", s.outer), "((x))");
    assert_eq!(text("((x))", s.inner), "x");
}

#[test]
fn a_parse_error_has_one_span_over_the_whole_source() {
    for src in ["1 +", "(1 + 2", "[1, 2; 3]", "1 $ 2"] {
        let (expr, spans) = parse_with_spans(src);
        assert!(matches!(expr, Expr::Error(_)));
        assert_eq!(spans.outer, Span::new(0, src.chars().count()));
        assert!(spans.kids.is_empty());
    }
}

#[test]
fn an_error_path_from_the_evaluator_names_the_failing_characters() {
    use opencalc_core::eval::{evaluate, RegionId};
    let src = "y := 2 * sqrt(1 / 0)";
    let (expr, spans) = parse_with_spans(src);
    let out = evaluate(&[(RegionId(1), expr)]);
    let path = &out.error_paths[&RegionId(1)];
    let node = spans.at_path(path).expect("the error path has a span");
    assert_eq!(text(src, node.outer), "1 / 0");
}

fn token() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "1", "2.5", "x", "y_1", "m", "kN", "+", "-", "*", "/", "^", "(", ")", "[", "]", ",", ";",
        "..", ":=", "=", "==", "<", "sin(", "sqrt(", "if(", "f(", "\u{3c3}", " ", "  ",
    ])
}

proptest! {
    /// Whatever parses has consistent spans, and what does not parse has one span.
    #[test]
    fn random_input_never_gets_inconsistent_spans(tokens in prop::collection::vec(token(), 1..30)) {
        let src = tokens.concat();
        let chars: Vec<char> = src.chars().collect();
        let (expr, spans) = parse_with_spans(&src);
        prop_assert_eq!(parse(&src), expr.clone());
        if let Some(p) = problem(&expr, &spans, &chars) {
            prop_assert!(false, "{:?}: {}", src, p);
        }
    }
}
