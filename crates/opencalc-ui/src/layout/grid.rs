//! Typesetting for matrices, indexing and conditionals.

use eframe::egui::Vec2;
use opencalc_core::expr::Expr;

use super::{Layout, Op, Typesetter, AXIS, SCRIPT};

impl Typesetter<'_> {
    /// A matrix as a grid of cells between tall brackets. Columns line up, and
    /// cells in a row share a baseline.
    pub(super) fn matrix(&self, rows: usize, cols: usize, cells: &[Expr], size: f32) -> Layout {
        let laid: Vec<Layout> = cells.iter().map(|c| self.layout(c, size)).collect();
        let col_gap = size * 0.9;
        let row_gap = size * 0.3;
        let width_of = |c: usize| {
            (0..rows)
                .map(|r| laid[r * cols + c].w)
                .fold(0.0_f32, f32::max)
        };
        let above_of = |r: usize| {
            (0..cols)
                .map(|c| laid[r * cols + c].baseline)
                .fold(0.0_f32, f32::max)
        };
        let below_of = |r: usize| {
            (0..cols)
                .map(|c| laid[r * cols + c].h - laid[r * cols + c].baseline)
                .fold(0.0_f32, f32::max)
        };
        let widths: Vec<f32> = (0..cols).map(width_of).collect();
        let pad = size * 0.3;
        let inner_w = widths.iter().sum::<f32>() + col_gap * (cols.saturating_sub(1)) as f32;
        let inner_h = (0..rows).map(|r| above_of(r) + below_of(r)).sum::<f32>()
            + row_gap * (rows.saturating_sub(1)) as f32;
        let (w, h) = (inner_w + 2.0 * pad + size * 0.4, inner_h + size * 0.2);
        let mut out = Layout {
            w,
            h,
            baseline: h / 2.0 + size * AXIS,
            ops: Vec::new(),
            marks: Vec::new(),
        };
        let (mut y, mut slots) = (size * 0.1, Vec::new());
        for r in 0..rows {
            let (above, below) = (above_of(r), below_of(r));
            let mut x = pad + size * 0.2;
            for (c, width) in widths.iter().enumerate() {
                slots.push((
                    r * cols + c,
                    Vec2::new(x + (width - laid[r * cols + c].w) / 2.0, y + above),
                ));
                x += width + col_gap;
            }
            y += above + below + row_gap;
        }
        for (slot, at) in slots {
            let cell = &laid[slot];
            let top_left = at - Vec2::new(0.0, cell.baseline);
            out.copy_into(cell, top_left);
        }
        out.brackets(size);
        out
    }

    /// `base[i, j]` written as a subscript: the base with its indices lowered.
    pub(super) fn index(&self, base: &Expr, indices: &[Expr], size: f32) -> Layout {
        let b = self.operand(base, size, 6);
        let mut parts = Vec::new();
        for (i, index) in indices.iter().enumerate() {
            if i > 0 {
                parts.push(self.text(", ", size * SCRIPT));
            }
            parts.push(self.layout(index, size * SCRIPT));
        }
        let sub = self.row(parts);
        let drop = size * 0.25;
        let (bw, bh, bb) = (b.w, b.h, b.baseline);
        let sub_top = bb + drop - sub.baseline;
        let h = bh.max(sub_top + sub.h);
        let mut out = Layout {
            w: bw + sub.w,
            h,
            baseline: bb,
            ops: Vec::new(),
            marks: Vec::new(),
        };
        out.absorb(b, Vec2::ZERO, false);
        out.absorb(sub, Vec2::new(bw, sub_top), false);
        out
    }

    /// `a..b`, or `a, b..c` when there is a step.
    pub(super) fn range(&self, start: &Expr, step: Option<&Expr>, end: &Expr, size: f32) -> Layout {
        let mut parts = vec![self.layout(start, size)];
        if let Some(step) = step {
            parts.push(self.text(", ", size));
            parts.push(self.layout(step, size));
        }
        parts.push(self.gap(size * 0.1));
        parts.push(self.text("..", size));
        parts.push(self.gap(size * 0.1));
        parts.push(self.layout(end, size));
        self.row(parts)
    }

    /// `name(a, b, c)`.
    pub(super) fn call_like(&self, name: &str, args: &[&Expr], size: f32) -> Layout {
        let mut parts = vec![self.text(name, size), self.text("(", size)];
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                parts.push(self.text(", ", size));
            }
            parts.push(self.layout(a, size));
        }
        parts.push(self.text(")", size));
        self.row(parts)
    }
}

impl Layout {
    /// Add a copy of `child`'s ops at `at` without consuming it.
    fn copy_into(&mut self, child: &Layout, at: Vec2) {
        for op in &child.ops {
            self.ops.push(match op {
                Op::Text { at: p, galley } => Op::Text {
                    at: *p + at,
                    galley: galley.clone(),
                },
                Op::Line { a, b } => Op::Line {
                    a: *a + at,
                    b: *b + at,
                },
            });
        }
        self.marks
            .extend(child.marks.iter().map(|m| m.translated(at)));
    }

    /// Square brackets down both sides of the whole box.
    fn brackets(&mut self, size: f32) {
        let (w, h, tick) = (self.w, self.h, size * 0.22);
        let edge = size * 0.06;
        for (x, dir) in [(edge, 1.0_f32), (w - edge, -1.0)] {
            self.ops.push(Op::Line {
                a: Vec2::new(x, 0.0),
                b: Vec2::new(x, h),
            });
            for y in [0.0, h] {
                self.ops.push(Op::Line {
                    a: Vec2::new(x, y),
                    b: Vec2::new(x + dir * tick, y),
                });
            }
        }
    }
}
