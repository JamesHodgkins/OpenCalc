//! Where each part of an expression came from in the source text.
//!
//! The parser builds a [`SpanNode`] tree beside every `Expr`. It has exactly the
//! same shape (a node's `kids` are the spans of `Expr::children()`, in order), so a
//! node is found with the same path that `Expr::at_path` and the evaluator's error
//! paths use. Offsets count characters, not bytes, like `ParseError`.

/// A half-open range of characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn contains_span(&self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

/// The source range of one node, and of the nodes inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanNode {
    /// The node itself, including any parentheses written around it. The
    /// characters of an enclosing `( )` belong to the node they wrap, so they sit
    /// between `outer` and `inner`.
    pub outer: Span,
    /// The node without those parentheses.
    pub inner: Span,
    /// One per child, in the order of `Expr::children()`.
    pub kids: Vec<SpanNode>,
}

impl SpanNode {
    pub fn new(start: usize, end: usize, kids: Vec<SpanNode>) -> SpanNode {
        let span = Span::new(start, end);
        SpanNode {
            outer: span,
            inner: span,
            kids,
        }
    }

    pub fn leaf(start: usize, end: usize) -> SpanNode {
        SpanNode::new(start, end, Vec::new())
    }

    /// The same node, now written inside a pair of parentheses.
    pub fn parenthesised(mut self, open: usize, close: usize) -> SpanNode {
        self.outer = Span::new(open, close);
        self
    }

    /// The node at `path`, a list of child numbers from this one.
    pub fn at_path(&self, path: &[u16]) -> Option<&SpanNode> {
        match path.split_first() {
            None => Some(self),
            Some((first, rest)) => self.kids.get(usize::from(*first))?.at_path(rest),
        }
    }
}

/// One node as the parser records it, before the tree is rebuilt.
#[derive(Clone, Copy, Debug)]
pub(super) struct Entry {
    pub(super) outer: Span,
    pub(super) inner: Span,
}

impl Entry {
    pub(super) fn new(start: usize, end: usize) -> Entry {
        let span = Span::new(start, end);
        Entry {
            outer: span,
            inner: span,
        }
    }
}

/// Rebuild the span tree for `expr` from the parser's log. The log lists each node
/// after its children, which is the order of a post-order walk of the tree, so the
/// walk consumes it exactly. `None` if the log is too short.
pub(super) fn rebuild(
    expr: &crate::expr::Expr,
    entries: &mut std::vec::IntoIter<Entry>,
) -> Option<SpanNode> {
    let kids = expr
        .children()
        .into_iter()
        .map(|child| rebuild(child, entries))
        .collect::<Option<Vec<_>>>()?;
    let entry = entries.next()?;
    Some(SpanNode {
        outer: entry.outer,
        inner: entry.inner,
        kids,
    })
}
