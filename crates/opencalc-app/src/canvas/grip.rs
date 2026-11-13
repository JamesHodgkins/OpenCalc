//! Grips: draggable points on a selected shape that change one of its settings
//! (as opposed to the eight resize handles, which change its box). The first is
//! the corner grip of a rectangle. All in screen or document space as noted, and
//! pure, so it is tested without a window.

use eframe::egui::{Pos2, Rect};
use opencalc_core::document::{Region, RegionKind, ShapeKind, ShapeSpec, MAX_RADIUS};

use super::magnet::{snap_point, Guide, Targets};
use super::shape::{constrained, line_ends_on_screen};
use super::view::Snapper;

/// Pixels from a grip's centre that still count as on it.
pub const REACH: f32 = 7.0;
/// How far inside the top and right edges the corner grip sits, in pixels.
const INSET: f32 = 14.0;
/// A shape smaller than this on screen, in pixels, shows no grip (it would sit on a handle).
const MIN_SHOWN: f32 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// The corner radius of a rectangle.
    Corner,
    /// The start of a line.
    Start,
    /// The end of a line.
    End,
}

impl Grip {
    /// For a line's end grips: whether it is the start. `None` for any other grip.
    pub fn line_end(self) -> Option<bool> {
        match self {
            Grip::Start => Some(true),
            Grip::End => Some(false),
            Grip::Corner => None,
        }
    }
}

/// The grips a region shows when it is the one selected shape, with the screen
/// point of each. `screen` is the region's box on screen and `zoom` the view's.
pub fn grips(r: &Region, screen: Rect, zoom: f32) -> Vec<(Grip, Pos2)> {
    let RegionKind::Shape(spec) = &r.kind else {
        return Vec::new();
    };
    if r.locked {
        return Vec::new();
    }
    if spec.kind == ShapeKind::Line {
        let (a, b) = line_ends_on_screen(spec, r.size, screen.min, zoom);
        return vec![(Grip::Start, a), (Grip::End, b)];
    }
    if spec.kind != ShapeKind::Rectangle {
        return Vec::new();
    }
    if screen.width().min(screen.height()) < MIN_SHOWN {
        return Vec::new();
    }
    // The radius the painter uses: never past half the shorter side.
    let radius = (spec.corner_radius * zoom).clamp(0.0, screen.width().min(screen.height()) / 2.0);
    vec![(
        Grip::Corner,
        Pos2::new(screen.right() - INSET - radius, screen.top() + INSET),
    )]
}

/// The end of a line that does not move while `grip` is dragged, in document
/// space; the origin for any other grip.
pub fn fixed_end(grip: Grip, r: &Region) -> Pos2 {
    let (RegionKind::Shape(spec), Some(is_start)) = (&r.kind, grip.line_end()) else {
        return Pos2::ZERO;
    };
    let (a, b) = spec.line_ends(r.pos, r.size);
    let fixed = if is_start { b } else { a };
    Pos2::new(fixed.x, fixed.y)
}

/// Where the moving end of a line goes when it is dragged to the document point
/// `p`: snapped to `grid`, or with `constrain` turned about `fixed` to a multiple of
/// fifteen degrees. It stays on the page (not left of or above it).
pub fn line_end_dragged(
    fixed: Pos2,
    p: Pos2,
    grid: Option<Snapper>,
    constrain: bool,
    edges: Option<(&Targets, f32)>,
) -> (Pos2, Vec<Guide>) {
    let mut guides = Vec::new();
    let moved = if constrain {
        constrained(fixed, p)
    } else {
        let on_grid = grid.map_or(p, |g| g.snap(p));
        match edges {
            // Another region's edge beats the grid on the axis where one is near.
            Some((t, reach)) => {
                let s = snap_point(p, t, reach, true, true);
                guides = s.guides.clone();
                Pos2::new(
                    s.dx.map_or(on_grid.x, |d| p.x + d),
                    s.dy.map_or(on_grid.y, |d| p.y + d),
                )
            }
            None => on_grid,
        }
    };
    (Pos2::new(moved.x.max(0.0), moved.y.max(0.0)), guides)
}

/// The grip under the screen point `p`, if any.
pub fn grip_at(r: &Region, screen: Rect, zoom: f32, p: Pos2) -> Option<Grip> {
    grips(r, screen, zoom)
        .into_iter()
        .map(|(g, at)| (g, at.distance(p)))
        .filter(|(_, d)| *d <= REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(g, _)| g)
}

/// The settings after dragging `grip` to the screen point `p`, for a shape whose
/// box is `screen` on screen.
pub fn dragged(grip: Grip, spec: &ShapeSpec, screen: Rect, zoom: f32, p: Pos2) -> ShapeSpec {
    let mut next = spec.clone();
    match grip {
        Grip::Start | Grip::End => {}
        Grip::Corner => {
            let most = screen.width().min(screen.height()) / 2.0;
            let radius = ((screen.right() - INSET - p.x) / zoom).clamp(0.0, most / zoom);
            next.corner_radius = radius.round().clamp(0.0, MAX_RADIUS);
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::{Point, Size, Worksheet};

    fn rect_region(kind: ShapeKind, radius: f32) -> Region {
        let mut w = Worksheet::new("t");
        let mut r = w.new_region(RegionKind::shape(kind), Point::new(0.0, 0.0));
        r.size = Size::new(200.0, 100.0);
        if let RegionKind::Shape(s) = &mut r.kind {
            s.corner_radius = radius;
        }
        r
    }

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 100.0), eframe::egui::vec2(200.0, 100.0))
    }

    #[test]
    fn only_a_rectangle_has_a_corner_grip_and_not_when_locked_or_tiny() {
        assert_eq!(
            grips(&rect_region(ShapeKind::Rectangle, 0.0), screen(), 1.0).len(),
            1
        );
        assert!(grips(&rect_region(ShapeKind::Ellipse, 0.0), screen(), 1.0).is_empty());
        let mut locked = rect_region(ShapeKind::Rectangle, 0.0);
        locked.locked = true;
        assert!(grips(&locked, screen(), 1.0).is_empty());
        let tiny = Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(30.0, 30.0));
        assert!(grips(&rect_region(ShapeKind::Rectangle, 0.0), tiny, 1.0).is_empty());
    }

    #[test]
    fn a_line_has_a_grip_at_each_end_and_the_fixed_end_is_the_other_one() {
        let mut r = rect_region(ShapeKind::Line, 0.0);
        r.pos = Point::new(100.0, 100.0);
        r.size = Size::new(200.0, 100.0);
        let gs = grips(&r, screen(), 1.0);
        assert_eq!(
            gs,
            vec![
                (Grip::Start, Pos2::new(100.0, 100.0)),
                (Grip::End, Pos2::new(300.0, 200.0))
            ]
        );
        assert_eq!(
            grip_at(&r, screen(), 1.0, Pos2::new(298.0, 201.0)),
            Some(Grip::End)
        );
        assert_eq!(fixed_end(Grip::Start, &r), Pos2::new(300.0, 200.0));
        assert_eq!(fixed_end(Grip::End, &r), Pos2::new(100.0, 100.0));
        r.locked = true;
        assert!(grips(&r, screen(), 1.0).is_empty());
    }

    #[test]
    fn dragging_a_line_end_snaps_constrains_and_stays_on_the_page() {
        let fixed = Pos2::new(100.0, 100.0);
        assert_eq!(
            line_end_dragged(
                fixed,
                Pos2::new(203.0, 148.0),
                Some(Snapper::plain(10.0)),
                false,
                None
            )
            .0,
            Pos2::new(200.0, 150.0)
        );
        let (out, _) = line_end_dragged(
            fixed,
            Pos2::new(300.0, 103.0),
            Some(Snapper::plain(10.0)),
            true,
            None,
        );
        assert!((out.y - 100.0).abs() < 0.1, "turned level: {out:?}");
        assert_eq!(
            line_end_dragged(fixed, Pos2::new(-20.0, -5.0), None, false, None).0,
            Pos2::new(0.0, 0.0)
        );
        // Another region's edge near the pointer beats the grid on that axis only.
        let t = Targets {
            xs: vec![207.0],
            ys: vec![],
        };
        let (out, guides) = line_end_dragged(
            fixed,
            Pos2::new(204.0, 148.0),
            Some(Snapper::plain(10.0)),
            false,
            Some((&t, 6.0)),
        );
        assert_eq!(out, Pos2::new(207.0, 150.0));
        assert_eq!(guides.len(), 1);
    }

    #[test]
    fn the_grip_moves_left_as_the_radius_grows_and_dragging_it_back_gives_the_radius() {
        let r = rect_region(ShapeKind::Rectangle, 20.0);
        let (_, at) = grips(&r, screen(), 1.0)[0];
        assert_eq!(at, Pos2::new(300.0 - INSET - 20.0, 100.0 + INSET));
        assert_eq!(
            grip_at(&r, screen(), 1.0, at + eframe::egui::vec2(3.0, 2.0)),
            Some(Grip::Corner)
        );
        assert_eq!(grip_at(&r, screen(), 1.0, Pos2::new(200.0, 150.0)), None);
        let RegionKind::Shape(spec) = &r.kind else {
            panic!()
        };
        let out = dragged(Grip::Corner, spec, screen(), 1.0, at);
        assert_eq!(out.corner_radius, 20.0);
    }

    #[test]
    fn dragging_stays_between_square_and_half_the_shorter_side_and_scales_with_zoom() {
        let spec = ShapeSpec::of(ShapeKind::Rectangle);
        let far_right = dragged(Grip::Corner, &spec, screen(), 1.0, Pos2::new(900.0, 0.0));
        assert_eq!(far_right.corner_radius, 0.0);
        let far_left = dragged(Grip::Corner, &spec, screen(), 1.0, Pos2::new(-900.0, 0.0));
        assert_eq!(far_left.corner_radius, 50.0, "half of the 100 high side");
        // At 2x zoom the box is twice as big on screen; 40 px of pull is 20 units.
        let big = Rect::from_min_size(Pos2::new(0.0, 0.0), eframe::egui::vec2(400.0, 200.0));
        let out = dragged(
            Grip::Corner,
            &spec,
            big,
            2.0,
            Pos2::new(400.0 - INSET - 40.0, 0.0),
        );
        assert_eq!(out.corner_radius, 20.0);
    }
}
