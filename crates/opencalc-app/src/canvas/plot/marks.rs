//! Drawing one data series on a plot: lines, steps, markers, stems and bars. The
//! geometry is plain functions on screen positions so it can be tested; the
//! painting wrappers only hand it to the painter.

use eframe::egui::{epaint::PathStroke, Color32, Painter, Pos2, Rect, Shape, Stroke};
use opencalc_core::document::{Marker, SeriesKind};

/// How a series is stroked and filled.
#[derive(Clone, Copy)]
pub struct Look {
    pub colour: Color32,
    /// Line width in screen pixels.
    pub width: f32,
    /// Marker radius in screen pixels.
    pub radius: f32,
}

/// The corners of a stepped path: from each point go across, then up or down.
pub fn step_path(points: &[Pos2]) -> Vec<Pos2> {
    let mut path = Vec::with_capacity(points.len() * 2);
    for (k, p) in points.iter().enumerate() {
        if let Some(previous) = k.checked_sub(1).map(|i| points[i]) {
            path.push(Pos2::new(p.x, previous.y));
        }
        path.push(*p);
    }
    path
}

/// The three corners of an upward triangle of radius `r` around `at`.
pub fn triangle(at: Pos2, r: f32) -> [Pos2; 3] {
    [
        Pos2::new(at.x, at.y - r),
        Pos2::new(at.x - r * 0.9, at.y + r * 0.7),
        Pos2::new(at.x + r * 0.9, at.y + r * 0.7),
    ]
}

/// The smallest gap between neighbouring x positions (screen pixels), which
/// bounds how wide a bar may be; `fallback` when fewer than two differ.
pub fn bar_slot(xs: impl Iterator<Item = f32>, fallback: f32) -> f32 {
    let mut sorted: Vec<f32> = xs.filter(|x| x.is_finite()).collect();
    sorted.sort_by(f32::total_cmp);
    sorted
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|gap| *gap > 0.5)
        .fold(None, |m: Option<f32>, g| Some(m.map_or(g, |m| m.min(g))))
        .unwrap_or(fallback)
}

/// The smallest positive gap between neighbouring values, if there are two
/// different ones (in data units, for padding an axis).
pub fn min_gap(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut sorted: Vec<f64> = values.filter(|v| v.is_finite()).collect();
    sorted.sort_by(f64::total_cmp);
    sorted
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|g| *g > 0.0)
        .fold(None, |m: Option<f64>, g| Some(m.map_or(g, |m| m.min(g))))
}

pub fn marker(painter: &Painter, shape: Marker, at: Pos2, r: f32, colour: Color32) {
    match shape {
        Marker::Circle => {
            painter.circle_filled(at, r, colour);
        }
        Marker::Square => {
            painter.rect_filled(
                Rect::from_center_size(at, eframe::egui::vec2(2.0 * r, 2.0 * r)),
                0.0,
                colour,
            );
        }
        Marker::Triangle => {
            painter.add(Shape::convex_polygon(
                triangle(at, r * 1.15).to_vec(),
                colour,
                Stroke::NONE,
            ));
        }
        Marker::Cross => {
            let stroke = Stroke::new((r * 0.45).max(1.0), colour);
            painter.line_segment(
                [Pos2::new(at.x - r, at.y - r), Pos2::new(at.x + r, at.y + r)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(at.x - r, at.y + r), Pos2::new(at.x + r, at.y - r)],
                stroke,
            );
        }
    }
}

fn polyline(painter: &Painter, points: Vec<Pos2>, look: Look) {
    match points.as_slice() {
        [] => {}
        [only] => {
            painter.circle_filled(*only, look.width * 1.8, look.colour);
        }
        _ => {
            painter.add(Shape::line(
                points,
                PathStroke::new(look.width, look.colour),
            ));
        }
    }
}

/// What a series needs to know about the plot to be drawn.
pub struct Frame {
    /// Screen y of the baseline that stems and bars grow from.
    pub baseline: f32,
    /// Width of one bar and the offset of this series' bars from their x position.
    pub bar_width: f32,
    pub bar_offset: f32,
}

/// Draw a series of `kind` through the screen positions `points`.
pub fn draw(
    painter: &Painter,
    kind: SeriesKind,
    shape: Marker,
    points: &[Pos2],
    look: Look,
    frame: &Frame,
) {
    match kind {
        SeriesKind::Line => polyline(painter, points.to_vec(), look),
        SeriesKind::Markers => points
            .iter()
            .for_each(|p| marker(painter, shape, *p, look.radius, look.colour)),
        SeriesKind::LineMarkers => {
            polyline(painter, points.to_vec(), look);
            points
                .iter()
                .for_each(|p| marker(painter, shape, *p, look.radius, look.colour));
        }
        SeriesKind::Step => polyline(painter, step_path(points), look),
        SeriesKind::Stem => {
            let stroke = Stroke::new(look.width, look.colour);
            for p in points {
                painter.line_segment([Pos2::new(p.x, frame.baseline), *p], stroke);
                marker(painter, shape, *p, look.radius, look.colour);
            }
        }
        SeriesKind::Bar => {
            let fill = look.colour.gamma_multiply(0.8);
            for p in points {
                let x = p.x + frame.bar_offset;
                let (top, bottom) = (p.y.min(frame.baseline), p.y.max(frame.baseline));
                let rect = Rect::from_min_max(
                    Pos2::new(x - frame.bar_width / 2.0, top),
                    Pos2::new(x + frame.bar_width / 2.0, bottom.max(top + 1.0)),
                );
                painter.rect_filled(rect, 0.0, fill);
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0_f32, look.colour),
                    eframe::egui::StrokeKind::Inside,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }

    #[test]
    fn a_step_goes_across_then_up_or_down() {
        let path = step_path(&[p(0.0, 0.0), p(10.0, 5.0), p(20.0, 2.0)]);
        assert_eq!(
            path,
            [
                p(0.0, 0.0),
                p(10.0, 0.0),
                p(10.0, 5.0),
                p(20.0, 5.0),
                p(20.0, 2.0)
            ]
        );
        assert_eq!(step_path(&[p(1.0, 1.0)]), [p(1.0, 1.0)]);
        assert!(step_path(&[]).is_empty());
    }

    #[test]
    fn the_triangle_points_up_and_is_centred_on_the_point() {
        let t = triangle(p(10.0, 10.0), 4.0);
        assert!(t[0].y < 10.0 && t[1].y > 10.0 && t[2].y > 10.0);
        assert!((t[1].x + t[2].x) / 2.0 - 10.0 < 1e-5);
        assert_eq!(t[1].y, t[2].y);
    }

    #[test]
    fn the_data_gap_ignores_repeats_and_needs_two_different_values() {
        assert_eq!(min_gap([3.0, 1.0, 1.0, 2.0].into_iter()), Some(1.0));
        assert_eq!(min_gap([5.0, 5.0].into_iter()), None);
        assert_eq!(min_gap(std::iter::empty()), None);
        assert_eq!(min_gap([0.0, f64::NAN, 0.5].into_iter()), Some(0.5));
    }

    #[test]
    fn a_bar_is_no_wider_than_the_closest_pair_of_x_values() {
        assert_eq!(bar_slot([0.0, 10.0, 30.0].into_iter(), 99.0), 10.0);
        assert_eq!(
            bar_slot([30.0, 0.0, 10.0].into_iter(), 99.0),
            10.0,
            "order does not matter"
        );
        assert_eq!(
            bar_slot([5.0].into_iter(), 99.0),
            99.0,
            "one value: the fallback"
        );
        assert_eq!(
            bar_slot([5.0, 5.0].into_iter(), 99.0),
            99.0,
            "equal values are not a gap"
        );
        assert_eq!(
            bar_slot([f32::NAN, 0.0, 4.0].into_iter(), 99.0),
            4.0,
            "non-finite ignored"
        );
        assert_eq!(bar_slot(std::iter::empty(), 7.0), 7.0);
    }
}
