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
            let line_start = self.start_of(line);
            match self
                .expr(0)
                .and_then(|e| self.guess_or(e, mark, line_start))
            {
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

    /// A line of a `given` block that is `x := value` is a guess for `x`; anything
    /// else is `e`, a constraint. `mark` is where the line's log entries start.
    fn guess_or(&mut self, e: Expr, mark: usize, from: usize) -> Result<Expr, String> {
        if self.peek() != Some(&Tok::Define) {
            return Ok(e);
        }
        let Expr::Name(target) = e else {
            return Err("left of ':=' must be a name".into());
        };
        let name_end = self.log.len();
        self.pos += 1;
        let value = self.expr(0)?;
        // The name is not a child: only the value is.
        self.log.drain(mark..name_end);
        self.done(from, self.last_node().end);
        Ok(Expr::Assign {
            kind: crate::expr::AssignKind::Define,
            target,
            value: Box::new(value),
        })
    }

    /// The indented block that follows, whose lines are indented further than
    /// `parent`. `from` is where the block is taken to start, if not at its first
    /// statement. The block is a `Program` node.
    pub(super) fn block(&mut self, parent: usize, from: Option<usize>) -> Result<Expr, String> {
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
    pub(super) fn next_char(&self) -> usize {
        self.spans.get(self.pos).map_or(self.end, |s| s.start)
    }

    /// The columns of whitespace after the last line break of the source.
    pub(super) fn trailing_indent(&self) -> usize {
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
    pub(super) fn rest_of_line(&self) -> (usize, usize) {
        let first = self.pos;
        let mut after = first;
        while after < self.toks.len() && self.toks[after] != Tok::Line {
            after += 1;
        }
        (first, after)
    }

    /// The rest of the line, as typed, kept as text (lenient parse).
    pub(super) fn garbage_line(&mut self) -> Expr {
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
    pub(super) fn garbage_to_line_end(&mut self, head: Expr) -> Expr {
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
    pub(super) fn at_statement_end(&self) -> bool {
        matches!(
            self.peek(),
            None | Some(Tok::Line | Tok::RParen | Tok::Comma | Tok::RBracket | Tok::Semicolon)
        )
    }

    pub(super) fn word(&self, at: usize, word: &str) -> bool {
        matches!(self.toks.get(at), Some(Tok::Ident(w)) if w == word)
    }
}
