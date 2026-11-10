//! Anchors: where every character of a region's source sits in its typeset form
//! (docs/EDITOR.md section 4.3).
//!
//! The typesetter, when asked to mark, records the rectangle of every expression
//! node it draws (and of the brackets it adds around one). The lenient parse says
//! which source characters each node covers. Putting the two together gives *stops*:
//! source offsets whose place on screen is known (the edges of every node, and every
//! letter of a name). Offsets between stops (operators, brackets, spaces, the text
//! of a signature) are interpolated along the line, or snap to the nearer stop when
//! the two stops are on different lines (a fraction bar, an exponent).
//!
//! `caret_rect` and `offset_at` are pure: they need no window, only the fonts the
//! layout was made with.

use std::collections::HashMap;

use eframe::egui::{epaint::text::Fonts, text::CCursor, Color32, FontId, Pos2, Rect, Vec2};
use opencalc_core::expr::Expr;
use opencalc_core::parser::{parse_lenient, Span, SpanNode};

use super::{Layout, Op, Typesetter};

/// What a recorded rectangle stands for.
#[derive(Clone, Debug)]
pub enum MarkKind {
    /// A node of the expression.
    Node,
    /// The node together with the brackets the typesetter put around it.
    Paren,
    /// A missing operand, drawn as a box.
    Hole,
    /// A name, with the x offset of every letter boundary from its left edge.
    Glyphs(Vec<f32>),
}

/// A rectangle (relative to the layout's top left) recorded for the node at `key`.
#[derive(Clone, Debug)]
pub struct Mark {
    pub key: usize,
    pub rect: Rect,
    pub kind: MarkKind,
}

/// Identifies a node while a layout is being made. Only compared, never followed.
pub fn key_of(e: &Expr) -> usize {
    e as *const Expr as usize
}

impl Mark {
    pub fn of(e: &Expr, layout: &Layout, kind: MarkKind) -> Mark {
        Mark {
            key: key_of(e),
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(layout.w, layout.caret_height())),
            kind,
        }
    }

    pub fn translated(&self, by: Vec2) -> Mark {
        Mark {
            rect: self.rect.translate(by),
            ..self.clone()
        }
    }
}

/// A source offset with a known place.
#[derive(Clone, Copy, Debug)]
struct Stop {
    offset: usize,
    x: f32,
    top: f32,
    bottom: f32,
}

impl Stop {
    fn at(offset: usize, x: f32, rect: Rect) -> Stop {
        Stop {
            offset,
            x,
            top: rect.top(),
            bottom: rect.bottom(),
        }
    }

    fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

/// A region's source typeset, with a caret position for every offset.
pub struct SourceLayout {
    pub layout: Layout,
    /// Caret positions, one per offset `0..=chars`.
    carets: Vec<Stop>,
    spans: SpanNode,
}

impl SourceLayout {
    pub fn new(fonts: &Fonts, colour: Color32, src: &str, size: f32) -> SourceLayout {
        SourceLayout::showing(fonts, colour, src, size, |e| e)
    }

    /// Like `new`, but typesets only the part of the expression `select` picks (the
    /// whole one, or one of its direct children, as a region that shows `x` for
    /// `x =` does). Offsets outside that part collapse onto its edges.
    pub fn showing(
        fonts: &Fonts,
        colour: Color32,
        src: &str,
        size: f32,
        select: impl for<'e> Fn(&'e Expr) -> &'e Expr,
    ) -> SourceLayout {
        let (expr, spans) = parse_lenient(src);
        // A program the lenient parse could not make a tree of (it does not fail on
        // lines, only on something like a source that is too long) is shown as typed.
        if src.contains('\n') && matches!(expr, Expr::Garbage { head: None, .. }) {
            return SourceLayout::as_typed(fonts, colour, src, size);
        }
        let shown = select(&expr);
        let shown_spans = if std::ptr::eq(shown, &expr) {
            &spans
        } else {
            expr.children()
                .iter()
                .position(|c| std::ptr::eq(*c, shown))
                .and_then(|i| spans.kids.get(i))
                .unwrap_or(&spans)
        };
        let layout = Typesetter::new(fonts, colour)
            .with_marks()
            .layout(shown, size);
        let mut spans_of = HashMap::new();
        collect(shown, shown_spans, &mut spans_of);
        let stops = stops(&layout, &spans_of);
        let chars = src.chars().count();
        let mut carets: Vec<Stop> = (0..=chars).map(|o| caret_at(&stops, o, layout.h)).collect();
        indent_carets(src, &mut carets);
        SourceLayout {
            layout,
            carets,
            spans,
        }
    }

    /// Lines of text as typed, in a fixed-width face so the indentation lines up.
    /// Every offset has a caret from the text itself, so none of the node
    /// bookkeeping the formulas need is involved.
    fn as_typed(fonts: &Fonts, colour: Color32, src: &str, size: f32) -> SourceLayout {
        let galley = fonts.layout_no_wrap(src.to_owned(), FontId::monospace(size), colour);
        let chars = src.chars().count();
        let carets = (0..=chars)
            .map(|offset| {
                let c = galley.pos_from_ccursor(CCursor::new(offset));
                Stop {
                    offset,
                    x: c.min.x,
                    top: c.min.y,
                    bottom: c.max.y,
                }
            })
            .collect();
        let r = galley.rect;
        let layout = Layout {
            w: r.width(),
            h: r.height(),
            baseline: r.height() * 0.78,
            ops: vec![Op::Text {
                at: Vec2::ZERO,
                galley,
            }],
            marks: Vec::new(),
            first_line: 0.0,
        };
        SourceLayout {
            layout,
            carets,
            spans: SpanNode::leaf(0, chars),
        }
    }

    /// The number of characters of the source.
    pub fn chars(&self) -> usize {
        self.carets.len() - 1
    }

    /// The caret at `offset` (clamped to the source): a vertical segment, so zero wide.
    pub fn caret_rect(&self, offset: usize) -> Rect {
        let c = &self.carets[offset.min(self.chars())];
        Rect::from_min_max(Pos2::new(c.x, c.top), Pos2::new(c.x, c.bottom))
    }

    /// Rectangles (relative to the layout) covering the characters `a..b`, one per
    /// line of the typeset form. An empty range gives a narrow one at its caret.
    pub fn bands(&self, a: usize, b: usize) -> Vec<Rect> {
        let (a, b) = (a.min(self.chars()), b.min(self.chars()));
        if a >= b {
            let c = self.caret_rect(a);
            return vec![c.expand2(Vec2::new(3.0, 0.0))];
        }
        let mut out: Vec<Rect> = Vec::new();
        for o in a..b {
            let (p, q) = (self.caret_rect(o), self.caret_rect(o + 1));
            if (p.center().y - q.center().y).abs() >= p.height() * 0.5 || q.min.x <= p.min.x {
                continue;
            }
            let band = Rect::from_min_max(
                Pos2::new(p.min.x, p.min.y.min(q.min.y)),
                Pos2::new(q.min.x, p.max.y.max(q.max.y)),
            );
            match out.last_mut() {
                Some(last) if (last.center().y - band.center().y).abs() < band.height() * 0.5 => {
                    *last = last.union(band);
                }
                _ => out.push(band),
            }
        }
        out
    }

    /// The smallest node of the expression that covers the character at `offset`
    /// (or before it, at the end), as `(start, end)`. What a double click selects.
    pub fn enclosing(&self, offset: usize) -> (usize, usize) {
        let at = offset.min(self.chars().saturating_sub(1));
        let mut node = &self.spans;
        loop {
            let next = node
                .kids
                .iter()
                .find(|k| k.inner.start <= at && at < k.inner.end);
            match next {
                Some(k) => node = k,
                None => return (node.inner.start, node.inner.end),
            }
        }
    }

    /// The next larger piece of the expression around the selection `a..b`: the
    /// smallest node range (with or without its brackets) that contains it and is
    /// bigger. With nothing selected, the node under the caret. Repeating it climbs
    /// the expression tree; at the top it returns `(a, b)` unchanged.
    pub fn grow(&self, a: usize, b: usize) -> (usize, usize) {
        if a == b {
            let (s, e) = self.enclosing(a);
            if s < e && s <= a && a <= e {
                return (s, e);
            }
        }
        fn walk(n: &SpanNode, a: usize, b: usize, best: &mut Option<(usize, usize)>) {
            for s in [n.inner, n.outer] {
                let bigger = s.start <= a && b <= s.end && (s.start, s.end) != (a, b);
                let smaller = best.is_none_or(|(x, y)| s.end - s.start < y - x);
                if bigger && smaller {
                    *best = Some((s.start, s.end));
                }
            }
            for k in &n.kids {
                walk(k, a, b, best);
            }
        }
        let mut best = None;
        walk(&self.spans, a, b, &mut best);
        best.unwrap_or((a, b))
    }

    /// The offset whose caret is nearest `point` (relative to the layout's top left).
    /// When several offsets share a place, the first one.
    pub fn offset_at(&self, point: Pos2) -> usize {
        let distance = |c: &Stop| {
            let dy = if point.y < c.top {
                c.top - point.y
            } else if point.y > c.bottom {
                point.y - c.bottom
            } else {
                0.0
            };
            // Among carets equally near, the one centred nearest the point wins.
            let centre = (point.y - (c.top + c.bottom) / 2.0).abs();
            (c.x - point.x).powi(2) + dy.powi(2) + 0.001 * centre.powi(2)
        };
        let mut best = (0, f32::INFINITY);
        for (offset, c) in self.carets.iter().enumerate() {
            let d = distance(c);
            if d < best.1 {
                best = (offset, d);
            }
        }
        best.0
    }
}

fn collect(e: &Expr, s: &SpanNode, out: &mut HashMap<usize, (Span, Span)>) {
    out.insert(key_of(e), (s.outer, s.inner));
    for (child, span) in e.children().into_iter().zip(&s.kids) {
        collect(child, span, out);
    }
}

/// The stops of a laid-out source, sorted by offset. At one offset the stop on the
/// smallest rectangle (the deepest node) is kept.
fn stops(layout: &Layout, spans: &HashMap<usize, (Span, Span)>) -> Vec<Stop> {
    let mut out: Vec<(Stop, f32)> = Vec::new();
    for mark in &layout.marks {
        let Some((outer, inner)) = spans.get(&mark.key) else {
            continue;
        };
        let r = mark.rect;
        let area = r.width() * r.height();
        let mut add = |offset: usize, x: f32| out.push((Stop::at(offset, x, r), area));
        match &mark.kind {
            MarkKind::Node => {
                add(inner.start, r.left());
                add(inner.end, r.right());
            }
            MarkKind::Paren => {
                if outer != inner {
                    add(outer.start, r.left());
                    add(outer.end, r.right());
                }
            }
            MarkKind::Hole => add(inner.start, r.center().x),
            MarkKind::Glyphs(xs) => {
                let len = inner.len();
                let total = xs.last().copied().unwrap_or(r.width());
                for k in 0..=len {
                    let x = if xs.len() == len + 1 {
                        xs[k]
                    } else {
                        total * k as f32 / len.max(1) as f32
                    };
                    add(inner.start + k, r.left() + x);
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.0.offset
            .cmp(&b.0.offset)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    out.dedup_by_key(|(s, _)| s.offset);
    out.into_iter().map(|(s, _)| s).collect()
}

/// In a program, the spaces that start a line are not drawn: a caret among them is
/// where the line's text starts (not out at the end of the line above, which is
/// nearer in the source).
fn indent_carets(src: &str, carets: &mut [Stop]) {
    let chars: Vec<char> = src.chars().collect();
    let mut line_start = 0;
    while line_start <= chars.len() {
        let text_start = line_start
            + chars[line_start..]
                .iter()
                .take_while(|c| **c == ' ')
                .count();
        if text_start > line_start && line_start > 0 {
            let at = carets[text_start];
            for (offset, caret) in carets
                .iter_mut()
                .enumerate()
                .take(text_start)
                .skip(line_start)
            {
                *caret = Stop { offset, ..at };
            }
        }
        match chars[line_start..].iter().position(|c| *c == '\n') {
            Some(i) => line_start += i + 1,
            None => break,
        }
    }
}

/// Two stops are on one line if their vertical extents overlap substantially.
fn same_line(a: &Stop, b: &Stop) -> bool {
    let overlap = a.bottom.min(b.bottom) - a.top.max(b.top);
    overlap >= 0.4 * a.height().min(b.height())
}

fn caret_at(stops: &[Stop], offset: usize, fallback_height: f32) -> Stop {
    let i = stops.partition_point(|s| s.offset < offset);
    if let Some(s) = stops.get(i).filter(|s| s.offset == offset) {
        return *s;
    }
    let before = i.checked_sub(1).map(|j| stops[j]);
    let after = stops.get(i).copied();
    match (before, after) {
        (Some(p), Some(q)) => {
            let t = (offset - p.offset) as f32 / (q.offset - p.offset) as f32;
            let nearer = if t <= 0.5 { p } else { q };
            if same_line(&p, &q) {
                Stop {
                    offset,
                    x: p.x + (q.x - p.x) * t,
                    ..nearer
                }
            } else {
                Stop { offset, ..nearer }
            }
        }
        (Some(s), None) | (None, Some(s)) => Stop { offset, ..s },
        (None, None) => Stop {
            offset,
            x: 0.0,
            top: 0.0,
            bottom: fallback_height,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    /// Run `f` with real fonts, as in a live frame.
    fn with_fonts<R>(f: impl FnOnce(&Fonts) -> R) -> R {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut result = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                if let Some(f) = f.take() {
                    result = Some(f(fonts));
                }
            });
        });
        result.expect("closure ran")
    }

    fn lay(fonts: &Fonts, src: &str) -> SourceLayout {
        SourceLayout::new(fonts, Color32::WHITE, src, 16.0)
    }

    /// The expressions of every corpus file, one per line.
    fn corpus_sources() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).expect("the corpus folder") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e == "calc") {
                let text = std::fs::read_to_string(&path).expect("a corpus file");
                for line in text.lines() {
                    if let Some(rest) = line.strip_prefix("=> ") {
                        if let Some((_, expr)) = rest.split_once(" = ") {
                            out.push(expr.to_owned());
                        }
                    } else if !(line.starts_with('#')
                        || line.starts_with("!>")
                        || line.starts_with('@')
                        || line.trim().is_empty())
                    {
                        out.push(line.to_owned());
                    }
                }
            }
        }
        out
    }

    const SHAPES: &[&str] = &[
        "a / b",
        "(a + b) / c",
        "x ^ 2",
        "x ^ (n + 1)",
        "sqrt(x ^ 2 + 1)",
        "[1, 2; 3, 4]",
        "v[i + 1]",
        "M[0, 1]",
        "f(x, y) := x * y",
        "y := if(a > b, a, b)",
        "0, 2..10",
        "5 kN / 2 m",
        "(1 + 2) * 3",
        "((x))",
        "F_x + sigma_y",
        "_a * b_",
        "v_1[i_2]",
        "M_ab := 1",
        "\u{3b1} + \u{3b2}",
        "x =",
        "1 +",
        "* 2",
        "(1 +",
        "1 $ 2",
        "",
    ];

    /// Programs, as they are typed (several lines).
    const PROGRAMS: &[&str] = &[
        "r := given\n    x + y == 10\n    x - y == 2\nfind(x, y)",
        "given\n    a ^ 2 == 9\n    a > 0\nfind(a)",
        "m := given\nminimize((a - 3)^2, a)",
        "p := program\n    q \u{2190} given\n        a == 1\n    find(a)\n    q",
        "f(n) := program\n    s \u{2190} 0\n    for i in 1..n\n        s \u{2190} s + i\n    return s",
        "g := program\n    if x > 1\n        a \u{2190} 1\n    else\n        a \u{2190} 2\n    v[1, 2] \u{2190} a\n    a",
        "h := program\n    try\n        a \u{2190} 1 / 0\n    on error e\n        a \u{2190} e\n    while 1\n        break\n    return \"done\"",
        "k := program\n    x \u{2190} 1\n  y \u{2190} 2\n        z \u{2190} 3\n    for i in\n    x +\n    ",
    ];

    /// The program with a `for` loop in it.
    fn the_loop_program() -> &'static str {
        PROGRAMS
            .iter()
            .find(|s| s.starts_with("f(n)"))
            .expect("the loop program")
    }

    fn everything() -> Vec<String> {
        let mut sources = corpus_sources();
        assert!(sources.len() > 300, "{} corpus lines", sources.len());
        sources.extend(SHAPES.iter().map(|s| (*s).to_owned()));
        sources.extend(PROGRAMS.iter().map(|s| (*s).to_owned()));
        sources
    }

    #[test]
    fn a_program_is_edited_in_its_typeset_form_with_a_caret_on_every_line() {
        with_fonts(|fonts| {
            let src = the_loop_program();
            let l = lay(fonts, src);
            let chars: Vec<char> = src.chars().collect();
            // Typeset: much taller than the plain text of one line would be, with
            // the bars of its blocks drawn.
            assert!(l.layout.h > 3.0 * 16.0, "{}", l.layout.h);
            let bars = l
                .layout
                .ops
                .iter()
                .filter(|op| matches!(op, Op::Line { a, b } if a.x == b.x))
                .count();
            assert_eq!(bars, 2);
            // The caret at the start of each line is on a lower line than the
            // line above it, and one line tall (not the height of the whole).
            let mut line_starts = vec![0];
            for (i, c) in chars.iter().enumerate() {
                if *c == '\n' {
                    line_starts.push(i + 1);
                }
            }
            let tops: Vec<f32> = line_starts.iter().map(|o| l.caret_rect(*o).min.y).collect();
            // (The first line of the body is level with `f(n) :=`.)
            assert_eq!(tops[0], tops[1], "{tops:?}");
            for pair in tops[1..].windows(2) {
                assert!(pair[1] > pair[0], "{tops:?}");
            }
            for o in &line_starts {
                assert!(
                    l.caret_rect(*o).height() < 2.0 * 16.0 * 1.5,
                    "offset {o}: {:?}",
                    l.caret_rect(*o)
                );
            }
        });
    }

    #[test]
    fn a_new_line_being_typed_has_its_caret_on_a_line_of_its_own() {
        with_fonts(|fonts| {
            let before = lay(fonts, "f := program\n    s \u{2190} 0");
            let after = lay(fonts, "f := program\n    s \u{2190} 0\n    ");
            let at = "f := program\n    s \u{2190} 0\n    ".chars().count();
            let (old_end, new) = (before.caret_rect(before.chars()), after.caret_rect(at));
            assert!(new.min.y > old_end.min.y, "{new:?} against {old_end:?}");
            assert!(after.layout.h > before.layout.h);
        });
    }

    #[test]
    fn clicking_in_a_program_finds_the_line_that_was_clicked() {
        with_fonts(|fonts| {
            let l = lay(fonts, the_loop_program());
            let chars = the_loop_program().chars().count();
            for o in 0..=chars {
                let c = l.caret_rect(o);
                let found = l.offset_at(c.center());
                let back = l.caret_rect(found);
                assert!(
                    (back.center().y - c.center().y).abs() < 1.0,
                    "offset {o} clicked at {:?} found {found} on another line",
                    c.center()
                );
            }
        });
    }

    /// Every offset of `src` has a caret inside the layout.
    fn check_inside(l: &SourceLayout, src: &str) {
        let (w, h) = (l.layout.w, l.layout.h);
        for o in 0..=l.chars() {
            let c = l.caret_rect(o);
            assert!(
                c.min.x >= -0.5
                    && c.max.x <= w + 0.5
                    && c.min.y >= -0.5
                    && c.max.y <= h + 0.5
                    && c.min.x.is_finite()
                    && c.height() > 0.0,
                "{src:?} offset {o}: caret {c:?} outside {w} x {h}"
            );
        }
    }

    #[test]
    fn every_offset_of_every_source_has_a_caret_inside_the_layout() {
        with_fonts(|fonts| {
            for src in &everything() {
                let l = lay(fonts, src);
                assert_eq!(l.chars(), src.chars().count());
                check_inside(&l, src);
            }
        });
    }

    #[test]
    fn every_prefix_of_every_source_has_carets_inside_the_layout() {
        with_fonts(|fonts| {
            for src in &everything() {
                let chars: Vec<char> = src.chars().collect();
                for n in 0..=chars.len() {
                    let prefix: String = chars[..n].iter().collect();
                    check_inside(&lay(fonts, &prefix), &prefix);
                }
            }
        });
    }

    #[test]
    fn clicking_where_the_caret_is_returns_an_offset_that_draws_there() {
        with_fonts(|fonts| {
            for src in &everything() {
                let l = lay(fonts, src);
                for o in 0..=l.chars() {
                    let c = l.caret_rect(o);
                    let back = l.offset_at(c.center());
                    let there = l.caret_rect(back);
                    assert!(
                        (there.min.x - c.min.x).abs() < 0.01
                            && (there.center().y - c.center().y).abs() < 0.01,
                        "{src:?}: offset {o} at {c:?} came back as {back} at {there:?}"
                    );
                }
            }
        });
    }

    #[test]
    fn walking_along_a_one_line_source_moves_the_caret_right_without_going_back() {
        with_fonts(|fonts| {
            for src in [
                "1 + 2 * 3",
                "alpha + beta - gamma",
                "sin(x) + cos(y)",
                "f(x, y) := x * y",
                "sigma := F * A",
                "5 kN + 2 m",
                "(a + b) * (c - d)",
            ] {
                let l = lay(fonts, src);
                let xs: Vec<f32> = (0..=l.chars()).map(|o| l.caret_rect(o).min.x).collect();
                for pair in xs.windows(2) {
                    assert!(pair[1] >= pair[0] - 0.01, "{src:?} goes back: {xs:?}");
                }
                assert!(xs[xs.len() - 1] > xs[0] + 20.0, "{src:?} does not advance");
                let distinct = xs.windows(2).filter(|p| p[1] > p[0] + 0.1).count();
                assert!(distinct >= l.chars() / 2, "{src:?}: {xs:?}");
            }
        });
    }

    #[test]
    fn the_caret_moves_letter_by_letter_in_a_name() {
        with_fonts(|fonts| {
            let l = lay(fonts, "abc");
            let x = |o| l.caret_rect(o).min.x;
            assert!(x(0) < x(1) && x(1) < x(2) && x(2) < x(3));
            assert!(
                (x(3) - l.layout.w).abs() < 0.5,
                "the end caret is at the right edge"
            );
            // A click past the end gives the end; before the start gives 0.
            assert_eq!(l.offset_at(Pos2::new(l.layout.w + 30.0, 5.0)), 3);
            assert_eq!(l.offset_at(Pos2::new(-30.0, 5.0)), 0);
        });
    }

    #[test]
    fn a_numerator_and_a_denominator_have_carets_on_different_lines() {
        with_fonts(|fonts| {
            let l = lay(fonts, "a / b");
            let top = l.caret_rect(0).center().y;
            let bottom = l.caret_rect(5).center().y;
            assert!(bottom > top + 10.0, "{top} {bottom}");
            let mid = l.layout.h / 2.0;
            assert!(l.offset_at(Pos2::new(l.layout.w / 2.0, mid + 12.0)) >= 4);
            assert!(l.offset_at(Pos2::new(l.layout.w / 2.0, mid - 12.0)) <= 1);
        });
    }

    #[test]
    fn an_exponent_is_higher_than_its_base() {
        with_fonts(|fonts| {
            let l = lay(fonts, "x ^ 2");
            assert!(l.caret_rect(4).center().y < l.caret_rect(1).center().y);
        });
    }

    #[test]
    fn a_hole_has_a_caret_on_its_box() {
        with_fonts(|fonts| {
            let l = lay(fonts, "1 +");
            let (end, before) = (l.caret_rect(3), l.caret_rect(1));
            // The box is drawn after the plus, so the caret at the end is past it.
            assert!(end.min.x > before.min.x + 10.0);
            assert!(end.min.x <= l.layout.w);
        });
    }

    #[test]
    fn typed_brackets_are_not_skipped() {
        with_fonts(|fonts| {
            let l = lay(fonts, "(a + b) * c");
            // Offset 0 is before the opening bracket, 1 after it.
            assert!(l.caret_rect(0).min.x < l.caret_rect(1).min.x);
            // Offset 7 is after the closing bracket.
            assert!(l.caret_rect(7).min.x > l.caret_rect(6).min.x);
            // The bracket the typesetter adds sits between offsets 9 and 10 here, so
            // the caret moves by exactly the width of a bracket across it.
            let l = lay(fonts, "x := c * (a + b)");
            let paren = fonts
                .layout_no_wrap("(".into(), egui::FontId::proportional(16.0), Color32::WHITE)
                .rect
                .width();
            let step = l.caret_rect(10).min.x - l.caret_rect(9).min.x;
            assert!((step - paren).abs() < 0.5, "step {step}, bracket {paren}");
        });
    }

    #[test]
    fn double_click_selects_the_smallest_node_under_the_caret() {
        with_fonts(|fonts| {
            let l = lay(fonts, "alpha + (b * c)");
            assert_eq!(l.enclosing(2), (0, 5), "a name");
            assert_eq!(l.enclosing(11), (9, 14), "a node inside brackets");
            assert_eq!(
                l.enclosing(7),
                (0, 15),
                "between the operands: the whole sum"
            );
            assert_eq!(l.enclosing(15), (0, 15), "the end of the source");
        });
    }

    #[test]
    fn unspaced_source_is_drawn_exactly_like_spaced_source() {
        with_fonts(|fonts| {
            for (tight, loose) in [
                ("x+1", "x + 1"),
                ("q:=7*a/b", "q := 7 * a / b"),
                ("f(x,y)^2-3", "f(x, y)^2 - 3"),
                ("a<=b", "a <= b"),
            ] {
                let (a, b) = (lay(fonts, tight), lay(fonts, loose));
                assert!(
                    (a.layout.w - b.layout.w).abs() < 0.5 && (a.layout.h - b.layout.h).abs() < 0.5,
                    "{tight}: {}x{} vs {loose}: {}x{}",
                    a.layout.w,
                    a.layout.h,
                    b.layout.w,
                    b.layout.h
                );
            }
        });
    }

    #[test]
    fn grow_climbs_the_expression_one_level_at_a_time_and_stops_at_the_top() {
        with_fonts(|fonts| {
            let l = lay(fonts, "alpha + (b * c)");
            assert_eq!(l.grow(11, 11), (9, 14), "from the caret, the node under it");
            assert_eq!(l.grow(9, 14), (8, 15), "then the node with its brackets");
            assert_eq!(l.grow(8, 15), (0, 15), "then the whole sum");
            assert_eq!(l.grow(0, 15), (0, 15), "nothing above the top");
            assert_eq!(lay(fonts, "").grow(0, 0), (0, 0), "an empty source");
        });
    }

    #[test]
    fn grow_always_returns_a_range_that_contains_the_selection() {
        with_fonts(|fonts| {
            for src in [
                "a / b",
                "x ^ (2 + 1)",
                "sqrt(a + b) * [1, 2]",
                "f(x, y) +",
                "(1 +",
            ] {
                let l = lay(fonts, src);
                let n = src.chars().count();
                for a in 0..=n {
                    for b in a..=n {
                        let (s, e) = l.grow(a, b);
                        assert!(s <= a && b <= e && e <= n, "{src}: {a}..{b} -> {s}..{e}");
                    }
                }
            }
        });
    }

    #[test]
    fn a_subscript_is_lowered_and_the_underscore_draws_nothing() {
        with_fonts(|fonts| {
            let plain = lay(fonts, "Fx");
            let sub = lay(fonts, "F_x");
            assert!(sub.layout.h > plain.layout.h, "the subscript hangs below");
            let x = |o| sub.caret_rect(o).min.x;
            assert!(x(0) < x(1), "F is drawn");
            assert!(
                (x(1) - x(2)).abs() < 0.01,
                "both sides of the underscore are one place"
            );
            assert!(x(3) > x(2), "x is drawn");
            // The subscript is smaller than the base: narrower than a full-size x.
            let full = lay(fonts, "x").layout.w;
            assert!(x(3) - x(2) < full);
            // A leading or trailing underscore is an ordinary character.
            let lead = lay(fonts, "_a");
            assert!(lead.caret_rect(1).min.x > lead.caret_rect(0).min.x);
            let trail = lay(fonts, "a_");
            assert!(trail.caret_rect(2).min.x > trail.caret_rect(1).min.x);
        });
    }

    #[test]
    fn an_empty_region_has_one_caret() {
        with_fonts(|fonts| {
            let l = lay(fonts, "");
            assert_eq!(l.chars(), 0);
            assert_eq!(l.offset_at(Pos2::new(500.0, 500.0)), 0);
            let c = l.caret_rect(0);
            assert!(c.height() > 0.0 && c.min.x >= 0.0);
        });
    }
}
