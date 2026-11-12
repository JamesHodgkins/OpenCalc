//! Resize handles: where they are, which one a press is on, and the new box a
//! drag gives. All in document space and pure, so it is tested without a window.

use eframe::egui::{CursorIcon, Pos2, Rect, Vec2};

use super::view::snap;

/// The smallest a region may be made, in document units.
pub const MIN_SIDE: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::NW,
        Handle::N,
        Handle::NE,
        Handle::E,
        Handle::SE,
        Handle::S,
        Handle::SW,
        Handle::W,
    ];

    /// Which way the handle pulls on each axis: -1 left or up, 1 right or down,
    /// 0 not at all.
    fn dir(self) -> (i8, i8) {
        match self {
            Handle::N => (0, -1),
            Handle::NE => (1, -1),
            Handle::E => (1, 0),
            Handle::SE => (1, 1),
            Handle::S => (0, 1),
            Handle::SW => (-1, 1),
            Handle::W => (-1, 0),
            Handle::NW => (-1, -1),
        }
    }

    /// Whether the handle pulls on the x axis and on the y axis.
    pub fn pulls(self) -> (bool, bool) {
        let (dx, dy) = self.dir();
        (dx != 0, dy != 0)
    }

    pub fn is_corner(self) -> bool {
        let (dx, dy) = self.dir();
        dx != 0 && dy != 0
    }

    /// Where the handle sits on `r`.
    pub fn point(self, r: Rect) -> Pos2 {
        let (dx, dy) = self.dir();
        let pick = |d: i8, lo: f32, mid: f32, hi: f32| match d {
            -1 => lo,
            1 => hi,
            _ => mid,
        };
        Pos2::new(
            pick(dx, r.min.x, r.center().x, r.max.x),
            pick(dy, r.min.y, r.center().y, r.max.y),
        )
    }

    pub fn cursor(self) -> CursorIcon {
        match self {
            Handle::N | Handle::S => CursorIcon::ResizeVertical,
            Handle::E | Handle::W => CursorIcon::ResizeHorizontal,
            Handle::NE | Handle::SW => CursorIcon::ResizeNeSw,
            Handle::NW | Handle::SE => CursorIcon::ResizeNwSe,
        }
    }
}

/// The handle under `p` on screen, if any. `reach` is how near counts, in pixels.
pub fn handle_at(screen_rect: Rect, p: Pos2, reach: f32) -> Option<Handle> {
    Handle::ALL
        .into_iter()
        .map(|h| (h, h.point(screen_rect).distance(p)))
        .filter(|(_, d)| *d <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(h, _)| h)
}

/// One axis: the new `(min, max)` when the edge in direction `d` is pulled to
/// `p`. With `centre` both sides move together about the middle.
fn axis(min: f32, max: f32, d: i8, p: f32, centre: bool) -> (f32, f32) {
    if d == 0 {
        return (min, max);
    }
    if centre {
        let c = (min + max) / 2.0;
        let half = (p - c).abs().max(MIN_SIDE / 2.0);
        return (c - half, c + half);
    }
    if d > 0 {
        (min, p.max(min + MIN_SIDE))
    } else {
        (p.min(max - MIN_SIDE), max)
    }
}

/// The box after pulling `handle` of `orig` to the pointer at `p`.
///
/// - `grid`: snap the pulled edge to this spacing.
/// - `ratio`: keep the original proportions (a corner handle only).
/// - `centre`: the opposite side moves the other way, so the middle stays put.
///
/// An edge never crosses the opposite one: the box stops at [`MIN_SIDE`].
pub fn resized(
    orig: Rect,
    handle: Handle,
    p: Pos2,
    grid: Option<f32>,
    ratio: bool,
    centre: bool,
) -> Rect {
    let (dx, dy) = handle.dir();
    let keep = ratio && handle.is_corner() && orig.width() > 0.0 && orig.height() > 0.0;
    let p = match grid {
        Some(g) if !keep => snap(p, g),
        _ => p,
    };
    let (x0, x1) = axis(orig.min.x, orig.max.x, dx, p.x, centre);
    let (y0, y1) = axis(orig.min.y, orig.max.y, dy, p.y, centre);
    let mut r = Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1));
    if keep {
        // Scale by whichever axis was pulled further, then hang the box on the
        // corner (or the middle) that stays fixed.
        let s = (r.width() / orig.width())
            .max(r.height() / orig.height())
            .max(MIN_SIDE / orig.width().min(orig.height()));
        let size = Vec2::new(orig.width() * s, orig.height() * s);
        r = if centre {
            Rect::from_center_size(orig.center(), size)
        } else {
            let fixed = Pos2::new(
                if dx > 0 { orig.min.x } else { orig.max.x },
                if dy > 0 { orig.min.y } else { orig.max.y },
            );
            let x = if dx > 0 { fixed.x } else { fixed.x - size.x };
            let y = if dy > 0 { fixed.y } else { fixed.y - size.y };
            Rect::from_min_size(Pos2::new(x, y), size)
        };
    }
    r
}

/// The box for something drawn by dragging from `start` to `end`.
///
/// - `square`: equal sides (the longer drag wins), going the way the pointer went.
/// - `centre`: `start` is the middle and the box grows both ways.
/// - `grid`: snap the corners (the middle when drawing from it).
pub fn drawn(start: Pos2, end: Pos2, grid: Option<f32>, square: bool, centre: bool) -> Rect {
    let snap_to = |p: Pos2| grid.map_or(p, |g| snap(p, g));
    let a = snap_to(start);
    let mut d = snap_to(end) - a;
    if square {
        let side = d.x.abs().max(d.y.abs());
        d = Vec2::new(side.copysign(d.x), side.copysign(d.y));
    }
    if centre {
        Rect::from_center_size(a, Vec2::new(d.x.abs() * 2.0, d.y.abs() * 2.0))
    } else {
        Rect::from_two_pos(a, a + d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(200.0, 100.0))
    }

    #[test]
    fn each_handle_is_on_its_edge_or_corner() {
        assert_eq!(Handle::NW.point(r()), Pos2::new(100.0, 100.0));
        assert_eq!(Handle::SE.point(r()), Pos2::new(300.0, 200.0));
        assert_eq!(Handle::N.point(r()), Pos2::new(200.0, 100.0));
        assert_eq!(Handle::W.point(r()), Pos2::new(100.0, 150.0));
        assert_eq!(Handle::ALL.iter().filter(|h| h.is_corner()).count(), 4);
    }

    #[test]
    fn a_press_finds_the_nearest_handle_within_reach() {
        let rect = r();
        assert_eq!(
            handle_at(rect, Pos2::new(103.0, 98.0), 6.0),
            Some(Handle::NW)
        );
        assert_eq!(
            handle_at(rect, Pos2::new(200.0, 103.0), 6.0),
            Some(Handle::N)
        );
        assert_eq!(
            handle_at(rect, Pos2::new(200.0, 150.0), 6.0),
            None,
            "the middle is not a handle"
        );
        assert_eq!(handle_at(rect, Pos2::new(150.0, 100.0), 6.0), None);
    }

    #[test]
    fn pulling_an_edge_moves_only_that_edge() {
        let out = resized(r(), Handle::E, Pos2::new(350.0, 999.0), None, false, false);
        assert_eq!(
            out,
            Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(350.0, 200.0))
        );
        let out = resized(r(), Handle::NW, Pos2::new(80.0, 60.0), None, false, false);
        assert_eq!(
            out,
            Rect::from_min_max(Pos2::new(80.0, 60.0), Pos2::new(300.0, 200.0))
        );
    }

    #[test]
    fn an_edge_stops_at_the_smallest_size_instead_of_crossing() {
        let out = resized(r(), Handle::E, Pos2::new(20.0, 150.0), None, false, false);
        assert_eq!(out.width(), MIN_SIDE);
        assert_eq!(out.min.x, 100.0);
        let out = resized(r(), Handle::W, Pos2::new(900.0, 150.0), None, false, false);
        assert_eq!(out.width(), MIN_SIDE);
        assert_eq!(out.max.x, 300.0);
    }

    #[test]
    fn the_centre_option_moves_both_sides_and_keeps_the_middle() {
        let out = resized(r(), Handle::E, Pos2::new(350.0, 150.0), None, false, true);
        assert_eq!(out.center(), r().center());
        assert_eq!(out.width(), 300.0);
        assert_eq!(out.height(), 100.0, "the other axis is untouched");
    }

    #[test]
    fn the_ratio_option_keeps_the_proportions_from_the_fixed_corner() {
        let out = resized(r(), Handle::SE, Pos2::new(500.0, 210.0), None, true, false);
        assert_eq!(
            out.min,
            Pos2::new(100.0, 100.0),
            "the opposite corner stays"
        );
        assert!((out.width() / out.height() - 2.0).abs() < 1e-4);
        assert_eq!(out.width(), 400.0, "the larger pull wins");
        let out = resized(r(), Handle::NW, Pos2::new(0.0, 90.0), None, true, false);
        assert_eq!(out.max, Pos2::new(300.0, 200.0));
        assert!((out.width() / out.height() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn ratio_and_centre_together_scale_about_the_middle() {
        let out = resized(r(), Handle::SE, Pos2::new(400.0, 250.0), None, true, true);
        assert_eq!(out.center(), r().center());
        assert!((out.width() / out.height() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn the_ratio_option_means_nothing_for_an_edge_handle() {
        let a = resized(r(), Handle::E, Pos2::new(350.0, 150.0), None, true, false);
        let b = resized(r(), Handle::E, Pos2::new(350.0, 150.0), None, false, false);
        assert_eq!(a, b);
    }

    #[test]
    fn the_grid_snaps_the_pulled_edge() {
        let out = resized(
            r(),
            Handle::E,
            Pos2::new(347.0, 150.0),
            Some(10.0),
            false,
            false,
        );
        assert_eq!(out.max.x, 350.0);
    }

    #[test]
    fn drawing_goes_any_direction_and_square_makes_equal_sides() {
        let a = Pos2::new(100.0, 100.0);
        let up_left = drawn(a, Pos2::new(40.0, 70.0), None, false, false);
        assert_eq!(up_left, Rect::from_min_max(Pos2::new(40.0, 70.0), a));
        let sq = drawn(a, Pos2::new(40.0, 70.0), None, true, false);
        assert_eq!(
            sq,
            Rect::from_min_max(Pos2::new(40.0, 40.0), a),
            "the longer drag wins"
        );
        let sq = drawn(a, Pos2::new(130.0, 190.0), None, true, false);
        assert_eq!((sq.width(), sq.height()), (90.0, 90.0));
        assert_eq!(sq.min, a);
    }

    #[test]
    fn drawing_from_the_centre_grows_both_ways() {
        let a = Pos2::new(100.0, 100.0);
        let c = drawn(a, Pos2::new(130.0, 120.0), None, false, true);
        assert_eq!(c.center(), a);
        assert_eq!((c.width(), c.height()), (60.0, 40.0));
        let both = drawn(a, Pos2::new(130.0, 120.0), None, true, true);
        assert_eq!(both.center(), a);
        assert_eq!((both.width(), both.height()), (60.0, 60.0));
    }

    #[test]
    fn drawing_snaps_to_the_grid() {
        let out = drawn(
            Pos2::new(13.0, 27.0),
            Pos2::new(98.0, 71.0),
            Some(10.0),
            false,
            false,
        );
        assert_eq!(
            out,
            Rect::from_min_max(Pos2::new(10.0, 30.0), Pos2::new(100.0, 70.0))
        );
    }
}
