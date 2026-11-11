//! Painting and hit-testing a shape region: a rectangle, rounded rectangle or
//! ellipse, filled and outlined as its settings say.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use opencalc_core::document::{Dash, Point, Region, RegionKind, ShapeKind, ShapeSpec, Size};

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
fn outline(kind: ShapeKind, rect: Rect, radius: f32) -> Vec<Pos2> {
    match kind {
        ShapeKind::Rectangle => vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ],
        ShapeKind::RoundedRectangle => {
            let r = radius.clamp(0.0, rect.width().min(rect.height()) / 2.0);
            if r <= 0.0 {
                return outline(ShapeKind::Rectangle, rect, 0.0);
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
    let pts = outline(kind, rect, ShapeSpec::default().corner_radius);
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
    let width = (spec.stroke_width * zoom).max(0.0);
    // The outline is drawn inside the box, so the box is also the shape's extent.
    let inset = if stroke_colour.is_some() {
        width / 2.0
    } else {
        0.0
    };
    let body = rect.shrink(inset.min(rect.width().min(rect.height()) / 2.0));
    let pts = outline(spec.kind, body, spec.corner_radius * zoom);
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

/// Whether a click at `p` (document space) is on the shape whose box is `rect`.
/// A shape with a visible fill is hit anywhere inside it; one without is hit only
/// near its outline, so a large empty frame does not block what lies inside it.
pub(super) fn hits(spec: &ShapeSpec, rect: Rect, p: Pos2) -> bool {
    let filled = spec
        .fill
        .is_some_and(|f| f.resolve([0, 0, 0, AUTO_FILL_ALPHA])[3] > 0);
    let reach = OUTLINE_REACH.max(spec.stroke_width / 2.0);
    match spec.kind {
        ShapeKind::Rectangle | ShapeKind::RoundedRectangle => {
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
        assert_eq!(outline(ShapeKind::Rectangle, b, 0.0).len(), 4);
        assert_eq!(outline(ShapeKind::RoundedRectangle, b, 0.0).len(), 4);
        for kind in [ShapeKind::RoundedRectangle, ShapeKind::Ellipse] {
            let pts = outline(kind, b, 1000.0);
            assert!(pts.len() > 8);
            let bounds = b.expand(0.01);
            assert!(pts.iter().all(|p| bounds.contains(*p)), "{kind:?}");
        }
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
