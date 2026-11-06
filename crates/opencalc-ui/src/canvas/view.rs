//! View transform for the worksheet canvas (spec section 4.2).
//!
//! `screen = origin + doc * zoom + offset`. Everything stays in document space
//! until painting. Pure geometry, so it is tested without a window.

use eframe::egui::{Pos2, Rect, Vec2};

pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 4.0;
/// Snap grid spacing in document units: an eighth of an inch, so every eighth
/// line is a whole inch.
pub const GRID: f32 = 12.0;

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

/// Round a document point to the nearest grid intersection.
pub fn snap(p: Pos2, grid: f32) -> Pos2 {
    Pos2::new((p.x / grid).round() * grid, (p.y / grid).round() * grid)
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
        assert_eq!(snap(Pos2::new(29.0, 31.0), 20.0), Pos2::new(20.0, 40.0));
        assert_eq!(snap(Pos2::new(-9.0, -11.0), 20.0), Pos2::new(0.0, -20.0));
        assert_eq!(snap(Pos2::new(40.0, 60.0), 20.0), Pos2::new(40.0, 60.0));
    }
}
