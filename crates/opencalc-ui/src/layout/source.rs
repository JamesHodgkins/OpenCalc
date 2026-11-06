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

use eframe::egui::{epaint::text::Fonts, Color32, Pos2, Rect, Vec2};
use opencalc_core::expr::Expr;
use opencalc_core::parser::{parse_lenient, Span, SpanNode};

use super::{Layout, Typesetter};

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
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(layout.w, layout.h)),
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
        let (expr, spans) = parse_lenient(src);
        let layout = Typesetter::new(fonts, colour)
            .with_marks()
            .layout(&expr, size);
        let mut spans_of = HashMap::new();
        collect(&expr, &spans, &mut spans_of);
        let stops = stops(&layout, &spans_of);
        let chars = src.chars().count();
        let carets = (0..=chars).map(|o| caret_at(&stops, o, layout.h)).collect();
        SourceLayout {
            layout,
            carets,
            spans,
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

    fn everything() -> Vec<String> {
        let mut sources = corpus_sources();
        assert!(sources.len() > 300, "{} corpus lines", sources.len());
        sources.extend(SHAPES.iter().map(|s| (*s).to_owned()));
        sources
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
