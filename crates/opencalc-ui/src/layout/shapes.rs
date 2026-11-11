//! Typesetting the shaped forms: fractions, powers, units, roots.

use eframe::egui::Vec2;
use opencalc_core::expr::Expr;

use super::{Layout, Op, Typesetter, AXIS, SCRIPT};

impl Typesetter<'_> {
    pub(super) fn fraction(&self, num: &Expr, den: &Expr, size: f32) -> Layout {
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
            marks: Vec::new(),
            first_line: 0.0,
        };
        out.absorb(n, Vec2::new((w - nw) / 2.0, 0.0), false);
        out.absorb(d, Vec2::new((w - dw) / 2.0, bar_y + size * 0.08), false);
        out.ops.push(Op::Line {
            a: Vec2::new(0.0, bar_y),
            b: Vec2::new(w, bar_y),
        });
        out
    }

    pub(super) fn power(&self, base: &Expr, exp: &Expr, size: f32) -> Layout {
        let b = self.operand(base, size, 6);
        let e = self.layout(exp, size * SCRIPT);
        self.raise(b, e, size)
    }

    /// A unit as it is written in a sheet: `N/mm^2` as N/mm with a raised 2, `kg*m`
    /// with a middle dot. Upright text, on one line (a unit is not a fraction).
    pub fn unit(&self, text: &str, size: f32) -> Layout {
        let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        let mut parts: Vec<Layout> = Vec::new();
        let mut word = String::new();
        let flush = |word: &mut String, parts: &mut Vec<Layout>| {
            if !word.is_empty() {
                parts.push(self.text(word, size));
                word.clear();
            }
        };
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '^' => {
                    flush(&mut word, &mut parts);
                    let mut exp = String::new();
                    if chars.get(i + 1) == Some(&'(') {
                        i += 2;
                        while i < chars.len() && chars[i] != ')' {
                            exp.push(chars[i]);
                            i += 1;
                        }
                    } else {
                        let mut j = i + 1;
                        if matches!(chars.get(j), Some('-' | '+')) {
                            exp.push(chars[j]);
                            j += 1;
                        }
                        while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                            exp.push(chars[j]);
                            j += 1;
                        }
                        i = j - 1;
                    }
                    let exp = exp.replace('-', "\u{2212}");
                    let base = parts.pop().unwrap_or_else(|| self.text("", size));
                    parts.push(self.raise(base, self.text(&exp, size * SCRIPT), size));
                }
                '*' | '\u{b7}' | '\u{22c5}' => {
                    flush(&mut word, &mut parts);
                    parts.push(self.text("\u{b7}", size));
                }
                '/' => {
                    flush(&mut word, &mut parts);
                    parts.push(self.text("/", size));
                }
                c => word.push(c),
            }
            i += 1;
        }
        flush(&mut word, &mut parts);
        self.row(parts)
    }

    /// `e` raised as a superscript on `b`.
    pub(super) fn raise(&self, b: Layout, e: Layout, size: f32) -> Layout {
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
            marks: Vec::new(),
            first_line: 0.0,
        };
        out.absorb(b, Vec2::new(0.0, base_y), false);
        out.absorb(e, Vec2::new(bw, exp_y), false);
        out
    }

    pub(super) fn root(&self, rad: &Expr, size: f32) -> Layout {
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
            marks: Vec::new(),
            first_line: 0.0,
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
