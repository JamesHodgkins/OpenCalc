//! Names, with subscripts: `F_x` is one name, drawn as F with a lowered x. The
//! source keeps the underscore; indexing stays `v[i]`.

use opencalc_core::expr::Expr;

use super::{Layout, Typesetter, SCRIPT};
use eframe::egui::Vec2;

/// The parts of a name drawn as a base and a subscript: the text before the first
/// underscore and the text after it. A leading or trailing underscore is just a
/// character of the name.
fn split(name: &str) -> Option<(&str, &str)> {
    let i = name.find('_')?;
    (i > 0 && i + 1 < name.len()).then(|| (&name[..i], &name[i + 1..]))
}

impl Typesetter<'_> {
    /// A name, with its subscript lowered when it has one.
    pub(super) fn name(&self, name: &str, size: f32) -> Layout {
        // The constant `pi` reads as the letter once the region is not being edited.
        // (While editing, every typed character keeps its own place for the caret.)
        if !self.marks && name == "pi" {
            return self.text("\u{3c0}", size);
        }
        let Some((base, sub)) = split(name) else {
            return self.text(name, size);
        };
        let (base, sub) = (self.text(base, size), self.text(sub, size * SCRIPT));
        let (bw, bh, bb) = (base.w, base.h, base.baseline);
        let sub_top = bb + size * 0.25 - sub.baseline;
        let mut out = Layout {
            w: bw + sub.w,
            h: bh.max(sub_top + sub.h),
            baseline: bb,
            ops: Vec::new(),
            marks: Vec::new(),
            first_line: 0.0,
        };
        out.absorb(base, Vec2::ZERO, false);
        out.absorb(sub, Vec2::new(bw, sub_top), false);
        out
    }

    /// The x offset of every character boundary of `name` as drawn (one more than
    /// it has characters). The underscore draws nothing: both of its sides sit at
    /// the start of the subscript.
    pub(super) fn name_offsets(&self, name: &str, size: f32) -> Vec<f32> {
        match split(name) {
            None => self.text_offsets(name, size),
            Some((base, sub)) => {
                let mut xs = self.text_offsets(base, size);
                let at = self.text(base, size).w;
                xs.extend(
                    self.text_offsets(sub, size * SCRIPT)
                        .into_iter()
                        .map(|x| at + x),
                );
                xs
            }
        }
    }

    /// Character boundaries of a single line of text.
    fn text_offsets(&self, s: &str, size: f32) -> Vec<f32> {
        let galley = self.fonts.layout_no_wrap(
            s.to_owned(),
            eframe::egui::FontId::proportional(size),
            self.colour,
        );
        match galley.rows.first() {
            Some(row) => (0..=s.chars().count()).map(|i| row.x_offset(i)).collect(),
            None => vec![0.0],
        }
    }

    /// The offsets for the name at an `Expr::Name` node, if it is one.
    pub(super) fn offsets_of(&self, e: &Expr, size: f32) -> Option<Vec<f32>> {
        match e {
            Expr::Name(n) => Some(self.name_offsets(n, size)),
            _ => None,
        }
    }
}
