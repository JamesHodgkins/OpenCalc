//! Errors drawn in place (docs/EDITOR.md section 4.6): the region's typeset form
//! with the characters that went wrong underlined. A parse error underlines the
//! range the parser reports; an evaluation error underlines the sub-expression at
//! the path the evaluator reports, found through the source spans.

use eframe::egui::{epaint::text::Fonts, Color32, Painter, Pos2, Rect, Shape, Stroke};
use opencalc_core::expr::Expr;
use opencalc_core::parser::parse_with_spans;

use super::result::typeset_part;
use crate::layout::{Layout, SourceLayout};

/// A region's typeset form and the rectangles (relative to it) to underline.
pub struct Marked {
    pub layout: Layout,
    pub bands: Vec<Rect>,
}

/// The typeset form of `source` with its error underlined, if there is anything to
/// underline: a parse error (`expr` is `Expr::Error`), or an evaluation error whose
/// `path` leads to a node. `None` when there is neither (the plain drawing is used).
pub fn mark(
    fonts: &Fonts,
    colour: Color32,
    source: &str,
    expr: &Expr,
    path: Option<&[u16]>,
    size: f32,
) -> Option<Marked> {
    let (shows_part, range) = match expr {
        Expr::Error(e) => (false, (e.start, e.end)),
        _ => {
            let node = parse_with_spans(source).1.at_path(path?)?.outer;
            (true, (node.start, node.end))
        }
    };
    let laid = SourceLayout::showing(fonts, colour, source, size, |e| {
        if shows_part {
            typeset_part(e)
        } else {
            e
        }
    });
    let bands = laid.bands(range.0, range.1);
    Some(Marked {
        layout: laid.layout,
        bands,
    })
}

/// Draw a wavy line under each band. `origin` is where the layout's top left is.
pub fn paint(painter: &Painter, bands: &[Rect], origin: Pos2, colour: Color32, zoom: f32) {
    let (step, amp) = (3.0 * zoom.max(0.6), 1.3 * zoom.max(0.6));
    let stroke = Stroke::new(1.2_f32, colour);
    for band in bands {
        let y = origin.y + band.max.y;
        let (x0, x1) = (origin.x + band.min.x, origin.x + band.max.x);
        let mut points = Vec::new();
        let mut x = x0;
        let mut up = false;
        while x < x1 {
            points.push(Pos2::new(x, if up { y - amp } else { y }));
            x += step;
            up = !up;
        }
        points.push(Pos2::new(x1, if up { y - amp } else { y }));
        painter.add(Shape::line(points, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    use opencalc_core::eval::{evaluate, RegionId};
    use opencalc_core::parser::parse;

    fn with_fonts<R>(f: impl FnOnce(&Fonts) -> R) -> R {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut result = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                if let Some(f) = f.take() {
                    result = Some(f(fonts));
                }
            });
        });
        result.expect("closure ran")
    }

    /// Mark `source` the way the canvas does: evaluate it for the error path.
    fn marked(fonts: &Fonts, source: &str) -> Option<Marked> {
        let expr = parse(source);
        let out = evaluate(&[(RegionId(1), expr.clone())]);
        let path = out.error_paths.get(&RegionId(1)).map(Vec::as_slice);
        mark(fonts, Color32::WHITE, source, &expr, path, 20.0)
    }

    fn x_range(fonts: &Fonts, source: &str, a: usize, b: usize) -> (f32, f32) {
        let l = SourceLayout::new(fonts, Color32::WHITE, source, 20.0);
        (l.caret_rect(a).min.x, l.caret_rect(b).min.x)
    }

    #[test]
    fn an_evaluation_error_underlines_exactly_the_failing_part() {
        with_fonts(|fonts| {
            // Adding a length to a time fails at the sum, characters 5..14.
            let source = "q := 1 m + 2 s";
            let m = marked(fonts, source).expect("an error to mark");
            assert_eq!(m.bands.len(), 1);
            let (x0, x1) = x_range(fonts, source, 5, 14);
            let band = m.bands[0];
            assert!((band.min.x - x0).abs() < 0.5 && (band.max.x - x1).abs() < 0.5);
            // A failure deeper down marks only that part, one band per line of a
            // fraction: `1 / 0` has a numerator and a denominator.
            let m = marked(fonts, "y := 2 * sqrt(1 / 0)").expect("an error to mark");
            assert_eq!(m.bands.len(), 2);
            let whole = SourceLayout::new(fonts, Color32::WHITE, "y := 2 * sqrt(1 / 0)", 20.0);
            assert!(m.bands.iter().all(|b| b.max.x < whole.layout.w));
        });
    }

    #[test]
    fn a_parse_error_underlines_the_characters_the_parser_points_at() {
        with_fonts(|fonts| {
            let source = "1 + 2 $ 3";
            let m = marked(fonts, source).expect("an error to mark");
            let (x0, x1) = x_range(fonts, source, 6, 7);
            assert_eq!(m.bands.len(), 1);
            assert!((m.bands[0].min.x - x0).abs() < 0.5 && (m.bands[0].max.x - x1).abs() < 0.5);
            // A missing end has no characters, so it gets a narrow mark at the end.
            let m = marked(fonts, "(1 + 2").expect("an error to mark");
            assert_eq!(m.bands.len(), 1);
            assert!(m.bands[0].width() <= 8.0 && m.bands[0].width() > 0.0);
        });
    }

    #[test]
    fn a_valid_region_has_nothing_to_underline() {
        with_fonts(|fonts| {
            assert!(marked(fonts, "x := 2 + 3").is_none());
            assert!(marked(fonts, "").is_some() == matches!(parse(""), Expr::Error(_)));
        });
    }

    #[test]
    fn an_evaluation_region_shows_x_and_underlines_inside_what_is_shown() {
        with_fonts(|fonts| {
            // `1 / 0 =` shows `1 / 0`; the error covers all of it.
            let m = marked(fonts, "1 / 0 =").expect("an error to mark");
            let plain = SourceLayout::new(fonts, Color32::WHITE, "1 / 0", 20.0);
            assert!(
                (m.layout.w - plain.layout.w).abs() < 0.5,
                "the = is not drawn"
            );
            assert!(!m.bands.is_empty());
        });
    }

    #[test]
    fn every_error_in_the_corpus_gets_a_mark_inside_its_layout() {
        with_fonts(|fonts| {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
            let mut marks = 0;
            for entry in std::fs::read_dir(dir).expect("the corpus folder") {
                let path = entry.expect("entry").path();
                if path.extension().is_none_or(|e| e != "calc") {
                    continue;
                }
                for line in std::fs::read_to_string(&path).expect("a file").lines() {
                    if line.starts_with(['#', '=', '!', '@']) || line.trim().is_empty() {
                        continue;
                    }
                    if let Some(m) = marked(fonts, line) {
                        marks += 1;
                        for b in &m.bands {
                            assert!(
                                b.min.x >= -0.5 && b.max.x <= m.layout.w + 0.5,
                                "{line:?}: {b:?} outside {}",
                                m.layout.w
                            );
                        }
                    }
                }
            }
            assert!(marks > 20, "only {marks} errors were marked");
        });
    }
}
