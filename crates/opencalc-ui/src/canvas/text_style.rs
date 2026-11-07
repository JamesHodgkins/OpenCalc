//! Bold and italic for text regions. Bold is a second bundled face (Inter Bold);
//! italic is egui's slant of the regular one.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontFamily, FontId};
use opencalc_core::document::FontStyle;

/// The font for a text region of this style at `size` points.
pub fn font_id(size: f32, style: FontStyle) -> FontId {
    let family = if style.bold {
        FontFamily::Name(crate::fonts::BOLD.into())
    } else {
        FontFamily::Proportional
    };
    FontId::new(size, family)
}

/// `text` laid out in this style, wrapped at `wrap` points.
pub fn job(text: &str, style: FontStyle, size: f32, colour: Color32, wrap: f32) -> LayoutJob {
    let format = TextFormat {
        font_id: font_id(size, style),
        color: colour,
        italics: style.italic,
        ..Default::default()
    };
    let mut job = LayoutJob::single_section(text.to_owned(), format);
    job.wrap.max_width = wrap;
    job
}
