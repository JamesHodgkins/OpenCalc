//! Program statements: assignment, `if`, loops and the other simple statements.

use super::lexer::Tok;
use super::Parser;
use crate::expr::Expr;

impl Parser {
    /// One statement, whose line is indented `indent`.
    pub(super) fn statement(&mut self, indent: usize) -> Result<Expr, String> {
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
                // `on error e` names the message.
                let message = match self.peek() {
                    Some(Tok::Ident(name)) => {
                        let name = name.clone();
                        self.pos += 1;
                        Some(name)
                    }
                    _ => None,
                };
                let handler = self.block(indent, None)?;
                self.done(from, self.last_node().end);
                Ok(Expr::Try {
                    body: Box::new(body),
                    handler: Box::new(handler),
                    message,
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
    pub(super) fn if_statement(&mut self, indent: usize, from: usize) -> Result<Expr, String> {
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
    pub(super) fn simple(&mut self, from: usize) -> Result<Expr, String> {
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
