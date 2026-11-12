//! The line half of a shape region: where a drawn line ends, its arrowheads
//! and its dashes.

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2};
use opencalc_core::document::{Arrowhead, Dash, Point, ShapeSpec, Size};

use super::MIN_DRAG;
use crate::canvas::view::Snapper;

/// Fifteen degrees, the steps a constrained line turns in.
const ANGLE_STEP: f32 = std::f32::consts::PI / 12.0;

/// `end` moved, keeping its distance from `start`, to the nearest multiple of
/// fifteen degrees.
pub(in crate::canvas) fn constrained(start: Pos2, end: Pos2) -> Pos2 {
    let d = end - start;
    let len = d.length();
    if len == 0.0 {
        return end;
    }
    let angle = (d.y.atan2(d.x) / ANGLE_STEP).round() * ANGLE_STEP;
    start + Vec2::new(angle.cos(), angle.sin()) * len
}

/// The ends of a line drawn by dragging from `start` to `end` (document space):
/// snapped to `grid` if there is one, or turned to a multiple of fifteen degrees
/// with `constrain`; with `centre`, `start` is the middle. A click (almost no
/// drag) gives a level line of the usual length, starting at the click or centred
/// on it. Ends stay on the page (not left of or above it).
pub(in crate::canvas) fn drawn_line(
    start: Pos2,
    end: Pos2,
    grid: Option<Snapper>,
    constrain: bool,
    centre: bool,
) -> (Point, Point) {
    let dragged = (end - start).abs();
    let (mut a, mut b) = if dragged.x < MIN_DRAG && dragged.y < MIN_DRAG {
        let at = grid.map_or(start, |g| g.snap(start));
        let half = DEFAULT_LINE / 2.0;
        if centre {
            (at - Vec2::new(half, 0.0), at + Vec2::new(half, 0.0))
        } else {
            (at, at + Vec2::new(DEFAULT_LINE, 0.0))
        }
    } else {
        let snap_to = |p: Pos2| grid.map_or(p, |g| g.snap(p));
        let a = snap_to(start);
        let b = if constrain {
            constrained(a, end)
        } else {
            snap_to(end)
        };
        if centre {
            (a - (b - a), b)
        } else {
            (a, b)
        }
    };
    a = Pos2::new(a.x.max(0.0), a.y.max(0.0));
    b = Pos2::new(b.x.max(0.0), b.y.max(0.0));
    (Point::new(a.x, a.y), Point::new(b.x, b.y))
}

/// An arrowhead's length, as a multiple of the line width.
const HEAD_PER_WIDTH: f32 = 6.0;
/// The usual length of a line made by a click, in document units.
const DEFAULT_LINE: f32 = 160.0;

/// Where one end of a line carries an arrowhead: the pieces to paint and how far
/// the line itself must stop short of the end so it does not poke through.
pub(super) struct Head {
    pub(super) shapes: Vec<Shape>,
    pub(super) trim: f32,
}

/// The arrowhead of `kind` with its tip at `tip`, on a line arriving along `dir`
/// (a unit vector pointing at the tip). `len` is its length and `width` the line's.
pub(super) fn head(
    kind: Arrowhead,
    tip: Pos2,
    dir: Vec2,
    len: f32,
    width: f32,
    colour: Color32,
) -> Head {
    let side = Vec2::new(-dir.y, dir.x);
    let back = tip - dir * len;
    let fill = |points: Vec<Pos2>| Shape::convex_polygon(points, colour, Stroke::NONE);
    match kind {
        Arrowhead::None => Head {
            shapes: Vec::new(),
            trim: 0.0,
        },
        Arrowhead::Triangle => Head {
            shapes: vec![fill(vec![
                tip,
                back + side * len * 0.4,
                back - side * len * 0.4,
            ])],
            trim: len * 0.9,
        },
        Arrowhead::Open => {
            let stroke = Stroke::new(width, colour);
            Head {
                shapes: vec![
                    Shape::line_segment([tip, back + side * len * 0.4], stroke),
                    Shape::line_segment([tip, back - side * len * 0.4], stroke),
                ],
                trim: 0.0,
            }
        }
        Arrowhead::Circle => {
            let r = len * 0.4;
            Head {
                shapes: vec![Shape::circle_filled(tip - dir * r, r, colour)],
                trim: r,
            }
        }
        Arrowhead::Diamond => {
            let mid = tip - dir * (len / 2.0);
            Head {
                shapes: vec![fill(vec![
                    tip,
                    mid + side * len * 0.35,
                    back,
                    mid - side * len * 0.35,
                ])],
                trim: len / 2.0,
            }
        }
    }
}

/// Paints a line from `a` to `b` (screen space) with the spec's heads and dashes.
pub(super) fn paint_line_between(
    painter: &Painter,
    spec: &ShapeSpec,
    a: Pos2,
    b: Pos2,
    stroke_colour: Color32,
    zoom: f32,
) {
    let width = (spec.stroke_width * zoom).max(0.0);
    let span = b - a;
    let length = span.length();
    if length < 0.5 {
        return;
    }
    let dir = span / length;
    // Head length follows the line width, but a hairline still gets a visible head.
    let len = HEAD_PER_WIDTH * spec.stroke_width.max(1.5) * zoom;
    let at_start = head(spec.arrow_start, a, -dir, len, width, stroke_colour);
    let at_end = head(spec.arrow_end, b, dir, len, width, stroke_colour);
    // Never trim away more than the line has.
    let room = length * 0.5;
    let (from, to) = (
        a + dir * at_start.trim.min(room),
        b - dir * at_end.trim.min(room),
    );
    let stroke = Stroke::new(width, stroke_colour);
    match dash_lengths(spec.dash, width, zoom) {
        None => {
            painter.add(Shape::line_segment([from, to], stroke));
        }
        Some((dash, gap)) => {
            painter.extend(Shape::dashed_line(&[from, to], stroke, dash, gap));
        }
    }
    painter.extend(at_start.shapes);
    painter.extend(at_end.shapes);
}

/// The ends of a line region in screen space, given where its box starts on
/// screen and the zoom.
pub(in crate::canvas) fn line_ends_on_screen(
    spec: &ShapeSpec,
    size: Size,
    top_left: Pos2,
    zoom: f32,
) -> (Pos2, Pos2) {
    let (a, b) = spec.line_ends(Point::new(0.0, 0.0), size);
    (
        top_left + Vec2::new(a.x, a.y) * zoom,
        top_left + Vec2::new(b.x, b.y) * zoom,
    )
}

/// `(dash, gap)` lengths for a broken outline of `width`; none for a solid one.
pub(super) fn dash_lengths(dash: Dash, width: f32, zoom: f32) -> Option<(f32, f32)> {
    match dash {
        Dash::Solid => None,
        Dash::Dashed => Some((6.0 * zoom + 2.0 * width, 4.0 * zoom + width)),
        Dash::Dotted => Some((width.max(1.0), 2.0 * zoom + 1.5 * width)),
    }
}
