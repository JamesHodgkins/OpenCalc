//! The smaller grammar forms: ranges, indexing, matrix literals, call arguments
//! and numbers with units. Each records the source span of every node it builds
//! (see `spans`).

use super::lexer::Tok;
use super::Parser;
use crate::expr::{AssignKind, BinaryOp, Expr};
use crate::units::{builtin_unit, offset_unit};

/// `name(args)`: `if` with three arguments and `sqrt` with one become their own
/// nodes; their children are still the arguments.
pub(super) fn call(name: String, mut args: Vec<Expr>) -> Expr {
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

/// The name and parameters of `f(x, y)` on the left of `:=`.
pub(super) fn function_signature(lhs: &Expr) -> Result<Signature, String> {
    let Expr::Call { name, args } = lhs else {
        return Err("left of ':=' must be a name or a function like f(x)".into());
    };
    let mut params: Vec<String> = Vec::new();
    let mut defaults: Vec<Expr> = Vec::new();
    for arg in args {
        let (p, default) = match arg {
            Expr::Name(p) => (p, None),
            Expr::Assign {
                kind: AssignKind::Define,
                target,
                value,
            } => (target, Some(&**value)),
            _ => return Err(format!("the parameters of '{name}' must be plain names")),
        };
        if params.contains(p) {
            return Err(format!("'{name}' has the parameter '{p}' twice"));
        }
        match default {
            Some(value) => {
                // A default is worked out where the function is called, so it can
                // see the worksheet but not the other parameters.
                let used = value.free_names();
                if let Some(other) = used.iter().find(|n| params.contains(n) || *n == p) {
                    return Err(format!(
                        "the default of '{p}' cannot use the parameter '{other}'"
                    ));
                }
                defaults.push(value.clone());
            }
            None if !defaults.is_empty() => {
                return Err(format!(
                    "'{p}' needs a default: a parameter after one with a default must have one too"
                ));
            }
            None => {}
        }
        params.push(p.clone());
    }
    if params.is_empty() {
        return Err(format!("'{name}' needs at least one parameter"));
    }
    Ok(Signature {
        name: name.clone().into_boxed_str(),
        params: params.into_boxed_slice(),
        defaults,
    })
}

/// `f(x, n := 2)` taken apart: see `Expr::FunctionDef`.
pub(super) struct Signature {
    pub name: Box<str>,
    pub params: Box<[String]>,
    pub defaults: Vec<Expr>,
}

/// Whether `expr` holds a default argument (`n := 2` written as an argument),
/// which only a function definition's signature may.
pub(super) fn has_default_argument(expr: &Expr) -> bool {
    // Walks with its own stack: the tree can be as deep as the parser allows, and
    // a recursion here would need more call stack than the parser itself does.
    let default = |e: &Expr| {
        matches!(
            e,
            Expr::Assign {
                kind: AssignKind::Define,
                ..
            }
        )
    };
    let mut todo = vec![expr];
    while let Some(e) = todo.pop() {
        if matches!(e, Expr::Call { args, .. } if args.iter().any(default)) {
            return true;
        }
        todo.extend(e.children());
    }
    false
}

/// `[x; y]` or `[x, y]` on the left of `:=`: the names, when it is only names.
pub(super) fn multi_targets(lhs: &Expr) -> Option<Vec<String>> {
    let Expr::Matrix { cells, .. } = lhs else {
        return None;
    };
    cells
        .iter()
        .map(|c| match c {
            Expr::Name(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn target_name(lhs: &Expr) -> Result<String, String> {
    match lhs {
        Expr::Name(n) => Ok(n.clone()),
        _ => Err("left of '=' must be a name".into()),
    }
}

impl Parser {
    /// An expression, or a range `a..b` / `a, b..c`. The step form takes a comma,
    /// so it is only allowed where a comma cannot separate anything else.
    pub(super) fn range_expr(&mut self, allow_step: bool) -> Result<Expr, String> {
        let first = self.expr(0)?;
        let from = self.last_node().start;
        let mut step = None;
        if allow_step && self.peek() == Some(&Tok::Comma) {
            self.pos += 1;
            step = Some(self.expr(0)?);
            if self.peek() != Some(&Tok::DotDot) && !self.lenient {
                return Err("a step needs '..' and an end value, as in 0, 2..10".into());
            }
        }
        // While typing, `0, 2` is a range still missing its end.
        if !self.eat(&Tok::DotDot) && !(self.lenient && step.is_some()) {
            return Ok(first);
        }
        let end = self.expr(0)?;
        self.done(from, self.last_node().end);
        Ok(Expr::Range {
            start: Box::new(first),
            step: step.map(Box::new),
            end: Box::new(end),
        })
    }

    /// Any number of `[i]` or `[i, j]` after a value (the node completed last).
    pub(super) fn indexed(&mut self, mut base: Expr) -> Result<Expr, String> {
        let from = self.last_node().start;
        while self.eat(&Tok::LBracket) {
            let mut indices = vec![self.expr(0)?];
            while self.eat(&Tok::Comma) {
                indices.push(self.expr(0)?);
            }
            if !self.eat(&Tok::RBracket) && !self.lenient {
                return Err("missing ']'".into());
            }
            self.done(from, self.close_end());
            base = Expr::Index {
                base: Box::new(base),
                indices,
            };
        }
        Ok(base)
    }

    /// The inside of `[ ... ]` after the opening bracket (which starts at
    /// character `open`): rows separated by `;`, cells by `,`. Every row must have
    /// the same length.
    pub(super) fn matrix(&mut self, open: usize) -> Result<Expr, String> {
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
            } else if in_row < cols && self.lenient {
                // A row still being typed: the missing cells show as holes.
                for _ in in_row..cols {
                    let hole = self.hole();
                    cells.push(hole);
                }
            } else if in_row != cols {
                return Err(format!(
                    "every row needs {cols} cell(s), this one has {in_row}"
                ));
            }
            rows += 1;
            if self.eat(&Tok::Semicolon) {
                continue;
            }
            if self.eat(&Tok::RBracket) || self.lenient {
                self.done(open, self.close_end());
                return Ok(Expr::Matrix { rows, cols, cells });
            }
            return Err("missing ']'".into());
        }
    }

    /// One argument: an expression, or `name := value`, which only the signature
    /// of a function definition may use (`f(x, n := 2) := ...`).
    fn argument(&mut self) -> Result<Expr, String> {
        let (mark, from) = (self.log.len(), self.spans.get(self.pos).map(|s| s.start));
        let first = self.range_expr(false)?;
        let (Expr::Name(target), Some(Tok::Define), Some(from)) = (&first, self.peek(), from)
        else {
            return Ok(first);
        };
        let target = target.clone();
        let name_end = self.log.len();
        self.pos += 1;
        let value = self.range_expr(false)?;
        self.log.drain(mark..name_end);
        self.done(from, self.last_node().end);
        Ok(Expr::Assign {
            kind: AssignKind::Define,
            target,
            value: Box::new(value),
        })
    }

    pub(super) fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.eat(&Tok::RParen) {
            return Ok(args);
        }
        loop {
            // A step needs a comma, which here separates arguments, so only the
            // plain `a..b` form is allowed inside a call.
            args.push(self.argument()?);
            if self.eat(&Tok::Comma) {
                continue;
            }
            if self.eat(&Tok::RParen) || self.lenient {
                return Ok(args);
            }
            return Err("missing ')'".into());
        }
    }

    /// Handle units and implicit multiplication after a numeric literal (the
    /// token just consumed).
    pub(super) fn after_number(&mut self, v: f64) -> Result<Expr, String> {
        let number_at = self.pos - 1;
        let from = self.start_of(number_at);
        self.done_token();
        match self.peek().cloned() {
            Some(Tok::Ident(name)) => {
                let is_call = self.toks.get(self.pos + 1) == Some(&Tok::LParen);
                if (builtin_unit(&name).is_some() || offset_unit(&name).is_some()) && !is_call {
                    self.pos += 1;
                    if self.eat(&Tok::Caret) {
                        // `5 m^2`: the number times the power of the unit's name.
                        let name_at = self.pos - 2;
                        let (name_start, name_end) =
                            (self.start_of(name_at), self.spans[name_at].end);
                        self.done(name_start, name_end);
                        let exp = self.expr(30)?;
                        self.done(name_start, self.last_node().end);
                        let unit = Expr::bin(BinaryOp::Pow, Expr::Name(name), exp);
                        self.done(from, self.last_node().end);
                        return Ok(Expr::bin(BinaryOp::Mul, Expr::Number(v), unit));
                    }
                    // A quantity is one node covering the number and its unit.
                    self.log.pop();
                    self.done(from, self.last_end());
                    return Ok(Expr::Quantity {
                        value: v,
                        unit: name,
                    });
                }
                self.times(v, from)
            }
            Some(Tok::LParen) => self.times(v, from),
            _ => Ok(Expr::Number(v)),
        }
    }

    /// `v` times whatever follows, as in `2x` or `2(x + 1)`.
    fn times(&mut self, v: f64, from: usize) -> Result<Expr, String> {
        let rest = self.expr(30)?;
        self.done(from, self.last_node().end);
        Ok(Expr::bin(BinaryOp::Mul, Expr::Number(v), rest))
    }
}
