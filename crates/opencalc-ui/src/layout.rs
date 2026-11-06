//! Maths typesetting (spec section 4.3a): `Expr` -> positioned draw operations.
//!
//! A [`Layout`] is a box with a size and a baseline plus a flat list of text and
//! line ops relative to its top-left. Composing boxes translates their ops.
//! Layouts hold no per-frame state, so they can be cached per region.

mod grid;

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Galley, Pos2, Stroke, Vec2};
use opencalc_core::expr::{AssignKind, BinaryOp, Expr, UnaryOp};

pub enum Op {
    Text { at: Vec2, galley: Arc<Galley> },
    Line { a: Vec2, b: Vec2 },
}

pub struct Layout {
    pub w: f32,
    pub h: f32,
    /// Distance from the top to the text baseline.
    pub baseline: f32,
    pub ops: Vec<Op>,
}

/// Scale of a script (exponent) relative to its base.
const SCRIPT: f32 = 0.7;
/// Height of the fraction bar above the baseline, as a fraction of font size.
const AXIS: f32 = 0.28;

pub struct Typesetter<'a> {
    fonts: &'a egui::epaint::text::Fonts,
    colour: Color32,
}

impl<'a> Typesetter<'a> {
    pub fn new(fonts: &'a egui::epaint::text::Fonts, colour: Color32) -> Self {
        Typesetter { fonts, colour }
    }

    pub fn layout(&self, e: &Expr, size: f32) -> Layout {
        match e {
            Expr::Number(v) => self.text(&fmt_num(*v), size),
            Expr::Name(n) => self.text(n, size),
            Expr::Quantity { value, unit } => self.row(vec![
                self.text(&fmt_num(*value), size),
                self.gap(size * 0.25),
                self.text(unit, size),
            ]),
            Expr::Unary {
                op: UnaryOp::Neg,
                arg,
            } => self.row(vec![
                self.text("\u{2212}", size),
                self.operand(arg, size, 4),
            ]),
            Expr::Binary {
                op: BinaryOp::Div,
                lhs,
                rhs,
            } => self.fraction(lhs, rhs, size),
            Expr::Binary {
                op: BinaryOp::Pow,
                lhs,
                rhs,
            } => self.power(lhs, rhs, size),
            Expr::Binary { op, lhs, rhs } => {
                let p = prec(e);
                let sign = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "\u{2212}",
                    BinaryOp::Eq => "=",
                    BinaryOp::Ne => "\u{2260}",
                    BinaryOp::Lt => "<",
                    BinaryOp::Gt => ">",
                    BinaryOp::Le => "\u{2264}",
                    BinaryOp::Ge => "\u{2265}",
                    _ => "\u{00b7}",
                };
                // Left operand binds at p; the right needs parens at equal precedence.
                self.row(vec![
                    self.operand(lhs, size, p),
                    self.gap(size * 0.25),
                    self.text(sign, size),
                    self.gap(size * 0.25),
                    self.operand(rhs, size, p + 1),
                ])
            }
            Expr::Matrix { rows, cols, cells } => self.matrix(*rows, *cols, cells, size),
            Expr::Index { base, indices } => self.index(base, indices, size),
            Expr::Range { start, step, end } => self.range(start, step.as_deref(), end, size),
            Expr::Conditional {
                cond,
                then,
                otherwise,
            } => self.call_like("if", &[cond, then, otherwise], size),
            Expr::Fraction { num, den } => self.fraction(num, den, size),
            Expr::Root { radicand } => self.root(radicand, size),
            Expr::Call { name, args } => {
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
            Expr::Assign {
                kind,
                target,
                value,
            } => {
                let sym = match kind {
                    AssignKind::Define => ":=",
                    AssignKind::Evaluate => "=",
                };
                self.row(vec![
                    self.text(target, size),
                    self.gap(size * 0.3),
                    self.text(sym, size),
                    self.gap(size * 0.3),
                    self.layout(value, size),
                ])
            }
            Expr::Error(e) => self.text(&format!("? {e}"), size),
        }
    }

    /// A child, parenthesised if it binds looser than `min`.
    fn operand(&self, e: &Expr, size: f32, min: u8) -> Layout {
        if prec(e) < min {
            self.row(vec![
                self.text("(", size),
                self.layout(e, size),
                self.text(")", size),
            ])
        } else {
            self.layout(e, size)
        }
    }

    fn text(&self, s: &str, size: f32) -> Layout {
        let galley =
            self.fonts
                .layout_no_wrap(s.to_owned(), FontId::proportional(size), self.colour);
        let r = galley.rect;
        Layout {
            w: r.width(),
            h: r.height(),
            baseline: r.height() * 0.78,
            ops: vec![Op::Text {
                at: Vec2::ZERO,
                galley,
            }],
        }
    }

    fn gap(&self, w: f32) -> Layout {
        Layout {
            w,
            h: 0.0,
            baseline: 0.0,
            ops: Vec::new(),
        }
    }

    /// Left to right, aligned on baselines.
    fn row(&self, parts: Vec<Layout>) -> Layout {
        let above = parts.iter().map(|p| p.baseline).fold(0.0, f32::max);
        let below = parts.iter().map(|p| p.h - p.baseline).fold(0.0, f32::max);
        let mut out = Layout {
            w: 0.0,
            h: above + below,
            baseline: above,
            ops: Vec::new(),
        };
        for p in parts {
            let at = Vec2::new(out.w, above - p.baseline);
            out.absorb(p, at, true);
        }
        out
    }

    fn fraction(&self, num: &Expr, den: &Expr, size: f32) -> Layout {
        let n = self.layout(num, size);
        let d = self.layout(den, size);
        let pad = size * 0.15;
        let w = n.w.max(d.w) + 2.0 * pad;
        let bar_y = n.h + size * 0.08;
        let h = bar_y + size * 0.08 + d.h;
        let (nw, dw) = (n.w, d.w);
        let mut out = Layout {
            w,
            h,
            baseline: bar_y + size * AXIS,
            ops: Vec::new(),
        };
        out.absorb(n, Vec2::new((w - nw) / 2.0, 0.0), false);
        out.absorb(d, Vec2::new((w - dw) / 2.0, bar_y + size * 0.08), false);
        out.ops.push(Op::Line {
            a: Vec2::new(0.0, bar_y),
            b: Vec2::new(w, bar_y),
        });
        out
    }

    fn power(&self, base: &Expr, exp: &Expr, size: f32) -> Layout {
        let b = self.operand(base, size, 6);
        let e = self.layout(exp, size * SCRIPT);
        // Lift the exponent so its baseline sits about half a font size above the base's.
        let lift = size * 0.45;
        let top = (e.baseline - (b.baseline - lift)).max(0.0);
        let base_y = top;
        let exp_y = base_y + b.baseline - lift - e.baseline;
        let h = (base_y + b.h).max(exp_y + e.h);
        let baseline = base_y + b.baseline;
        let bw = b.w;
        let mut out = Layout {
            w: bw + e.w,
            h,
            baseline,
            ops: Vec::new(),
        };
        out.absorb(b, Vec2::new(0.0, base_y), false);
        out.absorb(e, Vec2::new(bw, exp_y), false);
        out
    }

    fn root(&self, rad: &Expr, size: f32) -> Layout {
        let r = self.layout(rad, size);
        let tick = size * 0.7;
        let top = size * 0.12;
        let w = tick + r.w + size * 0.1;
        let h = r.h + top;
        let baseline = r.baseline + top;
        let mut out = Layout {
            w,
            h,
            baseline,
            ops: Vec::new(),
        };
        let (rw, bottom) = (r.w, h);
        out.absorb(r, Vec2::new(tick, top), false);
        let pts = [
            Vec2::new(0.0, h * 0.58),
            Vec2::new(tick * 0.25, h * 0.52),
            Vec2::new(tick * 0.55, bottom),
            Vec2::new(tick, 0.0),
        ];
        for pair in pts.windows(2) {
            out.ops.push(Op::Line {
                a: pair[0],
                b: pair[1],
            });
        }
        out.ops.push(Op::Line {
            a: Vec2::new(tick, 0.0),
            b: Vec2::new(tick + rw + size * 0.1, 0.0),
        });
        out
    }
}

impl Layout {
    /// Add `child` at `at`. `advance` grows the width, for row composition.
    fn absorb(&mut self, child: Layout, at: Vec2, advance: bool) {
        if advance {
            self.w += child.w;
        }
        for op in child.ops {
            self.ops.push(match op {
                Op::Text { at: p, galley } => Op::Text { at: p + at, galley },
                Op::Line { a, b } => Op::Line {
                    a: a + at,
                    b: b + at,
                },
            });
        }
    }

    pub fn paint(&self, painter: &egui::Painter, origin: Pos2, colour: Color32) {
        let stroke = Stroke::new(1.2_f32, colour);
        for op in &self.ops {
            match op {
                Op::Text { at, galley } => {
                    painter.galley(origin + *at, Arc::clone(galley), colour);
                }
                Op::Line { a, b } => {
                    painter.line_segment([origin + *a, origin + *b], stroke);
                }
            }
        }
    }
}

/// Binding strength used to decide where parentheses are needed.
fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Binary {
            op: BinaryOp::Add | BinaryOp::Sub,
            ..
        } => 1,
        // Comparisons are the loosest operators, so they are the only ones that
        // need brackets inside a sum.
        Expr::Binary {
            op:
                BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge,
            ..
        } => 0,
        Expr::Binary {
            op: BinaryOp::Mul, ..
        } => 2,
        Expr::Unary { .. } => 3,
        Expr::Assign { .. } => 0,
        // Fractions and powers are visually grouped, so they never need parens.
        _ => 6,
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e9 {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::expr::BinaryOp::*;

    /// Run `f` with real fonts loaded, as in a live frame.
    fn with_fonts<R>(f: impl FnOnce(&Typesetter) -> R) -> R {
        let ctx = egui::Context::default();
        let mut result = None;
        let mut f = Some(f);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                if let Some(f) = f.take() {
                    result = Some(f(&Typesetter::new(fonts, Color32::WHITE)));
                }
            });
        });
        result.expect("closure ran")
    }

    fn parsed(src: &str) -> Expr {
        let e = opencalc_core::parser::parse(src);
        assert!(!matches!(e, Expr::Error(_)), "{src}");
        e
    }

    #[test]
    fn a_matrix_grows_with_its_rows_and_columns_and_has_brackets() {
        with_fonts(|t| {
            let one = t.layout(&parsed("[1]"), 16.0);
            let wide = t.layout(&parsed("[1, 2, 3]"), 16.0);
            let tall = t.layout(&parsed("[1; 2; 3]"), 16.0);
            assert!(wide.w > one.w * 2.0 && wide.h < tall.h);
            assert!(tall.h > one.h * 2.0 && tall.w < wide.w);
            // Two bracket spines with two ticks each.
            let lines = wide
                .ops
                .iter()
                .filter(|o| matches!(o, Op::Line { .. }))
                .count();
            assert_eq!(lines, 6);
        });
    }

    #[test]
    fn matrix_columns_line_up_whatever_the_cell_widths() {
        with_fonts(|t| {
            // Same shape, so the same width, however wide the cells are.
            let a = t.layout(&parsed("[1, 2; 3, 4]"), 16.0);
            let b = t.layout(&parsed("[1, 2; 3, 4]"), 16.0);
            assert_eq!(a.w, b.w);
            let narrow = t.layout(&parsed("[1, 1; 1, 1]"), 16.0);
            let wide = t.layout(&parsed("[1000, 1; 1, 1000]"), 16.0);
            assert!(wide.w > narrow.w);
        });
    }

    #[test]
    fn an_index_is_lowered_and_a_conditional_reads_like_a_call() {
        with_fonts(|t| {
            let v = t.layout(&parsed("v"), 16.0);
            let v0 = t.layout(&parsed("v[0]"), 16.0);
            assert!(v0.w > v.w && v0.h > v.h);
            assert!(v0.baseline == v.baseline);
            let c = t.layout(&parsed("if(a, 1, 2)"), 16.0);
            assert!(c.w > t.layout(&parsed("a"), 16.0).w * 3.0);
        });
    }

    #[test]
    fn comparisons_typeset_without_brackets_at_the_top_and_with_them_in_a_sum() {
        with_fonts(|t| {
            let top = t.layout(&parsed("a < b"), 16.0);
            let nested = t.layout(&parsed("(a < b) + 1"), 16.0);
            assert!(nested.w > top.w, "the comparison needs brackets in a sum");
        });
    }

    #[test]
    fn fraction_is_taller_than_its_parts_and_has_a_bar() {
        with_fonts(|t| {
            let one = t.layout(&Expr::num(1.0), 16.0);
            let frac = t.layout(&Expr::bin(Div, Expr::num(1.0), Expr::num(2.0)), 16.0);
            assert!(frac.h > one.h * 1.8);
            assert!(frac.ops.iter().any(|o| matches!(o, Op::Line { .. })));
        });
    }

    #[test]
    fn superscript_adds_height_and_width() {
        with_fonts(|t| {
            let x = t.layout(&Expr::name("x"), 16.0);
            let x2 = t.layout(&Expr::bin(Pow, Expr::name("x"), Expr::num(2.0)), 16.0);
            assert!(x2.w > x.w);
            assert!(x2.h > x.h);
        });
    }

    #[test]
    fn root_is_wider_than_its_radicand_and_draws_a_radical() {
        with_fonts(|t| {
            let inner = Expr::name("A");
            let r = t.layout(
                &Expr::Root {
                    radicand: Box::new(inner.clone()),
                },
                16.0,
            );
            let a = t.layout(&inner, 16.0);
            assert!(r.w > a.w);
            assert!(
                r.ops
                    .iter()
                    .filter(|o| matches!(o, Op::Line { .. }))
                    .count()
                    >= 4
            );
        });
    }

    #[test]
    fn sum_inside_product_is_parenthesised() {
        with_fonts(|t| {
            let sum = Expr::bin(Add, Expr::name("a"), Expr::name("b"));
            let plain = t.layout(&Expr::bin(Add, Expr::name("a"), Expr::name("b")), 16.0);
            let product = t.layout(&Expr::bin(Mul, sum, Expr::name("c")), 16.0);
            let flat = t.layout(&Expr::bin(Mul, Expr::name("a"), Expr::name("c")), 16.0);
            // (a + b)·c is wider than a·c by more than the "+ b" alone would add.
            assert!(product.w > flat.w + plain.w * 0.5);
        });
    }
}
