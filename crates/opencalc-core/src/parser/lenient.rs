//! The lenient parse: the editor's display tree for text that is still being
//! typed (docs/EDITOR.md section 4.2). It reuses the strict parser with `lenient`
//! set, so the two cannot drift: the same code runs, and only the failure branches
//! differ.

use super::{parse_inner, spans, Parser, SpanNode};
use crate::expr::Expr;

/// Parse for display while the text is still being typed. It never fails: a
/// missing operand becomes `Expr::Hole`, an unclosed bracket is closed at the end,
/// and text it cannot use is kept as `Expr::Garbage`. For source that `parse`
/// accepts the tree is the same. The result is for drawing and editing only; it is
/// never evaluated or saved (`parse` is used for that).
pub fn parse_lenient(src: &str) -> (Expr, SpanNode) {
    let chars = src.chars().count();
    let whole = || {
        let text = src.to_owned();
        (Expr::Garbage { head: None, text }, SpanNode::leaf(0, chars))
    };
    let (expr, log) = parse_inner(src, true);
    if matches!(expr, Expr::Error(_)) {
        return whole();
    }
    let mut entries = log.into_iter();
    match spans::rebuild(&expr, &mut entries) {
        Some(spans) if entries.next().is_none() => (expr, spans),
        _ => whole(),
    }
}

impl Parser {
    /// Where a missing closing bracket is taken to be: after the last token and
    /// after everything recorded so far (holes can sit past the last token).
    pub(super) fn close_end(&self) -> usize {
        let last = if self.pos > 0 { self.last_end() } else { 0 };
        last.max(self.last_node().end)
    }

    /// A missing operand (lenient parse): an empty node at the next token, or at
    /// the end of the source. Consumes nothing.
    pub(super) fn hole(&mut self) -> Expr {
        let at = self.spans.get(self.pos).map_or(self.end, |s| s.start);
        self.done(at, at);
        Expr::Hole
    }

    /// Keep what parsed so far when the left of `=` or `:=` is not a valid target
    /// (lenient), or fail with the message.
    pub(super) fn keep_or_fail(&self, message: String, kept: Expr) -> Result<Expr, String> {
        if self.lenient {
            Ok(kept)
        } else {
            Err(message)
        }
    }

    /// Everything from the next token to the end, as `Garbage` after `head`.
    pub(super) fn garbage_tail(&mut self, head: Expr) -> Expr {
        let from = self.last_node().start;
        let start = self.spans.get(self.pos).map_or(self.end, |s| s.start);
        let text: String = self.text[start.min(self.end)..].iter().collect();
        self.pos = self.toks.len();
        self.done(from, self.end);
        Expr::Garbage {
            head: Some(Box::new(head)),
            text,
        }
    }
}
