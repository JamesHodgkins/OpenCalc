//! Caret stops: the places in a typeset source where a caret can stand.

use std::collections::HashMap;

use eframe::egui::Rect;
use opencalc_core::expr::Expr;
use opencalc_core::parser::{Span, SpanNode};

use super::{key_of, MarkKind};
use crate::layout::Layout;

/// A source offset with a known place.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stop {
    pub(super) offset: usize,
    pub(super) x: f32,
    pub(super) top: f32,
    pub(super) bottom: f32,
}

impl Stop {
    pub(super) fn at(offset: usize, x: f32, rect: Rect) -> Stop {
        Stop {
            offset,
            x,
            top: rect.top(),
            bottom: rect.bottom(),
        }
    }

    pub(super) fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

pub(super) fn collect(e: &Expr, s: &SpanNode, out: &mut HashMap<usize, (Span, Span)>) {
    out.insert(key_of(e), (s.outer, s.inner));
    for (child, span) in e.children().into_iter().zip(&s.kids) {
        collect(child, span, out);
    }
}

/// The stops of a laid-out source, sorted by offset. At one offset the stop on the
/// smallest rectangle (the deepest node) is kept.
pub(super) fn stops(layout: &Layout, spans: &HashMap<usize, (Span, Span)>) -> Vec<Stop> {
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
pub(super) fn indent_carets(src: &str, carets: &mut [Stop]) {
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

pub(super) fn caret_at(stops: &[Stop], offset: usize, fallback_height: f32) -> Stop {
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
