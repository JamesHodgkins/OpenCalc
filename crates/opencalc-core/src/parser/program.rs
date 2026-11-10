//! Programs: `program` and the indented statements under it (docs/PROGRAMMING.md).
//!
//! A block is the lines indented further than the line that opens it, all at one
//! indent. The statements are `name \u{2190} value`, `if` / `else`, `for name in
//! range`, `while`, `break`, `continue`, `return`, `try` / `on error`, and any
//! other expression. Like the rest of the parser, each node records its span.

use super::lexer::Tok;
use super::{Parser, MAX_BLOCK_DEPTH};
use crate::expr::Expr;

impl Parser {
    /// The columns of whitespace at the start of the line token `tok` is on.
    pub(super) fn line_indent(&self, tok: usize) -> usize {
        let at = self.spans[tok].start.min(self.text.len());
        let mut start = at;
        while start > 0 && self.text[start - 1] != '\n' {
            start -= 1;
        }
        let mut columns = 0;
        for c in &self.text[start..at] {
            match c {
                ' ' => columns += 1,
                '\t' => columns += 4,
                _ => break,
            }
        }
        columns
    }

    /// `program` (the token `first`, already consumed) and its block.
    pub(super) fn program(&mut self, first: usize) -> Result<Expr, String> {
        let from = self.start_of(first);
        let base = self.line_indent(first);
        self.block(base, Some(from))
    }

    /// The indented block that follows, whose lines are indented further than
    /// `parent`. `from` is where the block is taken to start, if not at its first
    /// statement. The block is a `Program` node.
    fn block(&mut self, parent: usize, from: Option<usize>) -> Result<Expr, String> {
        if self.blocks >= MAX_BLOCK_DEPTH {
            return Err("blocks are nested too deeply".into());
        }
        self.blocks += 1;
        let block = self.block_lines(parent, from);
        self.blocks -= 1;
        block
    }

    fn block_lines(&mut self, parent: usize, from: Option<usize>) -> Result<Expr, String> {
        let mut statements = Vec::new();
        let mut indent: Option<usize> = None;
        let mut start = from;
        while self.peek() == Some(&Tok::Line) && self.pos + 1 < self.toks.len() {
            let here = self.line_indent(self.pos + 1);
            match indent {
                None if here <= parent => break,
                None => indent = Some(here),
                Some(k) if here < k => break,
                Some(k) if here > k => {
                    self.pos += 1;
                    return Err("this line is indented more than the lines above it".into());
                }
                Some(_) => {}
            }
            self.pos += 1;
            start.get_or_insert(self.start_of(self.pos));
            statements.push(self.statement(here)?);
        }
        if statements.is_empty() {
            return Err("expected an indented line here".into());
        }
        self.done(start.unwrap_or(0), self.last_node().end);
        Ok(Expr::Program(statements))
    }

    /// Where a statement may end: at a line break, the end, or a bracket or comma
    /// that belongs to something around the program.
    fn at_statement_end(&self) -> bool {
        matches!(
            self.peek(),
            None | Some(Tok::Line | Tok::RParen | Tok::Comma | Tok::RBracket | Tok::Semicolon)
        )
    }

    fn word(&self, at: usize, word: &str) -> bool {
        matches!(self.toks.get(at), Some(Tok::Ident(w)) if w == word)
    }

    /// One statement, whose line is indented `indent`.
    fn statement(&mut self, indent: usize) -> Result<Expr, String> {
        let from = self.start_of(self.pos);
        let Some(Tok::Ident(word)) = self.peek().cloned() else {
            return self.simple(from);
        };
        match word.as_str() {
            "if" => self.if_statement(indent, from),
            "for" => {
                self.pos += 1;
                let Some(Tok::Ident(var)) = self.next() else {
                    self.pos -= 1;
                    return Err("'for' needs the name of a variable".into());
                };
                if !matches!(self.next(), Some(Tok::Ident(w)) if w == "in") {
                    self.pos -= 1;
                    return Err("expected 'in' after the variable of 'for'".into());
                }
                let over = self.range_expr(true)?;
                let body = self.block(indent, None)?;
                self.done(from, self.last_node().end);
                Ok(Expr::For {
                    var,
                    over: Box::new(over),
                    body: Box::new(body),
                })
            }
            "while" => {
                self.pos += 1;
                let cond = self.expr(0)?;
                let body = self.block(indent, None)?;
                self.done(from, self.last_node().end);
                Ok(Expr::While {
                    cond: Box::new(cond),
                    body: Box::new(body),
                })
            }
            "break" | "continue" => {
                self.pos += 1;
                self.done(from, self.last_end());
                Ok(if word == "break" {
                    Expr::Break
                } else {
                    Expr::Continue
                })
            }
            "return" => {
                self.pos += 1;
                let value = self.range_expr(true)?;
                self.done(from, self.last_node().end);
                Ok(Expr::Return(Box::new(value)))
            }
            "try" => {
                self.pos += 1;
                let body = self.block(indent, None)?;
                let handler_follows = self.peek() == Some(&Tok::Line)
                    && self.word(self.pos + 1, "on")
                    && self.word(self.pos + 2, "error")
                    && self.line_indent(self.pos + 1) == indent;
                if !handler_follows {
                    return Err("'try' needs an 'on error' block after it".into());
                }
                self.pos += 3;
                let handler = self.block(indent, None)?;
                self.done(from, self.last_node().end);
                Ok(Expr::Try {
                    body: Box::new(body),
                    handler: Box::new(handler),
                })
            }
            "else" => Err("'else' has no 'if' above it".into()),
            "on" if self.word(self.pos + 1, "error") => {
                Err("'on error' has no 'try' above it".into())
            }
            _ => self.simple(from),
        }
    }

    /// `if cond` and its block, with `else` or `else if` after it. `if(a, b, c)`
    /// on a line of its own is the expression.
    fn if_statement(&mut self, indent: usize, from: usize) -> Result<Expr, String> {
        if self.toks.get(self.pos + 1) == Some(&Tok::LParen) {
            let (pos, log) = (self.pos, self.log.len());
            if let Ok(e) = self.simple(from) {
                if matches!(e, Expr::Conditional { .. }) && self.at_statement_end() {
                    return Ok(e);
                }
            }
            self.pos = pos;
            self.log.truncate(log);
        }
        self.pos += 1;
        let cond = self.expr(0)?;
        let then = self.block(indent, None)?;
        let else_follows = self.peek() == Some(&Tok::Line)
            && self.word(self.pos + 1, "else")
            && self.line_indent(self.pos + 1) == indent;
        let otherwise = if else_follows {
            self.pos += 2;
            if self.word(self.pos, "if") {
                // `else if` is an `else` whose block is the one `if`.
                let nested_from = self.start_of(self.pos);
                let nested = self.if_statement(indent, nested_from)?;
                self.done(nested_from, self.last_node().end);
                Some(Box::new(Expr::Program(vec![nested])))
            } else {
                Some(Box::new(self.block(indent, None)?))
            }
        } else {
            None
        };
        self.done(from, self.last_node().end);
        Ok(Expr::IfBlock {
            cond: Box::new(cond),
            then: Box::new(then),
            otherwise,
        })
    }

    /// `name \u{2190} value`, or an expression.
    fn simple(&mut self, from: usize) -> Result<Expr, String> {
        let mark = self.log.len();
        let first = self.range_expr(true)?;
        if self.peek() != Some(&Tok::Arrow) {
            return Ok(first);
        }
        let (target, indices) = match first {
            Expr::Name(target) => (target, None),
            Expr::Index { base, indices } if matches!(*base, Expr::Name(_)) => {
                let Expr::Name(target) = *base else {
                    unreachable!("the base was checked to be a name")
                };
                (target, Some(indices))
            }
            _ => {
                return Err(
                    "only a name, or one element of one, can be on the left of '\u{2190}'".into(),
                )
            }
        };
        let name_end = self.log.len();
        self.pos += 1;
        let value = self.range_expr(true)?;
        let node = Box::new(value);
        if let Some(indices) = indices {
            // The name and the `[ ]` node around the indices are not children;
            // the indices and the value are.
            self.log.remove(name_end - 1);
            self.log.remove(mark);
            self.done(from, self.last_node().end);
            return Ok(Expr::IndexAssign {
                target,
                indices: indices.into_boxed_slice(),
                value: node,
            });
        }
        // The name is not a child: only the value is.
        self.log.drain(mark..name_end);
        self.done(from, self.last_node().end);
        Ok(Expr::LocalAssign {
            target,
            value: node,
        })
    }
}
