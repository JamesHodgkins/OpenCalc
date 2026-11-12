//! The text on a page, as runs: a piece of text with where it sits. They are read
//! from the shapes the canvas code painted, before they become pixels, and give the
//! PDF its invisible, selectable text layer.

use eframe::egui::epaint::{ClippedShape, Shape};

/// One line of text as painted, in page units (1/96 inch), from the page's top left.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    /// Left edge.
    pub x: f32,
    /// The baseline's distance from the top of the page.
    pub baseline: f32,
    /// The font size.
    pub size: f32,
    /// How wide the text was painted.
    pub width: f32,
    pub text: String,
}

/// Icons live in the private use area; they are pictures, not text to select.
fn is_text(c: char) -> bool {
    !c.is_control() && !('\u{e000}'..='\u{f8ff}').contains(&c)
}

fn visit(shape: &Shape, out: &mut Vec<TextRun>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| visit(s, out)),
        // Turned text (a plot's axis title) is left as the picture it is.
        Shape::Text(t) if t.angle == 0.0 && t.opacity_factor > 0.0 => {
            for row in &t.galley.rows {
                let kept: Vec<_> = row.glyphs.iter().filter(|g| is_text(g.chr)).collect();
                let (Some(first), Some(last)) = (kept.first(), kept.last()) else {
                    continue;
                };
                let text: String = kept.iter().map(|g| g.chr).collect();
                if text.trim().is_empty() {
                    continue;
                }
                let size = t
                    .galley
                    .job
                    .sections
                    .get(first.section_index as usize)
                    .map_or(first.font_height, |s| s.format.font_id.size);
                out.push(TextRun {
                    x: t.pos.x + first.pos.x,
                    baseline: t.pos.y + first.pos.y,
                    size,
                    width: last.max_x() - first.pos.x,
                    text,
                });
            }
        }
        _ => {}
    }
}

/// Every run of text in `shapes`, in reading order: top to bottom, and left to
/// right along a line (text within a fraction of a line's height of its baseline
/// is on that line, so a subscript or an exponent stays with its neighbours).
pub fn collect(shapes: &[ClippedShape]) -> Vec<TextRun> {
    let mut runs = Vec::new();
    shapes.iter().for_each(|s| visit(&s.shape, &mut runs));
    reading_order(runs)
}

fn reading_order(mut runs: Vec<TextRun>) -> Vec<TextRun> {
    runs.sort_by(|a, b| a.baseline.total_cmp(&b.baseline));
    let mut lines: Vec<Vec<TextRun>> = Vec::new();
    for run in runs {
        match lines.last_mut() {
            Some(line) if (run.baseline - line[0].baseline).abs() <= 0.4 * run.size => {
                line.push(run);
            }
            _ => lines.push(vec![run]),
        }
    }
    for line in &mut lines {
        line.sort_by(|a, b| a.x.total_cmp(&b.x));
    }
    lines.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(x: f32, baseline: f32, text: &str) -> TextRun {
        TextRun {
            x,
            baseline,
            size: 20.0,
            width: 10.0,
            text: text.to_owned(),
        }
    }

    #[test]
    fn runs_read_top_to_bottom_and_left_to_right_with_an_exponent_staying_on_its_line() {
        let ordered = reading_order(vec![
            run(80.0, 200.0, "second line"),
            run(50.0, 100.0, "2"), // an exponent, a little high
            run(10.0, 106.0, "x"),
            run(30.0, 108.0, "+ 1"),
        ]);
        let text: Vec<&str> = ordered.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(text, ["x", "+ 1", "2", "second line"]);
    }

    #[test]
    fn icons_are_not_text() {
        assert!(is_text('a') && is_text('\u{3c0}') && is_text(' '));
        assert!(!is_text('\u{e10e}') && !is_text('\n'));
    }
}
