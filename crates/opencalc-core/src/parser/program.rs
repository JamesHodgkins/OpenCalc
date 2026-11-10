//! Programs: `program` and the indented statements under it (docs/PROGRAMMING.md).
//!
//! A block is the lines indented further than the line that opens it, all at one
//! indent. The statements are `name \u{2190} value`, `if` / `else`, `for name in
//! range`, `while`, `break`, `continue`, `return`, `try` / `on error`, and any
//! other expression. Like the rest of the parser, each node records its span.

use super::lexer::Tok;
use super::{Parser, MAX_BLOCK_DEPTH};
use crate::expr::{is_solver_form, Expr, GivenBlock};

const CLOSING: &str =
    "a 'given' block ends with a line that starts find, minerr, minimize or maximize";

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

    /// `given` (the token `first`, already consumed), its constraint lines, and the
    /// `find`, `minerr`, `minimize` or `maximize` line at the same indent as the line
    /// `given` is on. The node's children are the constraints, then the closing
    /// line's arguments, as in the text.
    pub(super) fn given(&mut self, first: usize) -> Result<Expr, String> {
        let from = self.start_of(first);
        let base = self.line_indent(first);
        let mut constraints = Vec::new();
        while self.peek() == Some(&Tok::Line) {
            let blank_last = self.pos + 1 >= self.toks.len();
            let here = if blank_last {
                self.trailing_indent()
            } else {
                self.line_indent(self.pos + 1)
            };
            if here <= base {
                break;
            }
            self.pos += 1;
            if blank_last {
                let hole = self.hole();
                constraints.push(hole);
                break;
            }
            let (line, mark) = (self.pos, self.log.len());
            match self.expr(0) {
                Ok(e) if self.lenient && !self.at_statement_end() => {
                    let kept = self.garbage_to_line_end(e);
                    constraints.push(kept);
                }
                Ok(e) => constraints.push(e),
                Err(message) if !self.lenient => return Err(message),
                Err(_) => {
                    self.log.truncate(mark);
                    self.pos = line;
                    let kept = self.garbage_line();
                    constraints.push(kept);
                }
            }
        }
        let closes = self.peek() == Some(&Tok::Line)
            && self.pos + 1 < self.toks.len()
            && self.line_indent(self.pos + 1) == base
            && matches!(self.toks.get(self.pos + 1), Some(Tok::Ident(n)) if is_solver_form(n))
            && self.toks.get(self.pos + 2) == Some(&Tok::LParen);
        let (solver, args) = if closes {
            self.pos += 1;
            match self.expr(0)? {
                Expr::Call { name, args } => {
                    // The call's own entry: the block is not a call.
                    self.log.pop();
                    (name, args)
                }
                other if self.lenient => ("find".to_owned(), vec![other]),
                _ => return Err(CLOSING.into()),
            }
        } else if self.lenient {
            // The closing line is not written yet, or not finished being typed.
            let more = self.peek() == Some(&Tok::Line) && self.pos + 1 < self.toks.len();
            if self.peek() == Some(&Tok::Line) {
                self.pos += 1;
            }
            let placeholder = if more {
                self.garbage_line()
            } else {
                self.hole()
            };
            ("find".to_owned(), vec![placeholder])
        } else {
            return Err(CLOSING.into());
        };
        let end = self.close_end();
        self.done(from, end);
        Ok(Expr::Given(Box::new(GivenBlock {
            solver,
            args,
            constraints,
        })))
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
        while self.peek() == Some(&Tok::Line) {
            // A line break with nothing after it: only the lenient parse keeps it,
            // for the line being typed.
            let blank_last = self.pos + 1 >= self.toks.len();
            let here = if blank_last {
                self.trailing_indent()
            } else {
                self.line_indent(self.pos + 1)
            };
            match indent {
                None if here <= parent => break,
                None => indent = Some(here),
                Some(k) if here < k => break,
                // (Being typed, a line may sit deeper than the one above it.)
                Some(k) if here > k && !self.lenient => {
                    self.pos += 1;
                    return Err("this line is indented more than the lines above it".into());
                }
                Some(_) => {}
            }
            self.pos += 1;
            if blank_last {
                start.get_or_insert(self.next_char());
                let hole = self.hole();
                statements.push(hole);
                break;
            }
            start.get_or_insert(self.start_of(self.pos));
            let (line, mark) = (self.pos, self.log.len());
            match self.statement(here) {
                Ok(s) if self.lenient && !self.at_statement_end() => {
                    let kept = self.garbage_to_line_end(s);
                    statements.push(kept);
                }
                Ok(s) => statements.push(s),
                Err(message) if !self.lenient => return Err(message),
                Err(_) => {
                    // A line that does not parse is kept as it was typed.
                    self.log.truncate(mark);
                    self.pos = line;
                    let kept = self.garbage_line();
                    statements.push(kept);
                }
            }
        }
        if statements.is_empty() {
            if !self.lenient {
                return Err("expected an indented line here".into());
            }
            if self.peek() == Some(&Tok::Line) && self.pos + 1 >= self.toks.len() {
                self.pos += 1;
            }
            start.get_or_insert(self.next_char());
            let hole = self.hole();
            statements.push(hole);
        }
        self.done(start.unwrap_or(0), self.last_node().end);
        Ok(Expr::Program(statements))
    }

    /// Where the next token starts, or the end of the source.
    fn next_char(&self) -> usize {
        self.spans.get(self.pos).map_or(self.end, |s| s.start)
    }

    /// The columns of whitespace after the last line break of the source.
    fn trailing_indent(&self) -> usize {
        let mut columns = 0;
        for c in self.text.iter().rev() {
            match c {
                ' ' => columns += 1,
                '\t' => columns += 4,
                _ => break,
            }
        }
        columns
    }

    /// The tokens from here to the end of the line, as `(first, after_last)`.
    fn rest_of_line(&self) -> (usize, usize) {
        let first = self.pos;
        let mut after = first;
        while after < self.toks.len() && self.toks[after] != Tok::Line {
            after += 1;
        }
        (first, after)
    }

    /// The rest of the line, as typed, kept as text (lenient parse).
    fn garbage_line(&mut self) -> Expr {
        let (first, after) = self.rest_of_line();
        let start = self.start_of(first);
        let end = if after > first {
            self.spans[after - 1].end
        } else {
            start
        };
        self.pos = after;
        self.done(start, end);
        Expr::Garbage {
            head: None,
            text: self.text[start..end].iter().collect(),
        }
    }

    /// `head`, then whatever else is on its line, kept as text (lenient parse).
    fn garbage_to_line_end(&mut self, head: Expr) -> Expr {
        let from = self.last_node().start;
        let (first, after) = self.rest_of_line();
        let start = self.start_of(first);
        let end = self.spans[after - 1].end;
        self.pos = after;
        self.done(from, end);
        Expr::Garbage {
            head: Some(Box::new(head)),
            text: self.text[start..end].iter().collect(),
        }
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
