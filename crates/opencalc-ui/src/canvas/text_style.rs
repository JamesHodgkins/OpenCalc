//! Bold, italic, underline and colour for text regions. Bold and italic are real
//! faces (Inter Bold, Italic and Bold Italic), so nothing is slanted or smeared.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontFamily, FontId, Stroke};
use opencalc_core::document::{FontStyle, Run, RunStyle, TEXT_COLOURS};

/// The colours a stretch of text can have, by palette index (see `TEXT_COLOURS`).
/// Chosen to read on white paper.
pub const COLOURS: [(&str, Color32); TEXT_COLOURS as usize] = [
    ("Red", Color32::from_rgb(0xC6, 0x28, 0x28)),
    ("Blue", Color32::from_rgb(0x15, 0x65, 0xC0)),
    ("Green", Color32::from_rgb(0x2E, 0x7D, 0x32)),
    ("Orange", Color32::from_rgb(0xE6, 0x51, 0x00)),
    ("Purple", Color32::from_rgb(0x6A, 0x1B, 0x9A)),
    ("Teal", Color32::from_rgb(0x00, 0x69, 0x5C)),
    ("Grey", Color32::from_rgb(0x75, 0x75, 0x75)),
    ("Brown", Color32::from_rgb(0x6D, 0x4C, 0x41)),
];

/// The font at `size` points in the given weight and slant.
pub fn font_for(size: f32, bold: bool, italic: bool) -> FontId {
    let family = match (bold, italic) {
        (false, false) => FontFamily::Proportional,
        (true, false) => FontFamily::Name(crate::fonts::BOLD.into()),
        (false, true) => FontFamily::Name(crate::fonts::ITALIC.into()),
        (true, true) => FontFamily::Name(crate::fonts::BOLD_ITALIC.into()),
    };
    FontId::new(size, family)
}

/// The font for a text region of this style at `size` points.
pub fn font_id(size: f32, style: FontStyle) -> FontId {
    font_for(size, style.bold, style.italic)
}

fn format(size: f32, base: FontStyle, run: RunStyle, colour: Color32) -> TextFormat {
    let colour = run
        .colour
        .and_then(|c| COLOURS.get(usize::from(c)))
        .map_or(colour, |(_, c)| *c);
    TextFormat {
        font_id: font_for(size, base.bold || run.bold, base.italic || run.italic),
        color: colour,
        underline: if run.underline {
            Stroke::new((size / 14.0).max(1.0), colour)
        } else {
            Stroke::NONE
        },
        ..Default::default()
    }
}

/// `text` laid out with styled stretches (`runs`, in characters from the start;
/// characters after the last run are plain), wrapped at `wrap` points.
pub fn job_runs(
    text: &str,
    style: FontStyle,
    runs: &[Run],
    size: f32,
    colour: Color32,
    wrap: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    // Where each character starts, so a run's length in characters finds its bytes.
    let starts: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();
    let chars = starts.len() - 1;
    let mut at = 0;
    for run in runs {
        let end = (at + run.len as usize).min(chars);
        if end > at {
            job.append(
                &text[starts[at]..starts[end]],
                0.0,
                format(size, style, run.style, colour),
            );
            at = end;
        }
    }
    if at < chars || chars == 0 {
        job.append(
            &text[starts[at]..],
            0.0,
            format(size, style, RunStyle::default(), colour),
        );
    }
    job.wrap.max_width = wrap;
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold(len: u32) -> Run {
        Run {
            len,
            style: RunStyle {
                bold: true,
                ..RunStyle::default()
            },
        }
    }

    #[test]
    fn runs_split_the_text_into_sections_that_cover_all_of_it() {
        let job = job_runs(
            "héllo wörld",
            FontStyle::default(),
            &[bold(3)],
            14.0,
            Color32::BLACK,
            100.0,
        );
        let parts: Vec<&str> = job
            .sections
            .iter()
            .map(|s| &job.text[s.byte_range.clone()])
            .collect();
        assert_eq!(parts, ["hél", "lo wörld"]);
        assert_eq!(
            job.sections[0].format.font_id.family,
            FontFamily::Name(crate::fonts::BOLD.into())
        );
    }

    #[test]
    fn runs_longer_than_the_text_and_empty_text_are_safe() {
        let j = job_runs(
            "ab",
            FontStyle::default(),
            &[bold(9)],
            14.0,
            Color32::BLACK,
            50.0,
        );
        assert_eq!(j.sections.len(), 1);
        let j = job_runs(
            "",
            FontStyle::default(),
            &[bold(3)],
            14.0,
            Color32::BLACK,
            50.0,
        );
        assert_eq!(j.text, "");
    }

    #[test]
    fn the_region_style_and_a_run_add_up() {
        let style = FontStyle {
            italic: true,
            ..FontStyle::default()
        };
        let j = job_runs("ab", style, &[bold(2)], 14.0, Color32::BLACK, 50.0);
        assert_eq!(
            j.sections[0].format.font_id.family,
            FontFamily::Name(crate::fonts::BOLD_ITALIC.into())
        );
    }

    #[test]
    fn colour_and_underline_come_from_the_run() {
        let run = Run {
            len: 2,
            style: RunStyle {
                underline: true,
                colour: Some(1),
                ..RunStyle::default()
            },
        };
        let j = job_runs(
            "ab",
            FontStyle::default(),
            &[run],
            14.0,
            Color32::BLACK,
            50.0,
        );
        assert_eq!(j.sections[0].format.color, COLOURS[1].1);
        assert_ne!(j.sections[0].format.underline, Stroke::NONE);
    }
}
