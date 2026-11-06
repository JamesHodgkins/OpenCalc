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

mod lexer;

use lexer::{describe, tokenize, Tok};

use crate::expr::ParseError;
use crate::expr::{AssignKind, BinaryOp, Expr, UnaryOp};
use crate::units::builtin_unit;

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
const MAX_SOURCE_CHARS: usize = 4096;

/// Parse one region's source text.
pub fn parse(src: &str) -> Expr {
    let chars = src.chars().count();
    if chars > MAX_SOURCE_CHARS {
        return Expr::Error(ParseError::new(too_long(), MAX_SOURCE_CHARS, chars));
    }
    let (toks, spans) = match tokenize(src) {
        Ok(t) => t,
        Err((message, start, end)) => return Expr::Error(ParseError::new(message, start, end)),
    };
    if toks.len() > MAX_SYMBOLS {
        let start = spans[MAX_SYMBOLS].0;
        return Expr::Error(ParseError::new(too_long(), start, chars));
    }
    let mut parser = Parser { toks, pos: 0 };
    match parser.parse_all() {
        Ok(e) => e,
        Err(message) => {
            // The parser stops on the token it could not use; past the last token
            // the problem is the end of the input.
            let (start, end) = spans.get(parser.pos).copied().unwrap_or((chars, chars));
            Expr::Error(ParseError::new(message, start, end))
        }
    }
}

fn too_long() -> String {
    format!("expression is too long (limit {MAX_SYMBOLS} symbols)")
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
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

    fn parse_all(&mut self) -> Result<Expr, String> {
        if self.toks.is_empty() {
            return Err("empty".into());
        }
        let lhs = self.range_expr(true)?;
        let result = match self.peek() {
            Some(Tok::Define) => {
                // Checked first, while the position is still on the `:=`.
                let target = target_name(&lhs)?;
                self.pos += 1;
                let value = self.range_expr(true)?;
                Expr::Assign {
                    kind: AssignKind::Define,
                    target,
                    value: Box::new(value),
                }
            }
            Some(Tok::Equals) => {
                let target = if self.toks.len() > self.pos + 1 {
                    Some(target_name(&lhs)?)
                } else {
                    None
                };
                self.pos += 1;
                if self.peek().is_none() {
                    // `x =` shows the value of the expression on the left.
                    Expr::Assign {
                        kind: AssignKind::Evaluate,
                        target: match &lhs {
                            Expr::Name(n) => n.clone(),
                            _ => String::new(),
                        },
                        value: Box::new(lhs),
                    }
                } else {
                    let value = self.range_expr(true)?;
                    Expr::Assign {
                        kind: AssignKind::Evaluate,
                        target: target.unwrap_or_default(),
                        value: Box::new(value),
                    }
                }
            }
            _ => lhs,
        };
        match self.peek() {
            None => Ok(result),
            Some(t) => Err(format!("unexpected {}", describe(t))),
        }
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = self.prefix()?;
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
            lhs = Expr::bin(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Tok::Num(v)) => self.after_number(v),
            Some(Tok::Ident(name)) => {
                let e = if self.eat(&Tok::LParen) {
                    let args = self.args()?;
                    call(name, args)
                } else {
                    Expr::Name(name)
                };
                self.indexed(e)
            }
            Some(Tok::LParen) => {
                let e = self.range_expr(true)?;
                if self.eat(&Tok::RParen) {
                    self.indexed(e)
                } else {
                    Err("missing ')'".into())
                }
            }
            Some(Tok::LBracket) => {
                let m = self.matrix()?;
                self.indexed(m)
            }
            Some(Tok::Minus) => Ok(Expr::Unary {
                op: UnaryOp::Neg,
                arg: Box::new(self.expr(25)?),
            }),
            Some(Tok::Plus) => self.expr(25),
            // `next` stepped past the token; step back so the error is on it.
            Some(t) => {
                self.pos -= 1;
                Err(format!("unexpected {}", describe(&t)))
            }
            None => {
                self.pos -= 1;
                Err("expression ends early".into())
            }
        }
    }

    /// An expression, or a range `a..b` / `a, b..c`. The step form takes a comma,
    /// so it is only allowed where a comma cannot separate anything else.
    fn range_expr(&mut self, allow_step: bool) -> Result<Expr, String> {
        let first = self.expr(0)?;
        let mut step = None;
        if allow_step && self.peek() == Some(&Tok::Comma) {
            self.pos += 1;
            step = Some(self.expr(0)?);
            if self.peek() != Some(&Tok::DotDot) {
                return Err("a step needs '..' and an end value, as in 0, 2..10".into());
            }
        }
        if !self.eat(&Tok::DotDot) {
            return Ok(first);
        }
        let end = self.expr(0)?;
        Ok(Expr::Range {
            start: Box::new(first),
            step: step.map(Box::new),
            end: Box::new(end),
        })
    }

    /// Any number of `[i]` or `[i, j]` after a value.
    fn indexed(&mut self, mut base: Expr) -> Result<Expr, String> {
        while self.eat(&Tok::LBracket) {
            let mut indices = vec![self.expr(0)?];
            while self.eat(&Tok::Comma) {
                indices.push(self.expr(0)?);
            }
            if !self.eat(&Tok::RBracket) {
                return Err("missing ']'".into());
            }
            base = Expr::Index {
                base: Box::new(base),
                indices,
            };
        }
        Ok(base)
    }

    /// The inside of `[ ... ]` after the opening bracket: rows separated by `;`,
    /// cells by `,`. Every row must have the same length.
    fn matrix(&mut self) -> Result<Expr, String> {
        let mut cells = Vec::new();
        let (mut rows, mut cols) = (0, 0);
        loop {
            let mut in_row = 0;
            loop {
                cells.push(self.expr(0)?);
                in_row += 1;
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            if rows == 0 {
                cols = in_row;
            } else if in_row != cols {
                return Err(format!(
                    "every row needs {cols} cell(s), this one has {in_row}"
                ));
            }
            rows += 1;
            if self.eat(&Tok::Semicolon) {
                continue;
            }
            if self.eat(&Tok::RBracket) {
                return Ok(Expr::Matrix { rows, cols, cells });
            }
            return Err("missing ']'".into());
        }
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.eat(&Tok::RParen) {
            return Ok(args);
        }
        loop {
            // A step needs a comma, which here separates arguments, so only the
            // plain `a..b` form is allowed inside a call.
            args.push(self.range_expr(false)?);
            if self.eat(&Tok::Comma) {
                continue;
            }
            if self.eat(&Tok::RParen) {
                return Ok(args);
            }
            return Err("missing ')'".into());
        }
    }

    /// Handle units and implicit multiplication after a numeric literal.
    fn after_number(&mut self, v: f64) -> Result<Expr, String> {
        match self.peek().cloned() {
            Some(Tok::Ident(name)) => {
                let is_call = self.toks.get(self.pos + 1) == Some(&Tok::LParen);
                if builtin_unit(&name).is_some() && !is_call {
                    self.pos += 1;
                    if self.eat(&Tok::Caret) {
                        let exp = self.expr(30)?;
                        let unit = Expr::bin(BinaryOp::Pow, Expr::Name(name), exp);
                        return Ok(Expr::bin(BinaryOp::Mul, Expr::Number(v), unit));
                    }
                    return Ok(Expr::Quantity {
                        value: v,
                        unit: name,
                    });
                }
                Ok(Expr::bin(BinaryOp::Mul, Expr::Number(v), self.expr(30)?))
            }
            Some(Tok::LParen) => Ok(Expr::bin(BinaryOp::Mul, Expr::Number(v), self.expr(30)?)),
            _ => Ok(Expr::Number(v)),
        }
    }
}

fn call(name: String, mut args: Vec<Expr>) -> Expr {
    if name == "if" && args.len() == 3 {
        let (otherwise, then, cond) = (args.pop(), args.pop(), args.pop());
        if let (Some(cond), Some(then), Some(otherwise)) = (cond, then, otherwise) {
            return Expr::Conditional {
                cond: Box::new(cond),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            };
        }
        unreachable!("three arguments were checked");
    }
    if name == "sqrt" && args.len() == 1 {
        if let Some(radicand) = args.pop() {
            return Expr::Root {
                radicand: Box::new(radicand),
            };
        }
    }
    Expr::Call { name, args }
}

fn target_name(lhs: &Expr) -> Result<String, String> {
    match lhs {
        Expr::Name(n) => Ok(n.clone()),
        _ => Err("left of '=' must be a name".into()),
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
