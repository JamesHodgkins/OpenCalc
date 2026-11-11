//! Prefix forms: numbers, names, calls, brackets, signs and the other things an
//! expression can start with.

use super::lexer::{describe, Tok};
use super::Span;
use super::{forms, Parser};
use crate::expr::{Expr, UnaryOp};

impl Parser {
    pub(super) fn prefix(&mut self) -> Result<Expr, String> {
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
            Some(Tok::Ident(name)) if name == "given" && self.peek() == Some(&Tok::Line) => {
                self.given(first)
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
