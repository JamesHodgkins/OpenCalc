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

use crate::expr::{AssignKind, BinaryOp, Expr, UnaryOp};
use crate::units::builtin_unit;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Define,
    Equals,
}

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
    if src.len() > MAX_SOURCE_CHARS {
        return Expr::Error(too_long());
    }
    let parsed = tokenize(src).and_then(|toks| {
        if toks.len() > MAX_SYMBOLS {
            Err(too_long())
        } else {
            Parser { toks, pos: 0 }.parse_all()
        }
    });
    match parsed {
        Ok(e) => e,
        Err(msg) => Expr::Error(msg),
    }
}

fn too_long() -> String {
    format!("expression is too long (limit {MAX_SYMBOLS} symbols)")
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                // Exponent: only when `e` is followed by a digit or a signed digit.
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    let mut j = i + 1;
                    if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                        j += 1;
                    }
                    if j < chars.len() && chars[j].is_ascii_digit() {
                        while j < chars.len() && chars[j].is_ascii_digit() {
                            j += 1;
                        }
                        i = j;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let v = text
                    .parse::<f64>()
                    .map_err(|_| format!("bad number '{text}'"))?;
                toks.push(Tok::Num(v));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                toks.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            ':' if chars.get(i + 1) == Some(&'=') => {
                toks.push(Tok::Define);
                i += 2;
            }
            '+' => {
                toks.push(Tok::Plus);
                i += 1;
            }
            '-' | '\u{2212}' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '*' | '\u{00b7}' | '\u{00d7}' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '/' => {
                toks.push(Tok::Slash);
                i += 1;
            }
            '^' => {
                toks.push(Tok::Caret);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '=' => {
                toks.push(Tok::Equals);
                i += 1;
            }
            other => return Err(format!("unexpected '{other}'")),
        }
    }
    Ok(toks)
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

    fn parse_all(mut self) -> Result<Expr, String> {
        if self.toks.is_empty() {
            return Err("empty".into());
        }
        let lhs = self.expr(0)?;
        let result = match self.peek() {
            Some(Tok::Define) => {
                self.pos += 1;
                let value = self.expr(0)?;
                Expr::Assign {
                    kind: AssignKind::Define,
                    target: target_name(&lhs)?,
                    value: Box::new(value),
                }
            }
            Some(Tok::Equals) => {
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
                    let value = self.expr(0)?;
                    Expr::Assign {
                        kind: AssignKind::Evaluate,
                        target: target_name(&lhs)?,
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
                if self.eat(&Tok::LParen) {
                    let args = self.args()?;
                    Ok(call(name, args))
                } else {
                    Ok(Expr::Name(name))
                }
            }
            Some(Tok::LParen) => {
                let e = self.expr(0)?;
                if self.eat(&Tok::RParen) {
                    Ok(e)
                } else {
                    Err("missing ')'".into())
                }
            }
            Some(Tok::Minus) => Ok(Expr::Unary {
                op: UnaryOp::Neg,
                arg: Box::new(self.expr(25)?),
            }),
            Some(Tok::Plus) => self.expr(25),
            Some(t) => Err(format!("unexpected {}", describe(&t))),
            None => Err("expression ends early".into()),
        }
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.eat(&Tok::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.expr(0)?);
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

fn describe(t: &Tok) -> String {
    match t {
        Tok::Num(v) => format!("number {v}"),
        Tok::Ident(n) => format!("'{n}'"),
        Tok::Plus => "'+'".into(),
        Tok::Minus => "'-'".into(),
        Tok::Star => "'*'".into(),
        Tok::Slash => "'/'".into(),
        Tok::Caret => "'^'".into(),
        Tok::LParen => "'('".into(),
        Tok::RParen => "')'".into(),
        Tok::Comma => "','".into(),
        Tok::Define => "':='".into(),
        Tok::Equals => "'='".into(),
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
        out.values[&RegionId(which as u64)].clone().unwrap().si
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
        let q = out.values[&RegionId(0)].clone().unwrap();
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
            "1..2",
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
        assert!(matches!(parse(&src), Expr::Error(m) if m.contains("too long")));
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
    }

    #[test]
    fn unary_minus_and_precedence_with_power_on_units() {
        assert_eq!(run(&["-3 + 5"], 0), 2.0);
        assert!(matches!(parse("a - b"), Expr::Binary { op: Sub, .. }));
        assert!(matches!(parse("a / b"), Expr::Binary { op: Div, .. }));
    }
}
