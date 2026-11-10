//! Typesetting for programs: a bar down the left of each block, and the lines of
//! the block stacked beside it, nested blocks indented under the line that opens
//! them (docs/PROGRAMMING.md).

use eframe::egui::Vec2;
use opencalc_core::expr::Expr;

use super::{Layout, Op, Typesetter};

/// How far a line is set in from the bar, and a nested block from its line.
const INDENT: f32 = 0.6;
/// Space between lines, as a share of the font size.
const LINE_GAP: f32 = 0.25;

impl Typesetter<'_> {
    /// Lines from top to bottom, each at its own x. The baseline is the first line's,
    /// so a program sits level with the `f(x) :=` before it.
    fn stack(&self, lines: Vec<(f32, Layout)>, gap: f32) -> Layout {
        let mut out = Layout {
            w: 0.0,
            h: 0.0,
            baseline: 0.0,
            ops: Vec::new(),
            marks: Vec::new(),
            first_line: 0.0,
        };
        let mut y = 0.0;
        for (i, (x, line)) in lines.into_iter().enumerate() {
            if i == 0 {
                out.baseline = line.baseline;
                out.first_line = line.caret_height();
            } else {
                y += gap;
            }
            out.w = out.w.max(x + line.w);
            let height = line.h;
            out.absorb(line, Vec2::new(x, y), false);
            y += height;
        }
        out.h = y;
        out
    }

    /// A block of statements with a bar down its left side.
    pub(super) fn program(&self, statements: &[Expr], size: f32) -> Layout {
        let lines = statements
            .iter()
            .map(|s| (size * INDENT, self.layout(s, size)))
            .collect();
        let mut block = self.stack(lines, size * LINE_GAP);
        let bar = size * 0.2;
        block.ops.push(Op::Line {
            a: Vec2::new(bar, 0.0),
            b: Vec2::new(bar, block.h),
        });
        block
    }

    /// A statement: its line, and under it any blocks it holds.
    pub(super) fn statement(&self, e: &Expr, size: f32) -> Layout {
        let word = |s: &str| self.text(s, size);
        let space = || self.gap(size * 0.3);
        let indent = size * INDENT;
        match e {
            Expr::LocalAssign { target, value } => self.row(vec![
                self.name(target, size),
                space(),
                word("\u{2190}"),
                space(),
                self.layout(value, size),
            ]),
            Expr::IndexAssign {
                target,
                indices,
                value,
            } => {
                let mut parts = vec![self.name(target, size), word("[")];
                for (k, i) in indices.iter().enumerate() {
                    if k > 0 {
                        parts.push(word(", "));
                    }
                    parts.push(self.layout(i, size));
                }
                parts.extend([
                    word("]"),
                    space(),
                    word("←"),
                    space(),
                    self.layout(value, size),
                ]);
                self.row(parts)
            }
            Expr::IfBlock {
                cond,
                then,
                otherwise,
            } => {
                let header = self.row(vec![word("if"), space(), self.layout(cond, size)]);
                let mut lines = vec![(0.0, header), (indent, self.layout(then, size))];
                if let Some(o) = otherwise {
                    lines.push((0.0, word("else")));
                    lines.push((indent, self.layout(o, size)));
                }
                self.stack(lines, size * LINE_GAP)
            }
            Expr::For { var, over, body } => {
                let header = self.row(vec![
                    word("for"),
                    space(),
                    self.name(var, size),
                    space(),
                    word("in"),
                    space(),
                    self.layout(over, size),
                ]);
                self.stack(
                    vec![(0.0, header), (indent, self.layout(body, size))],
                    size * LINE_GAP,
                )
            }
            Expr::While { cond, body } => {
                let header = self.row(vec![word("while"), space(), self.layout(cond, size)]);
                self.stack(
                    vec![(0.0, header), (indent, self.layout(body, size))],
                    size * LINE_GAP,
                )
            }
            Expr::Try {
                body,
                handler,
                message,
            } => {
                let on_error = match message {
                    Some(name) => self.row(vec![word("on error"), space(), self.name(name, size)]),
                    None => word("on error"),
                };
                self.stack(
                    vec![
                        (0.0, word("try")),
                        (indent, self.layout(body, size)),
                        (0.0, on_error),
                        (indent, self.layout(handler, size)),
                    ],
                    size * LINE_GAP,
                )
            }
            Expr::Return(value) => {
                self.row(vec![word("return"), space(), self.layout(value, size)])
            }
            Expr::Break => word("break"),
            Expr::Continue => word("continue"),
            other => self.layout(other, size),
        }
    }
}
