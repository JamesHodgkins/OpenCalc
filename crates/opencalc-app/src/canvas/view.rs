//! View transform for the worksheet canvas (spec section 4.2).
//!
//! `screen = origin + doc * zoom + offset`. Everything stays in document space
//! until painting. Pure geometry, so it is tested without a window.

use eframe::egui::{Pos2, Rect, Vec2};

pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasView {
    pub zoom: f32,
    pub offset: Vec2,
}

impl Default for CanvasView {
    fn default() -> Self {
        CanvasView {
            zoom: 1.0,
            offset: Vec2::ZERO,
        }
    }
}

impl CanvasView {
    pub fn doc_to_screen(&self, p: Pos2, origin: Pos2) -> Pos2 {
        origin + p.to_vec2() * self.zoom + self.offset
    }

    pub fn screen_to_doc(&self, p: Pos2, origin: Pos2) -> Pos2 {
        ((p - origin - self.offset) / self.zoom).to_pos2()
    }

    pub fn rect_to_screen(&self, r: Rect, origin: Pos2) -> Rect {
        Rect::from_min_max(
            self.doc_to_screen(r.min, origin),
            self.doc_to_screen(r.max, origin),
        )
    }

    /// Multiply the zoom by `factor`, keeping the document point under
    /// `screen_pt` where it is.
    pub fn zoom_about(&mut self, screen_pt: Pos2, origin: Pos2, factor: f32) {
        let doc = self.screen_to_doc(screen_pt, origin);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = (screen_pt - origin) - doc.to_vec2() * self.zoom;
    }

    pub fn set_zoom_about(&mut self, screen_pt: Pos2, origin: Pos2, zoom: f32) {
        self.zoom_about(screen_pt, origin, zoom / self.zoom);
    }

    /// Choose zoom and offset so `bounds` (document space) fits in a viewport
    /// of the given size, with `margin` screen pixels around it.
    pub fn fit(&mut self, bounds: Rect, viewport: Vec2, margin: f32) {
        let avail = (viewport - Vec2::splat(2.0 * margin)).max(Vec2::splat(1.0));
        let size = bounds.size().max(Vec2::splat(1.0));
        self.zoom = (avail.x / size.x)
            .min(avail.y / size.y)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        // Centre the bounds in the viewport.
        let centre = bounds.center().to_vec2() * self.zoom;
        self.offset = viewport / 2.0 - centre;
    }
}

/// The snap grid, anchored to the top left of each page's margin box so that
/// snapped things line up with the margins whatever the step. Pages stack at a
/// fixed stride, so a point on page `k` snaps to that page's own lattice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapper {
    pub step: f32,
    /// The margin, in document units: the lattice starts this far in.
    pub margin: f32,
    /// Distance from the top of one page to the top of the next, or infinite for
    /// one lattice over the whole sheet (draft view).
    pub stride: f32,
}

impl Snapper {
    /// A lattice from the document origin, with no pages. For tests and for sheets
    /// with no page setup to hand.
    pub fn plain(step: f32) -> Snapper {
        Snapper {
            step,
            margin: 0.0,
            stride: f32::INFINITY,
        }
    }

    pub fn new(step: f32, setup: &opencalc_core::document::PageSetup, draft: bool) -> Snapper {
        Snapper {
            step,
            margin: setup.margin(),
            stride: if draft {
                f32::INFINITY
            } else {
                setup.size().1 + super::page::GAP
            },
        }
    }

    /// The lattice origin for a point at document height `y`: the top left of the
    /// margin box of the page that `y` falls on (the gap belongs to the page above).
    pub fn origin_at(&self, y: f32) -> Pos2 {
        let k = if self.stride.is_finite() {
            ((y + super::page::GAP / 2.0) / self.stride)
                .floor()
                .max(0.0)
        } else {
            0.0
        };
        let top = if k == 0.0 { 0.0 } else { k * self.stride };
        Pos2::new(self.margin, top + self.margin)
    }

    /// Round a document point to the nearest grid intersection.
    pub fn snap(&self, p: Pos2) -> Pos2 {
        let o = self.origin_at(p.y);
        Pos2::new(
            o.x + ((p.x - o.x) / self.step).round() * self.step,
            o.y + ((p.y - o.y) / self.step).round() * self.step,
        )
    }

    /// The grid point `n` lines from `p` in the direction `dir` (each of -1, 0, 1).
    /// A point off the grid first goes to the nearest line that way, so a nudge
    /// always ends on the grid. An axis with direction 0 is left alone.
    pub fn step_toward(&self, p: Pos2, dir: (i8, i8), n: u32) -> Pos2 {
        let o = self.origin_at(p.y);
        let go = |v: f32, o: f32, d: i8| {
            let k = (v - o) / self.step;
            let line = match d {
                1 => (k + 1e-3).floor() + n as f32,
                -1 => (k - 1e-3).ceil() - n as f32,
                _ => return v,
            };
            o + line * self.step
        };
        Pos2::new(go(p.x, o.x, dir.0), go(p.y, o.y, dir.1))
    }

    /// Round a point right and down onto the grid; one already on it (within
    /// rounding error) stays where it is.
    pub fn ceil(&self, p: Pos2) -> Pos2 {
        let o = self.origin_at(p.y);
        let up = |v: f32, o: f32| o + (((v - o) / self.step) - 1e-3).ceil() * self.step;
        Pos2::new(up(p.x, o.x), up(p.y, o.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Pos2, b: Pos2) -> bool {
        (a - b).length() < 1e-3
    }

    #[test]
    fn doc_and_screen_round_trip() {
        let v = CanvasView {
            zoom: 1.7,
            offset: Vec2::new(-30.0, 12.5),
        };
        let origin = Pos2::new(200.0, 80.0);
        for p in [Pos2::ZERO, Pos2::new(10.0, 20.0), Pos2::new(-400.0, 999.0)] {
            assert!(close(
                v.screen_to_doc(v.doc_to_screen(p, origin), origin),
                p
            ));
        }
    }

    #[test]
    fn transform_matches_the_spec_formula() {
        let v = CanvasView {
            zoom: 2.0,
            offset: Vec2::new(5.0, 7.0),
        };
        let s = v.doc_to_screen(Pos2::new(10.0, 20.0), Pos2::new(100.0, 100.0));
        assert!(close(s, Pos2::new(125.0, 147.0)));
    }

    #[test]
    fn zoom_about_keeps_the_point_under_the_cursor_fixed() {
        let mut v = CanvasView::default();
        let origin = Pos2::new(50.0, 40.0);
        let cursor = Pos2::new(300.0, 250.0);
        let before = v.screen_to_doc(cursor, origin);
        v.zoom_about(cursor, origin, 1.5);
        assert!((v.zoom - 1.5).abs() < 1e-6);
        assert!(close(v.screen_to_doc(cursor, origin), before));
        v.zoom_about(cursor, origin, 0.4);
        assert!(close(v.screen_to_doc(cursor, origin), before));
    }

    #[test]
    fn zoom_is_clamped_and_still_anchored() {
        let mut v = CanvasView::default();
        let origin = Pos2::ZERO;
        let cursor = Pos2::new(120.0, 90.0);
        let before = v.screen_to_doc(cursor, origin);
        v.zoom_about(cursor, origin, 1000.0);
        assert_eq!(v.zoom, MAX_ZOOM);
        assert!(close(v.screen_to_doc(cursor, origin), before));
        v.zoom_about(cursor, origin, 1e-6);
        assert_eq!(v.zoom, MIN_ZOOM);
        assert!(close(v.screen_to_doc(cursor, origin), before));
    }

    #[test]
    fn set_zoom_about_reaches_the_requested_zoom() {
        let mut v = CanvasView::default();
        v.zoom_about(Pos2::new(10.0, 10.0), Pos2::ZERO, 2.5);
        v.set_zoom_about(Pos2::new(10.0, 10.0), Pos2::ZERO, 1.0);
        assert!((v.zoom - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fit_puts_the_bounds_inside_the_viewport() {
        let mut v = CanvasView::default();
        let bounds = Rect::from_min_max(Pos2::new(100.0, 50.0), Pos2::new(900.0, 450.0));
        let viewport = Vec2::new(640.0, 480.0);
        v.fit(bounds, viewport, 20.0);
        let s = v.rect_to_screen(bounds, Pos2::ZERO);
        assert!(s.min.x >= 19.9 && s.min.y >= 19.9);
        assert!(s.max.x <= viewport.x - 19.9 && s.max.y <= viewport.y - 19.9);
        // It is the limiting axis that touches the margin.
        assert!(
            (s.min.x - 20.0).abs() < 0.1 || (s.min.y - 20.0).abs() < 0.1,
            "fit is not as large as it could be"
        );
    }

    #[test]
    fn snap_rounds_to_the_nearest_grid_point() {
        let g = Snapper::plain(20.0);
        assert_eq!(g.snap(Pos2::new(29.0, 31.0)), Pos2::new(20.0, 40.0));
        assert_eq!(g.snap(Pos2::new(-9.0, -11.0)), Pos2::new(0.0, -20.0));
        assert_eq!(g.snap(Pos2::new(40.0, 60.0)), Pos2::new(40.0, 60.0));
    }

    /// The point of the anchor: a snapped point sits a whole number of steps from
    /// its page's margin corner, for every step and margin, on every page.
    #[test]
    fn snapping_is_anchored_to_each_pages_margin_box() {
        use opencalc_core::document::PageSetup;
        let setup = PageSetup::default();
        for step in [6.0_f32, 12.0, 24.0, 7.559, 18.897] {
            let g = Snapper::new(step, &setup, false);
            for page in 0..3 {
                let top = page as f32 * g.stride;
                for p in [(100.3, 50.7), (333.0, 400.2), (700.9, 900.5)] {
                    let p = Pos2::new(p.0, top + p.1);
                    let s = g.snap(p);
                    let o = g.origin_at(s.y);
                    let (kx, ky) = ((s.x - o.x) / step, (s.y - o.y) / step);
                    assert!((kx - kx.round()).abs() < 1e-2, "x {step} page {page}");
                    assert!((ky - ky.round()).abs() < 1e-2, "y {step} page {page}");
                }
                // The margin corner itself is a grid point.
                let corner = g.origin_at(top + 10.0);
                assert!((g.snap(corner) - corner).length() < 1e-3);
            }
        }
    }

    #[test]
    fn draft_view_has_one_lattice_for_the_whole_sheet() {
        use opencalc_core::document::PageSetup;
        let g = Snapper::new(12.0, &PageSetup::default(), true);
        assert_eq!(g.origin_at(5000.0), g.origin_at(10.0));
    }

    #[test]
    fn ceil_rounds_right_and_down_but_keeps_a_point_on_the_grid() {
        let g = Snapper::plain(20.0);
        assert_eq!(g.ceil(Pos2::new(21.0, 39.0)), Pos2::new(40.0, 40.0));
        assert_eq!(g.ceil(Pos2::new(40.0, 60.0)), Pos2::new(40.0, 60.0));
    }
}
