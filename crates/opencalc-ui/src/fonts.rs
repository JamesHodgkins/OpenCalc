//! Bundled fonts (spec section 4.6): Inter for text and Lucide for icons.
//!
//! Both are embedded so rendering is identical on every platform. Lucide's
//! glyphs sit in the Private Use Area, so registering it in the proportional
//! family lets icon characters resolve without a separate family. It must come
//! before Inter: Inter defines some Private Use codepoints of its own, and those
//! would otherwise win and draw the wrong glyph. Lucide has no letters, so text
//! still falls through to Inter.
//! egui's own fonts stay at the end of the list as the fallback for glyphs
//! these two lack.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

const INTER: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const LUCIDE: &[u8] = include_bytes!("../assets/fonts/lucide.ttf");

pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("inter".to_owned(), Arc::new(FontData::from_static(INTER)));
    fonts
        .font_data
        .insert("lucide".to_owned(), Arc::new(FontData::from_static(LUCIDE)));
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "lucide".to_owned());
    proportional.insert(1, "inter".to_owned());
    fonts
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons;

    #[test]
    fn lucide_comes_first_then_inter_then_egui_defaults() {
        let fonts = definitions();
        let list = &fonts.families[&FontFamily::Proportional];
        assert_eq!(&list[..2], ["lucide", "inter"]);
        assert!(list.len() > 2, "egui defaults must remain as fallback");
    }

    /// Every icon constant must be a real glyph in the bundled Lucide font, and
    /// must lie in the Private Use Area so it cannot collide with Inter's text.
    #[test]
    fn every_icon_resolves_to_a_glyph_in_lucide() {
        let src = include_str!("icons.rs");
        let chars: Vec<char> = src
            .lines()
            .filter(|l| l.starts_with("pub const"))
            .filter_map(|l| {
                let hex = l.split("\\u{").nth(1)?.split('}').next()?;
                char::from_u32(u32::from_str_radix(hex, 16).ok()?)
            })
            .collect();
        assert!(chars.len() > 40);
        let face = ttf_has_glyphs(LUCIDE);
        for c in chars {
            assert!(
                ('\u{e000}'..='\u{f8ff}').contains(&c),
                "{c:?} outside the PUA"
            );
            assert!(face(c), "glyph {:x} missing from lucide.ttf", c as u32);
        }
        // `icons::CLOSE` is referenced so the module is checked for use as well.
        assert_eq!(icons::CLOSE, '\u{e1b1}');
    }

    /// Minimal cmap lookup via egui's font stack: lay out the character with only
    /// Lucide installed and see whether the glyph is the font's "missing" box.
    fn ttf_has_glyphs(_bytes: &'static [u8]) -> impl Fn(char) -> bool {
        let mut fonts = FontDefinitions::empty();
        fonts
            .font_data
            .insert("lucide".to_owned(), Arc::new(FontData::from_static(LUCIDE)));
        fonts
            .families
            .insert(FontFamily::Proportional, vec!["lucide".to_owned()]);
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        // Fonts only exist once a frame has run.
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        move |c| ctx.fonts(|f| f.has_glyph(&egui::FontId::proportional(14.0), c))
    }
}
