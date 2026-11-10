//! Pratt parser for typed math input (spec section 3.3 and Milestone 2).
//!
//! `parse` never panics and never returns `Result`: failures become
//! `Expr::Error` so the UI can show them in place.
//!
//! Grammar notes:
//! - `:=` defines, `=` evaluates. `x =` alone evaluates the name `x`.
//! - A number directly followed by a built-in unit is a quantity (`10 kN`).
//!   If a power follows (`5 m^2`) it becomes `5 * m^2`; unit names that are
//!   not defined as variables resolve as units at evaluation time.
//! - A number followed by another name or `(` is implicit multiplication (`2x`).
//! - `sqrt(x)` is typeset as a radical.

mod forms;
mod lenient;
mod lexer;
mod program;
mod spans;

pub use lenient::parse_lenient;
use lexer::{describe, tokenize, Tok};
pub use spans::{Span, SpanNode};

use crate::expr::ParseError;
use crate::expr::{AssignKind, BinaryOp, Expr, UnaryOp};

/// Most symbols (numbers, names, operators, brackets) one region may hold.
///
/// This bounds the depth of every recursive step downstream (parsing, cloning,
/// evaluating, typesetting, dropping), so typed or pasted input can never
/// overflow the stack. Measured: the worst shape (a chain of unary minus signs)
/// costs about 3 KB of stack per symbol in a debug build, so 128 symbols needs
/// under 400 KB, inside the 1 MB main-thread stack on Windows. Real equations
/// are far shorter. Raise it only together with the stack-margin test below.
pub const MAX_SYMBOLS: usize = 128;
/// Source text longer than this is rejected before tokenising.
const MAX_SOURCE_CHARS: usize = 8192;
/// Most symbols in a whole program. Each line is still held to `MAX_SYMBOLS`, which
/// is what bounds the depth of an expression; blocks nest at most `MAX_BLOCK_DEPTH`
/// deep, so the depth of a program is the two together.
pub const MAX_PROGRAM_SYMBOLS: usize = 2048;
const MAX_BLOCK_DEPTH: usize = 12;

/// Parse one region's source text.
pub fn parse(src: &str) -> Expr {
    parse_inner(src, false).0
}

/// Parse, and also say which characters each node came from. The span tree has the
/// shape of the expression tree (see [`SpanNode`]). On a parse error the
/// expression is `Expr::Error` and the span tree is a single node over the whole
/// source.
pub fn parse_with_spans(src: &str) -> (Expr, SpanNode) {
    let (expr, log) = parse_inner(src, false);
    let chars = src.chars().count();
    let whole = || SpanNode::leaf(0, chars);
    if matches!(expr, Expr::Error(_)) {
        return (expr, whole());
    }
    let mut entries = log.into_iter();
    let spans = spans::rebuild(&expr, &mut entries);
    // Every node must have had exactly one entry. A mismatch is a bug in the
    // parser; fall back to one span over the source rather than lie.
    debug_assert!(
        spans.is_some() && entries.next().is_none(),
        "span log out of step with the tree for {src:?}"
    );
    match spans {
        Some(s) if entries.next().is_none() => (expr, s),
        _ => (expr, whole()),
    }
}

/// The expression and the log of node spans, in the order nodes were completed.
fn parse_inner(src: &str, lenient: bool) -> (Expr, Vec<spans::Entry>) {
    let chars = src.chars().count();
    let failed = |error: ParseError| (Expr::Error(error), Vec::new());
    if chars > MAX_SOURCE_CHARS {
        return failed(ParseError::new(too_long(), MAX_SOURCE_CHARS, chars));
    }
    let (toks, spans) = match tokenize(src, lenient) {
        Ok(t) => t,
        Err((message, start, end)) => return failed(ParseError::new(message, start, end)),
    };
    if let Some(at) = over_the_limit(&toks) {
        let start = spans[at].0;
        return failed(ParseError::new(too_long(), start, chars));
    }
    let mut parser = Parser {
        toks,
        spans: spans.iter().map(|(s, e)| Span::new(*s, *e)).collect(),
        pos: 0,
        log: Vec::new(),
        lenient,
        text: src.chars().collect(),
        blocks: 0,
        end: chars,
    };
    match parser.parse_all() {
        Ok(e) => (e, parser.log),
        Err(message) => {
            // The parser stops on the token it could not use; past the last token
            // the problem is the end of the input.
            let (start, end) = spans.get(parser.pos).copied().unwrap_or((chars, chars));
            failed(ParseError::new(message, start, end))
        }
    }
}

/// The first token past the limits: more than `MAX_SYMBOLS` on one line, or more
/// than `MAX_PROGRAM_SYMBOLS` altogether.
fn over_the_limit(toks: &[Tok]) -> Option<usize> {
    let mut on_line = 0;
    for (i, t) in toks.iter().enumerate() {
        on_line = if *t == Tok::Line { 0 } else { on_line + 1 };
        if on_line > MAX_SYMBOLS || i >= MAX_PROGRAM_SYMBOLS {
            return Some(i);
        }
    }
    None
}

fn too_long() -> String {
    format!("expression is too long (limit {MAX_SYMBOLS} symbols)")
}

struct Parser {
    toks: Vec<Tok>,
    /// The character range of each token.
    spans: Vec<Span>,
    pos: usize,
    /// One entry per node, pushed when the node is complete (children first).
    log: Vec<spans::Entry>,
    /// Display parse (`parse_lenient`): recover instead of failing.
    lenient: bool,
    /// The source characters (for `Garbage` text and the indentation of lines).
    text: Vec<char>,
    /// How many blocks of a program the parser is inside.
    blocks: usize,
    /// The length of the source in characters.
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Where token `i` starts, in characters.
    fn start_of(&self, i: usize) -> usize {
        self.spans[i].start
    }

    /// Where the most recently consumed token ends.
    fn last_end(&self) -> usize {
        self.spans[self.pos - 1].end
    }

    /// Record a finished node covering characters `start..end`.
    fn done(&mut self, start: usize, end: usize) {
        self.log.push(spans::Entry::new(start, end));
    }

    /// Record the token just consumed as a node of its own.
    fn done_token(&mut self) {
        let s = self.spans[self.pos - 1];
        self.done(s.start, s.end);
    }

    /// The span, parentheses included, of the node completed last.
    fn last_node(&self) -> Span {
        self.log.last().map_or(Span::new(0, 0), |e| e.outer)
    }

    fn parse_all(&mut self) -> Result<Expr, String> {
        if self.toks.is_empty() {
            if self.lenient {
                return Ok(self.hole());
            }
            return Err("empty".into());
        }
        let mark = self.log.len();
        let lhs = self.range_expr(true)?;
        let from = self.last_node().start;
        let result = match self.peek() {
            Some(Tok::Define) if matches!(lhs, Expr::Call { .. }) => {
                // `f(x, y) := body`. Checked first, while the position is still
                // on the `:=`.
                match forms::function_signature(&lhs) {
                    Err(message) => self.keep_or_fail(message, lhs)?,
                    Ok(forms::Signature {
                        name,
                        params,
                        defaults,
                    }) => {
                        self.pos += 1;
                        let body_mark = self.log.len();
                        let body = self.range_expr(true)?;
                        // The signature is not a child: only the body is.
                        self.log.drain(mark..body_mark);
                        self.done(from, self.last_node().end);
                        Expr::FunctionDef {
                            name,
                            params,
                            defaults: Box::new(defaults),
                            body: Box::new(body),
                        }
                    }
                }
            }
            Some(Tok::Define) => {
                // Checked first, while the position is still on the `:=`.
                match forms::target_name(&lhs) {
                    Err(message) => self.keep_or_fail(message, lhs)?,
                    Ok(target) => {
                        self.pos += 1;
                        let body_mark = self.log.len();
                        let value = self.range_expr(true)?;
                        self.log.drain(mark..body_mark);
                        self.done(from, self.last_node().end);
                        Expr::Assign {
                            kind: AssignKind::Define,
                            target,
                            value: Box::new(value),
                        }
                    }
                }
            }
            Some(Tok::Equals)
                if self.toks.len() > self.pos + 1 && forms::target_name(&lhs).is_err() =>
            {
                let message = "left of '=' must be a name".to_owned();
                self.keep_or_fail(message, lhs)?
            }
            Some(Tok::Equals) => {
                let target = if self.toks.len() > self.pos + 1 {
                    Some(forms::target_name(&lhs)?)
                } else {
                    None
                };
                self.pos += 1;
                if self.peek().is_none() {
                    // `x =` shows the value of the expression on the left, so the
                    // left side is the child.
                    self.done(from, self.last_end());
                    Expr::Assign {
                        kind: AssignKind::Evaluate,
                        target: match &lhs {
                            Expr::Name(n) => n.clone(),
                            _ => String::new(),
                        },
                        value: Box::new(lhs),
                    }
                } else {
                    let body_mark = self.log.len();
                    let value = self.range_expr(true)?;
                    self.log.drain(mark..body_mark);
                    self.done(from, self.last_node().end);
                    Expr::Assign {
                        kind: AssignKind::Evaluate,
                        target: target.unwrap_or_default(),
                        value: Box::new(value),
                    }
                }
            }
            _ => lhs,
        };
        if forms::has_default_argument(&result) {
            let message =
                "a default (n := 2) belongs in a function definition, f(x, n := 2) := ...";
            return self.keep_or_fail(message.to_owned(), result);
        }
        match self.peek() {
            None => Ok(result),
            Some(_) if self.lenient => Ok(self.garbage_tail(result)),
            Some(t) => Err(format!("unexpected {}", describe(t))),
        }
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = self.prefix()?;
        let from = self.last_node().start;
        loop {
            let (op, lbp, rbp) = match self.peek() {
                Some(Tok::EqEq) => (BinaryOp::Eq, 5, 6),
                Some(Tok::NotEq) => (BinaryOp::Ne, 5, 6),
                Some(Tok::Less) => (BinaryOp::Lt, 5, 6),
                Some(Tok::Greater) => (BinaryOp::Gt, 5, 6),
                Some(Tok::LessEq) => (BinaryOp::Le, 5, 6),
                Some(Tok::GreaterEq) => (BinaryOp::Ge, 5, 6),
                Some(Tok::Plus) => (BinaryOp::Add, 10, 11),
                Some(Tok::Minus) => (BinaryOp::Sub, 10, 11),
                Some(Tok::Star) => (BinaryOp::Mul, 20, 21),
                Some(Tok::Slash) => (BinaryOp::Div, 20, 21),
                Some(Tok::Caret) => (BinaryOp::Pow, 31, 30),
                _ => break,
            };
            if lbp < min_bp {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(rbp)?;
            self.done(from, self.last_node().end);
            lhs = Expr::bin(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<Expr, String> {
        let first = self.pos;
        match self.next() {
            Some(Tok::Num(v)) => self.after_number(v),
            Some(Tok::Str(text)) => {
                self.done_token();
                Ok(Expr::Text(text))
            }
            Some(Tok::Ident(name)) if name == "program" && self.peek() == Some(&Tok::Line) => {
                self.program(first)
            }
            Some(Tok::Ident(name)) => {
                let e = if self.eat(&Tok::LParen) {
                    let args = self.args()?;
                    self.done(self.start_of(first), self.close_end());
                    forms::call(name, args)
                } else {
                    self.done_token();
                    Expr::Name(name)
                };
                self.indexed(e)
            }
            Some(Tok::LParen) => {
                let e = self.range_expr(true)?;
                if self.eat(&Tok::RParen) || self.lenient {
                    // The brackets belong to the node they wrap.
                    let (open, close) = (self.start_of(first), self.close_end());
                    if let Some(entry) = self.log.last_mut() {
                        entry.outer = Span::new(open, close);
                    }
                    self.indexed(e)
                } else {
                    Err("missing ')'".into())
                }
            }
            Some(Tok::LBracket) => {
                let m = self.matrix(self.start_of(first))?;
                self.indexed(m)
            }
            Some(Tok::Minus) => {
                let arg = self.expr(25)?;
                self.done(self.start_of(first), self.last_node().end);
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    arg: Box::new(arg),
                })
            }
            Some(Tok::Plus) => self.expr(25),
            Some(Tok::Bad(text)) if self.lenient => {
                self.done_token();
                Ok(Expr::Garbage { head: None, text })
            }
            // `next` stepped past the token; step back so the error is on it.
            Some(t) => {
                self.pos -= 1;
                if self.lenient {
                    return Ok(self.hole());
                }
                Err(format!("unexpected {}", describe(&t)))
            }
            None => {
                self.pos -= 1;
                if self.lenient {
                    return Ok(self.hole());
                }
                Err("expression ends early".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{evaluate, RegionId};
    use BinaryOp::*;

    /// Evaluate several source lines (layout order) and return the SI value of one.
    fn run(lines: &[&str], which: usize) -> f64 {
        let regions: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, s)| (RegionId(i as u64), parse(s)))
            .collect();
        let out = evaluate(&regions);
        out.values[&RegionId(which as u64)]
            .clone()
            .unwrap()
            .as_real()
            .unwrap()
            .si
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(run(&["1 + 2 * 3"], 0), 7.0);
        assert_eq!(run(&["(1 + 2) * 3"], 0), 9.0);
        assert_eq!(run(&["2 ^ 3 ^ 2"], 0), 512.0); // right associative
        assert_eq!(run(&["-2 ^ 2"], 0), -4.0); // power binds tighter than negation
        assert_eq!(run(&["10 - 3 - 2"], 0), 5.0); // left associative
        assert_eq!(run(&["2 ^ -1"], 0), 0.5);
    }

    #[test]
    fn definitions_and_evaluate_forms() {
        assert_eq!(parse("x := 1"), Expr::define("x", Expr::Number(1.0)),);
        assert_eq!(parse("x ="), Expr::evaluate("x", Expr::name("x")),);
    }

    #[test]
    fn quantities_and_unit_powers() {
        assert_eq!(parse("10 kN"), Expr::qty(10.0, "kN"));
        // 5 m^2 is five square metres, not (5 m)^2.
        assert_eq!(run(&["5 m^2"], 0), 5.0);
        assert_eq!(run(&["(5 m)^2"], 0), 25.0);
    }

    #[test]
    fn stress_worksheet_in_any_order() {
        let lines = ["sigma := F / A", "F := 10 kN", "A := (0.1 m)^2"];
        let regions: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, s)| (RegionId(i as u64), parse(s)))
            .collect();
        let out = evaluate(&regions);
        let q = out.values[&RegionId(0)].clone().unwrap().as_real().unwrap();
        assert!((q.in_unit("MPa").unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn user_variable_shadows_a_unit_name() {
        assert_eq!(run(&["m := 3", "r := m * 2"], 1), 6.0);
    }

    #[test]
    fn division_of_units_resolves_without_definitions() {
        // 6 kN / 2 m = 3000 N/m; check the SI value only.
        assert_eq!(run(&["6 kN / 2 m"], 0), 3000.0);
    }

    #[test]
    fn implicit_multiplication_and_functions() {
        assert_eq!(run(&["x := 4", "2x ^ 2"], 1), 32.0);
        assert_eq!(run(&["sqrt(16)"], 0), 4.0);
        assert!(matches!(parse("sqrt(16)"), Expr::Root { .. }));
        assert_eq!(run(&["cos(0)"], 0), 1.0);
    }

    #[test]
    fn scientific_notation_without_eating_a_name() {
        assert_eq!(run(&["1.5e3"], 0), 1500.0);
        assert_eq!(run(&["2e-2"], 0), 0.02);
        // `2e` has no digits after e, so e is a separate name (implicit multiply).
        assert_eq!(run(&["e := 3", "2e"], 1), 6.0);
    }

    #[test]
    fn greek_names_are_names() {
        assert_eq!(
            parse("\u{03c3} := 1"),
            Expr::define("\u{03c3}", Expr::Number(1.0))
        );
    }

    #[test]
    fn bad_input_becomes_error_nodes_never_panics() {
        for src in [
            "",
            "1 +",
            "(1",
            "1)",
            "* 2",
            "x := ",
            ":= 3",
            "1 + $",
            "f(1,",
            "2 = 3",
            "x := y := 2",
            "1 2 3 ,",
            "^",
            "((((",
        ] {
            assert!(
                matches!(parse(src), Expr::Error(_)),
                "expected error for {src:?}"
            );
        }
    }

    /// Stack for the margin test: half the 1 MB Windows main-thread stack, which
    /// leaves the rest for the UI's own frames. The worst case measures ~384 KB.
    const SMALL_STACK: usize = 512 * 1024;

    /// Parse, clone (as the UI does for each snapshot), evaluate and drop, on a
    /// thread with a deliberately small stack. A stack overflow aborts the whole
    /// test binary, so merely finishing proves there is a safety margin.
    fn survives_on_small_stack(src: String) {
        let handle = std::thread::Builder::new()
            .stack_size(SMALL_STACK)
            .spawn(move || {
                let e = parse(&src);
                let copy = e.clone();
                let regions = vec![(RegionId(1), copy)];
                let _ = evaluate(&regions);
                drop(e);
                drop(regions);
            })
            .unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn absurdly_deep_input_is_rejected_not_a_crash() {
        let src = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
        assert!(matches!(parse(&src), Expr::Error(m) if m.message.contains("too long")));
        assert!(matches!(parse(&"1+".repeat(1_000_000)), Expr::Error(_)));
        assert!(matches!(parse(&"-".repeat(50_000)), Expr::Error(_)));
    }

    #[test]
    fn the_symbol_limit_is_exact() {
        // "1" followed by "+1" pairs: 1 + 2n symbols.
        let ok = format!("1{}", "+1".repeat((MAX_SYMBOLS - 1) / 2));
        assert!(!matches!(parse(&ok), Expr::Error(_)), "at the limit");
        let too_long = format!("1{}", "+1".repeat((MAX_SYMBOLS - 1) / 2 + 1));
        assert!(matches!(parse(&too_long), Expr::Error(_)), "one over");
    }

    #[test]
    fn worst_case_shapes_at_the_limit_fit_in_a_small_stack() {
        let half = (MAX_SYMBOLS - 1) / 2;
        survives_on_small_stack(format!("{}1{}", "(".repeat(half), ")".repeat(half)));
        survives_on_small_stack(format!("{}1", "-".repeat(MAX_SYMBOLS - 1)));
        survives_on_small_stack(format!("1{}", "+1".repeat(half)));
        survives_on_small_stack(format!("2{}", "^2".repeat(half)));
        survives_on_small_stack(format!(
            "{}x{}",
            "f(".repeat(half / 2),
            ")".repeat(half / 2)
        ));
        // The new nesting forms: matrix literals, indexing, conditionals, comparisons.
        // Each must be valid (not a parse error), or only the error path is tested.
        let valid = |src: String| {
            assert!(!matches!(parse(&src), Expr::Error(_)), "{src}");
            survives_on_small_stack(src);
        };
        valid(format!("{}1{}", "[".repeat(half), "]".repeat(half)));
        valid(format!("x{}", "[0]".repeat(half / 2)));
        valid(format!(
            "{}1{}",
            "x[".repeat(half / 2),
            "]".repeat(half / 2)
        ));
        valid(format!(
            "{}1{}",
            "if(1,".repeat(half / 4),
            ",2)".repeat(half / 4)
        ));
        valid(format!("1{}", "<1".repeat(half)));
        valid(format!("1{}", "==1".repeat(half / 2)));
    }

    #[test]
    fn unary_minus_and_precedence_with_power_on_units() {
        assert_eq!(run(&["-3 + 5"], 0), 2.0);
        assert!(matches!(parse("a - b"), Expr::Binary { op: Sub, .. }));
        assert!(matches!(parse("a / b"), Expr::Binary { op: Div, .. }));
    }

    /// The text a parse error points at, for the cases below.
    fn error_at(src: &str) -> (String, String) {
        let Expr::Error(e) = parse(src) else {
            panic!("{src:?} should not parse");
        };
        assert!(e.start <= e.end, "{src:?}: {e:?}");
        let chars: Vec<char> = src.chars().collect();
        assert!(e.end <= chars.len(), "{src:?}: {e:?}");
        (e.message, chars[e.start..e.end].iter().collect())
    }

    #[test]
    fn parse_errors_point_at_the_offending_source() {
        let cases = [
            ("1 + * 2", "unexpected '*'", "*"),
            ("(1 + 2", "missing ')'", ""),
            ("1 + 2)", "unexpected ')'", ")"),
            ("1 + ", "expression ends early", ""),
            ("2 $ 3", "unexpected '$'", "$"),
            ("1.2.3 + 1", "bad number '1.2.3'", "1.2.3"),
            ("2 + 3 = 4", "left of '=' must be a name", "="),
            ("f(1 2)", "missing ')'", "2"),
            ("a := b := 2", "unexpected ':='", ":="),
        ];
        for (src, message, text) in cases {
            assert_eq!(
                error_at(src),
                (message.to_owned(), text.to_owned()),
                "{src}"
            );
        }
    }

    #[test]
    fn default_arguments() {
        let f = "f(x, n := 2, m := n0 + 1) := x ^ n + m";
        let lines = |call: &str| [f, "n0 := 10", call].map(str::to_owned);
        let value = |call: &str| {
            let l = lines(call);
            run(&[&l[0], &l[1], &l[2]], 2)
        };
        assert_eq!(value("f(3)"), 3.0_f64.powi(2) + 11.0);
        assert_eq!(value("f(3, 3)"), 27.0 + 11.0);
        assert_eq!(value("f(3, 3, 0)"), 27.0);
        assert_eq!(parse(f).to_string(), f);
        let err = |call: &str| {
            let out = evaluate(&[
                (RegionId(0), parse(f)),
                (RegionId(1), parse("n0 := 10")),
                (RegionId(2), parse(call)),
            ]);
            out.values[&RegionId(2)].clone().unwrap_err().to_string()
        };
        assert_eq!(
            err("f()"),
            "cannot evaluate: function 'f' takes 1 to 3 arguments"
        );
        assert_eq!(
            err("f(1, 2, 3, 4)"),
            "cannot evaluate: function 'f' takes 1 to 3 arguments"
        );
    }

    #[test]
    fn default_arguments_are_checked_where_they_are_written() {
        let bad = [
            ("f(x, n := 2, m) := x", "'m' needs a default"),
            ("f(x, n := x) := x", "cannot use the parameter 'x'"),
            ("f(x := 1, n := x) := x", "cannot use the parameter 'x'"),
            ("g(1, n := 2)", "belongs in a function definition"),
            ("1 + g(n := 2)", "belongs in a function definition"),
        ];
        for (src, part) in bad {
            let (message, _) = error_at(src);
            assert!(message.contains(part), "{src}: {message}");
        }
        // A default is worked out in the worksheet, so a missing name is an error
        // at the call, and a default that is itself a unit expression works.
        let out = evaluate(&[
            (RegionId(0), parse("f(x, n := nothing) := x + n")),
            (RegionId(1), parse("f(1)")),
            (RegionId(2), parse("h(x, d := 2 m) := x + d")),
            (RegionId(3), parse("h(1 m)")),
        ]);
        assert!(out.values[&RegionId(1)].is_err());
        assert_eq!(
            out.values[&RegionId(3)]
                .clone()
                .unwrap()
                .as_real()
                .unwrap()
                .si,
            3.0
        );
    }

    #[test]
    fn binary_octal_and_hexadecimal_literals() {
        assert_eq!(run(&["0xFF"], 0), 255.0);
        assert_eq!(run(&["0Xff + 1"], 0), 256.0);
        assert_eq!(run(&["0b1011"], 0), 11.0);
        assert_eq!(run(&["0o17 * 2"], 0), 30.0);
        assert_eq!(run(&["0x1F m"], 0), 31.0);
        assert_eq!(run(&["0x20000000000000"], 0), 9007199254740992.0);
        assert_eq!(run(&["2^0x3"], 0), 8.0);
        // Too large to be exact, rather than quietly rounded.
        assert_eq!(
            error_at("0x20000000000001"),
            (
                "number too large to hold exactly (most 2^53)".to_owned(),
                "0x20000000000001".to_owned()
            )
        );
        // Not a literal: a digit outside the radix is a name run on to the zero,
        // which is an ordinary (undefined) name; nothing is read as 0b1 times 2.
        for src in ["0b12", "0o8", "0x", "0xG"] {
            let out = evaluate(&[(RegionId(0), parse(src))]);
            assert!(out.values[&RegionId(0)].is_err(), "{src}");
        }
    }

    #[test]
    fn the_global_definition_sign_is_the_same_as_define() {
        assert_eq!(parse("x \u{2261} 1 + 2"), parse("x := 1 + 2"));
        assert_eq!(parse("f(x) \u{2261} x^2"), parse("f(x) := x^2"));
        assert!(matches!(parse("x \u{2261}"), Expr::Error(_)));
    }

    #[test]
    fn error_ranges_count_characters_not_bytes() {
        // Greek letters are two bytes each; the range must still land on the `$`.
        let (_, text) = error_at("\u{3b1} + \u{3b2} $ 2");
        assert_eq!(text, "$");
    }

    #[test]
    fn an_over_long_expression_is_flagged_from_where_it_becomes_too_long() {
        let src = "1+".repeat(MAX_SYMBOLS);
        let Expr::Error(e) = parse(&src) else {
            panic!("should be too long");
        };
        assert_eq!(e.start, MAX_SYMBOLS);
        assert_eq!(e.end, src.chars().count());
    }
}
