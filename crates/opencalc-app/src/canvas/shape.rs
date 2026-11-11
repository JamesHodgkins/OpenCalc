//! Painting and hit-testing a shape region: a rectangle (square or rounded
//! corners), ellipse or line, filled and outlined as its settings say.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use opencalc_core::document::{
    Arrowhead, Dash, Point, Region, RegionKind, ShapeKind, ShapeSpec, Size,
};

use super::adapt::vec;
use super::paint::Place;
use super::Env;

/// How near the pointer must be to an outline to hit a shape that has no fill,
/// in document units.
const OUTLINE_REACH: f32 = 6.0;
/// Segments to a corner arc, and to a whole ellipse.
const ARC_STEPS: usize = 8;
const ELLIPSE_STEPS: usize = 96;
/// Opacity of the automatic fill, out of 255.
const AUTO_FILL_ALPHA: u8 = 48;

fn to_color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn to_array(c: Color32) -> [u8; 4] {
    c.to_srgba_unmultiplied()
}

/// The painted fill colour, if the shape has a visible one. Automatic is the
/// theme's accent colour, faintly.
fn fill_of(spec: &ShapeSpec, accent: Color32) -> Option<Color32> {
    let [r, g, b, _] = to_array(accent);
    let c = spec.fill?.resolve([r, g, b, AUTO_FILL_ALPHA]);
    (c[3] > 0).then(|| to_color(c))
}

/// The painted outline colour, if the shape has a visible one. Automatic is the
/// text colour.
fn stroke_of(spec: &ShapeSpec, text: Color32) -> Option<Color32> {
    let c = spec.stroke?.resolve(to_array(text));
    (c[3] > 0 && spec.stroke_width > 0.0).then(|| to_color(c))
}

/// The corners of the shape's outline, going round once. A rectangle is four
/// points, a rounded one adds the arcs, an ellipse is a many-sided polygon.
fn outline(kind: ShapeKind, rect: Rect, radius: f32, sides: u8) -> Vec<Pos2> {
    match kind {
        ShapeKind::Triangle => vec![
            Pos2::new(rect.center().x, rect.top()),
            rect.right_bottom(),
            rect.left_bottom(),
        ],
        ShapeKind::Diamond => vec![
            Pos2::new(rect.center().x, rect.top()),
            Pos2::new(rect.right(), rect.center().y),
            Pos2::new(rect.center().x, rect.bottom()),
            Pos2::new(rect.left(), rect.center().y),
        ],
        ShapeKind::Polygon => {
            let n = sides.max(3) as usize;
            let (c, rx, ry) = (rect.center(), rect.width() / 2.0, rect.height() / 2.0);
            (0..n)
                .map(|k| {
                    let a =
                        std::f32::consts::TAU * k as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
                    Pos2::new(c.x + rx * a.cos(), c.y + ry * a.sin())
                })
                .collect()
        }
        // A line is not a closed outline; it is painted by `paint_line`.
        ShapeKind::Line => Vec::new(),
        ShapeKind::Rectangle => {
            let r = radius.clamp(0.0, rect.width().min(rect.height()) / 2.0);
            if r <= 0.0 {
                return vec![
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                ];
            }
            // Each corner's arc centre and the angle it starts from, going
            // clockwise on screen from the top-left.
            let corners = [
                (rect.left() + r, rect.top() + r, 180.0_f32),
                (rect.right() - r, rect.top() + r, 270.0),
                (rect.right() - r, rect.bottom() - r, 0.0),
                (rect.left() + r, rect.bottom() - r, 90.0),
            ];
            let mut pts = Vec::with_capacity(4 * (ARC_STEPS + 1));
            for (cx, cy, start) in corners {
                for k in 0..=ARC_STEPS {
                    let a = (start + 90.0 * k as f32 / ARC_STEPS as f32).to_radians();
                    pts.push(Pos2::new(cx + r * a.cos(), cy + r * a.sin()));
                }
            }
            pts
        }
        ShapeKind::Ellipse => {
            let (c, rx, ry) = (rect.center(), rect.width() / 2.0, rect.height() / 2.0);
            (0..ELLIPSE_STEPS)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / ELLIPSE_STEPS as f32;
                    Pos2::new(c.x + rx * a.cos(), c.y + ry * a.sin())
                })
                .collect()
        }
    }
}

/// Fifteen degrees, the steps a constrained line turns in.
const ANGLE_STEP: f32 = std::f32::consts::PI / 12.0;

/// `end` moved, keeping its distance from `start`, to the nearest multiple of
/// fifteen degrees.
pub(super) fn constrained(start: Pos2, end: Pos2) -> Pos2 {
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
pub(super) fn drawn_line(
    start: Pos2,
    end: Pos2,
    grid: Option<f32>,
    constrain: bool,
    centre: bool,
) -> (Point, Point) {
    let dragged = (end - start).abs();
    let (mut a, mut b) = if dragged.x < MIN_DRAG && dragged.y < MIN_DRAG {
        let at = grid.map_or(start, |g| super::view::snap(start, g));
        let half = DEFAULT_LINE / 2.0;
        if centre {
            (at - Vec2::new(half, 0.0), at + Vec2::new(half, 0.0))
        } else {
            (at, at + Vec2::new(DEFAULT_LINE, 0.0))
        }
    } else {
        let snap_to = |p: Pos2| grid.map_or(p, |g| super::view::snap(p, g));
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
struct Head {
    shapes: Vec<Shape>,
    trim: f32,
}

/// The arrowhead of `kind` with its tip at `tip`, on a line arriving along `dir`
/// (a unit vector pointing at the tip). `len` is its length and `width` the line's.
fn head(kind: Arrowhead, tip: Pos2, dir: Vec2, len: f32, width: f32, colour: Color32) -> Head {
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
fn paint_line_between(
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
pub(super) fn line_ends_on_screen(
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
fn dash_lengths(dash: Dash, width: f32, zoom: f32) -> Option<(f32, f32)> {
    match dash {
        Dash::Solid => None,
        Dash::Dashed => Some((6.0 * zoom + 2.0 * width, 4.0 * zoom + width)),
        Dash::Dotted => Some((width.max(1.0), 2.0 * zoom + 1.5 * width)),
    }
}

/// A drag shorter than this, in document units, counts as a click.
const MIN_DRAG: f32 = 6.0;
/// The smallest a drawn shape may be, in document units.
const MIN_SIDE: f32 = 8.0;

/// The box for a shape drawn from `start` to `end` (document space): the dragged
/// rectangle (see `resize::drawn` for `square` and `centre`), snapped to `grid` if
/// there is one and kept a usable size. A click (almost no drag) gives the shape's
/// usual size with its corner, or with `centre` its middle, at the click.
pub(super) fn drawn_box(
    kind: ShapeKind,
    start: Pos2,
    end: Pos2,
    grid: Option<f32>,
    square: bool,
    centre: bool,
) -> (Point, Size) {
    let dragged = (end - start).abs();
    if dragged.x < MIN_DRAG && dragged.y < MIN_DRAG {
        let size = RegionKind::shape(kind).default_size();
        let at = grid.map_or(start, |g| super::view::snap(start, g));
        let min = if centre {
            at - Vec2::new(size.w, size.h) / 2.0
        } else {
            at
        };
        return (Point::new(min.x.max(0.0), min.y.max(0.0)), size);
    }
    let r = super::resize::drawn(start, end, grid, square, centre);
    (
        Point::new(r.min.x.max(0.0), r.min.y.max(0.0)),
        Size::new(r.width().max(MIN_SIDE), r.height().max(MIN_SIDE)),
    )
}

/// The outline of a shape being drawn, in the accent colour.
pub(super) fn preview(painter: &Painter, kind: ShapeKind, rect: Rect, colour: Color32) {
    if kind == ShapeKind::Line {
        return;
    }
    let d = ShapeSpec::default();
    let pts = outline(kind, rect, d.corner_radius, d.sides);
    let stroke = Stroke::new(1.5_f32, colour);
    painter.add(Shape::convex_polygon(
        pts.clone(),
        colour.gamma_multiply(0.15),
        Stroke::NONE,
    ));
    painter.add(Shape::closed_line(pts, stroke));
}

/// Paints the shape in the region's box; returns the box size.
pub(super) fn paint(
    painter: &Painter,
    env: &Env,
    r: &Region,
    spec: &ShapeSpec,
    place: Place,
) -> Vec2 {
    let Place {
        top_left,
        zoom,
        skip_content,
    } = place;
    if skip_content {
        return vec(r.size);
    }
    let rect = Rect::from_min_size(top_left, vec(r.size) * zoom);
    let stroke_colour = stroke_of(spec, env.paper.text);
    if spec.kind == ShapeKind::Line {
        if let Some(colour) = stroke_colour {
            let (a, b) = line_ends_on_screen(spec, r.size, top_left, zoom);
            paint_line_between(painter, spec, a, b, colour, zoom);
        }
        return vec(r.size);
    }
    let width = (spec.stroke_width * zoom).max(0.0);
    // The outline is drawn inside the box, so the box is also the shape's extent.
    let inset = if stroke_colour.is_some() {
        width / 2.0
    } else {
        0.0
    };
    let body = rect.shrink(inset.min(rect.width().min(rect.height()) / 2.0));
    let pts = outline(spec.kind, body, spec.corner_radius * zoom, spec.sides);
    if let Some(fill) = fill_of(spec, env.paper.accent) {
        painter.add(Shape::convex_polygon(pts.clone(), fill, Stroke::NONE));
    }
    if let Some(colour) = stroke_colour {
        let stroke = Stroke::new(width, colour);
        match dash_lengths(spec.dash, width, zoom) {
            None => {
                painter.add(Shape::closed_line(pts, stroke));
            }
            Some((dash, gap)) => {
                let mut closed = pts;
                closed.push(closed[0]);
                painter.extend(Shape::dashed_line(&closed, stroke, dash, gap));
            }
        }
    }
    vec(r.size)
}

/// Whether `p` is inside the polygon `pts` (even-odd rule).
fn inside(pts: &[Pos2], p: Pos2) -> bool {
    let mut odd = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// How far `p` is from the segment `a`-`b`.
pub(super) fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Whether a click at `p` (document space) is on the shape whose box is `rect`.
/// A shape with a visible fill is hit anywhere inside it; one without is hit only
/// near its outline, so a large empty frame does not block what lies inside it.
pub(super) fn hits(spec: &ShapeSpec, rect: Rect, p: Pos2) -> bool {
    let filled = spec
        .fill
        .is_some_and(|f| f.resolve([0, 0, 0, AUTO_FILL_ALPHA])[3] > 0);
    let reach = OUTLINE_REACH.max(spec.stroke_width / 2.0);
    match spec.kind {
        ShapeKind::Triangle | ShapeKind::Diamond | ShapeKind::Polygon => {
            let pts = outline(spec.kind, rect, 0.0, spec.sides);
            let near_edge = (0..pts.len())
                .any(|i| distance_to_segment(p, pts[i], pts[(i + 1) % pts.len()]) <= reach);
            near_edge || (filled && inside(&pts, p))
        }
        ShapeKind::Line => {
            let (a, b) = spec.line_ends(
                Point::new(rect.min.x, rect.min.y),
                Size::new(rect.width(), rect.height()),
            );
            distance_to_segment(p, Pos2::new(a.x, a.y), Pos2::new(b.x, b.y)) <= reach
        }
        ShapeKind::Rectangle => {
            if !rect.expand(reach).contains(p) {
                return false;
            }
            filled
                || !rect
                    .shrink(reach.min(rect.width().min(rect.height()) / 2.0))
                    .contains(p)
        }
        ShapeKind::Ellipse => {
            let (c, rx, ry) = (rect.center(), rect.width() / 2.0, rect.height() / 2.0);
            if rx <= 0.0 || ry <= 0.0 {
                return false;
            }
            // Distance from the curve, near enough in document units: how far
            // past the ellipse (1.0 is on it) times the smaller radius.
            let rho = (((p.x - c.x) / rx).powi(2) + ((p.y - c.y) / ry).powi(2)).sqrt();
            let off = (rho - 1.0) * rx.min(ry);
            if filled {
                off <= reach
            } else {
                off.abs() <= reach
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::Colour;

    fn frame() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(200.0, 100.0))
    }

    fn spec(kind: ShapeKind, filled: bool) -> ShapeSpec {
        ShapeSpec {
            fill: filled.then(|| Colour::theme(4, 2)),
            ..ShapeSpec::of(kind)
        }
    }

    #[test]
    fn dragging_makes_the_box_in_any_direction_and_a_click_the_usual_size() {
        let k = ShapeKind::Rectangle;
        let (p, s) = drawn_box(
            k,
            Pos2::new(100.0, 100.0),
            Pos2::new(50.0, 160.0),
            None,
            false,
            false,
        );
        assert_eq!((p, s), (Point::new(50.0, 100.0), Size::new(50.0, 60.0)));
        let (p, s) = drawn_box(
            k,
            Pos2::new(70.0, 80.0),
            Pos2::new(72.0, 81.0),
            None,
            false,
            false,
        );
        assert_eq!(p, Point::new(70.0, 80.0));
        assert_eq!(s, RegionKind::shape(k).default_size());
    }

    #[test]
    fn a_drawn_box_snaps_to_the_grid_and_never_goes_off_the_page_or_vanishes() {
        let k = ShapeKind::Ellipse;
        let (p, s) = drawn_box(
            k,
            Pos2::new(13.0, 27.0),
            Pos2::new(98.0, 71.0),
            Some(10.0),
            false,
            false,
        );
        assert_eq!((p, s), (Point::new(10.0, 30.0), Size::new(90.0, 40.0)));
        let (p, _) = drawn_box(
            k,
            Pos2::new(-30.0, -5.0),
            Pos2::new(40.0, 40.0),
            None,
            false,
            false,
        );
        assert_eq!((p.x, p.y), (0.0, 0.0));
        let (_, s) = drawn_box(
            k,
            Pos2::new(0.0, 0.0),
            Pos2::new(40.0, 7.0),
            None,
            false,
            false,
        );
        assert!(s.h >= MIN_SIDE);
    }

    #[test]
    fn a_filled_rectangle_is_hit_inside_and_a_little_outside() {
        let s = spec(ShapeKind::Rectangle, true);
        assert!(hits(&s, frame(), Pos2::new(200.0, 150.0)));
        assert!(hits(&s, frame(), Pos2::new(97.0, 150.0)));
        assert!(!hits(&s, frame(), Pos2::new(80.0, 150.0)));
    }

    #[test]
    fn an_empty_frame_is_hit_only_on_its_outline() {
        let s = spec(ShapeKind::Rectangle, false);
        assert!(
            !hits(&s, frame(), Pos2::new(200.0, 150.0)),
            "the middle is free"
        );
        assert!(hits(&s, frame(), Pos2::new(101.0, 150.0)));
        assert!(hits(&s, frame(), Pos2::new(200.0, 198.0)));
    }

    #[test]
    fn a_fill_with_no_opacity_counts_as_no_fill() {
        let s = ShapeSpec {
            fill: Some(Colour::Rgba([1, 2, 3, 0])),
            ..ShapeSpec::of(ShapeKind::Rectangle)
        };
        assert!(!hits(&s, frame(), Pos2::new(200.0, 150.0)));
    }

    #[test]
    fn an_ellipse_is_hit_by_its_curve_not_its_corners() {
        let s = spec(ShapeKind::Ellipse, true);
        assert!(hits(&s, frame(), Pos2::new(200.0, 150.0)));
        assert!(
            !hits(&s, frame(), Pos2::new(102.0, 102.0)),
            "the box corner is outside the curve"
        );
        let empty = spec(ShapeKind::Ellipse, false);
        assert!(!hits(&empty, frame(), Pos2::new(200.0, 150.0)));
        assert!(hits(&empty, frame(), Pos2::new(101.0, 150.0)));
    }

    #[test]
    fn outlines_have_the_right_point_counts_and_stay_in_the_box() {
        let b = frame();
        assert_eq!(outline(ShapeKind::Rectangle, b, 0.0, 6).len(), 4);
        assert_eq!(outline(ShapeKind::Triangle, b, 0.0, 6).len(), 3);
        assert_eq!(outline(ShapeKind::Diamond, b, 0.0, 6).len(), 4);
        assert_eq!(outline(ShapeKind::Polygon, b, 0.0, 5).len(), 5);
        for kind in [ShapeKind::Rectangle, ShapeKind::Ellipse] {
            let pts = outline(kind, b, 1000.0, 6);
            assert!(pts.len() > 8);
            let bounds = b.expand(0.01);
            assert!(pts.iter().all(|p| bounds.contains(*p)), "{kind:?}");
        }
    }

    #[test]
    fn a_triangle_is_hit_inside_its_slopes_not_in_the_empty_corners() {
        // frame: x 100..300, y 100..200; the apex is at (200, 100).
        let s = spec(ShapeKind::Triangle, true);
        assert!(hits(&s, frame(), Pos2::new(200.0, 150.0)));
        assert!(hits(&s, frame(), Pos2::new(110.0, 195.0)), "a base corner");
        assert!(
            !hits(&s, frame(), Pos2::new(110.0, 110.0)),
            "above the left slope"
        );
        let empty = spec(ShapeKind::Triangle, false);
        assert!(!hits(&empty, frame(), Pos2::new(200.0, 150.0)));
        assert!(
            hits(&empty, frame(), Pos2::new(200.0, 198.0)),
            "on the base"
        );
    }

    #[test]
    fn a_diamond_and_a_polygon_are_hit_by_their_shape() {
        let d = spec(ShapeKind::Diamond, true);
        assert!(hits(&d, frame(), Pos2::new(200.0, 150.0)));
        assert!(
            !hits(&d, frame(), Pos2::new(110.0, 110.0)),
            "the box corner"
        );
        let p = ShapeSpec {
            sides: 3,
            ..spec(ShapeKind::Polygon, true)
        };
        assert!(hits(&p, frame(), Pos2::new(200.0, 150.0)));
        assert!(!hits(&p, frame(), Pos2::new(105.0, 105.0)));
    }

    #[test]
    fn a_line_is_hit_near_the_line_and_not_near_its_box() {
        let s = spec(ShapeKind::Line, false);
        // The frame runs (100,100) to (300,200).
        assert!(hits(&s, frame(), Pos2::new(200.0, 150.0)), "the middle");
        assert!(hits(&s, frame(), Pos2::new(200.0, 154.0)));
        assert!(!hits(&s, frame(), Pos2::new(200.0, 175.0)));
        assert!(
            !hits(&s, frame(), Pos2::new(110.0, 190.0)),
            "the other corner"
        );
        assert!(!hits(&s, frame(), Pos2::new(330.0, 220.0)), "past the end");
        let up = ShapeSpec { flip_y: true, ..s };
        assert!(hits(&up, frame(), Pos2::new(110.0, 190.0)), "rising line");
    }

    #[test]
    fn a_dragged_line_follows_the_pointer_and_a_click_makes_a_level_line() {
        let a = Pos2::new(50.0, 60.0);
        let (from, to) = drawn_line(a, Pos2::new(150.0, 160.0), None, false, false);
        assert_eq!(
            (from, to),
            (Point::new(50.0, 60.0), Point::new(150.0, 160.0))
        );
        let (from, to) = drawn_line(a, a, None, false, false);
        assert_eq!(
            (from, to),
            (Point::new(50.0, 60.0), Point::new(210.0, 60.0))
        );
        let (from, to) = drawn_line(a, a, None, false, true);
        assert_eq!(
            (from.x, to.x),
            (0.0_f32.max(50.0 - 80.0), 130.0),
            "centred, kept on the page"
        );
        let (from, to) = drawn_line(a, Pos2::new(250.0, 80.0), None, false, true);
        assert_eq!(
            from,
            Point::new(0.0, 40.0),
            "the mirror end is kept on the page"
        );
        assert_eq!(to, Point::new(250.0, 80.0));
    }

    #[test]
    fn a_constrained_line_turns_to_a_multiple_of_fifteen_degrees_and_keeps_its_length() {
        let a = Pos2::new(0.0, 0.0);
        let out = constrained(a, Pos2::new(100.0, 10.0));
        let angle = out.y.atan2(out.x).to_degrees();
        assert!((angle - 15.0).abs() < 1e-3 || angle.abs() < 1e-3, "{angle}");
        assert!((out.to_vec2().length() - Pos2::new(100.0, 10.0).to_vec2().length()).abs() < 1e-3);
        let level = constrained(a, Pos2::new(100.0, 3.0));
        assert!(level.y.abs() < 1e-3);
        assert_eq!(constrained(a, a), a);
    }

    #[test]
    fn heads_point_at_the_tip_and_the_line_stops_short_of_filled_ones() {
        let c = Color32::BLACK;
        let tip = Pos2::new(100.0, 0.0);
        let dir = Vec2::new(1.0, 0.0);
        assert!(head(Arrowhead::None, tip, dir, 10.0, 1.0, c)
            .shapes
            .is_empty());
        let tri = head(Arrowhead::Triangle, tip, dir, 10.0, 1.0, c);
        assert_eq!(tri.shapes.len(), 1);
        assert!(tri.trim > 0.0 && tri.trim <= 10.0);
        assert_eq!(
            head(Arrowhead::Open, tip, dir, 10.0, 1.0, c).shapes.len(),
            2
        );
        assert_eq!(head(Arrowhead::Open, tip, dir, 10.0, 1.0, c).trim, 0.0);
        assert_eq!(
            head(Arrowhead::Circle, tip, dir, 10.0, 1.0, c).shapes.len(),
            1
        );
        assert_eq!(
            head(Arrowhead::Diamond, tip, dir, 10.0, 1.0, c)
                .shapes
                .len(),
            1
        );
    }

    #[test]
    fn automatic_colours_follow_the_theme_and_none_draws_nothing() {
        let text = Color32::from_rgb(10, 20, 30);
        let accent = Color32::from_rgb(40, 50, 60);
        let auto = ShapeSpec {
            fill: Some(Colour::Auto),
            stroke: Some(Colour::Auto),
            ..ShapeSpec::default()
        };
        assert_eq!(stroke_of(&auto, text), Some(text));
        assert_eq!(fill_of(&auto, accent).unwrap().a(), AUTO_FILL_ALPHA);
        let none = ShapeSpec {
            fill: None,
            stroke: None,
            ..ShapeSpec::default()
        };
        assert_eq!(
            (fill_of(&none, accent), stroke_of(&none, text)),
            (None, None)
        );
        let thin = ShapeSpec {
            stroke_width: 0.0,
            ..ShapeSpec::default()
        };
        assert_eq!(stroke_of(&thin, text), None);
    }
}
