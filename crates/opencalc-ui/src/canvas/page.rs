//! Page geometry and painting: white pages stacked on the desk.
//!
//! Pages are a background and a measure. Regions keep their document coordinates;
//! the page setup only decides where page edges fall. Page `k` (from 0) has its top
//! at `k * (height + GAP)`, so there is a gap between pages that regions may cross.
//! Pure geometry is separate from painting so it is tested without a window.

use eframe::egui::{epaint::Shadow, Color32, Painter, Pos2, Rect, Stroke, Vec2};
use opencalc_core::document::{PageSetup, Region};

use super::adapt::{rect_of, Sizes};
use super::view::{CanvasView, MAX_ZOOM, MIN_ZOOM};
use crate::theme::Theme;

/// Space between two pages, in document units.
pub const GAP: f32 = 24.0;
/// Screen pixels kept free around a fitted page.
const FIT_MARGIN: f32 = 24.0;

/// The rectangle of page `k` in document space.
pub fn page_rect(setup: &PageSetup, k: usize) -> Rect {
    let (w, h) = setup.size();
    let top = k as f32 * (h + GAP);
    Rect::from_min_size(Pos2::new(0.0, top), Vec2::new(w, h))
}

/// How many pages are needed for content reaching down to `bottom` (document y).
/// Always at least one. Content that ends in the gap below page `k` spills onto the
/// next page, so that page exists.
pub fn page_count(setup: &PageSetup, bottom: f32) -> usize {
    let (_, h) = setup.size();
    if !(bottom.is_finite() && bottom > h) {
        return 1;
    }
    let stride = h + GAP;
    let k = (bottom / stride).floor();
    let into = bottom - k * stride;
    // Pages are at most a few hundred long; clamp rather than trust the input.
    let k = k.min(10_000.0) as usize;
    if into <= h {
        k + 1
    } else {
        k + 2
    }
}

/// Which page (from 0) a document y falls on or is nearest to, given `count` pages.
pub fn page_at(setup: &PageSetup, y: f32, count: usize) -> usize {
    let (_, h) = setup.size();
    let stride = h + GAP;
    if !y.is_finite() || y <= 0.0 {
        return 0;
    }
    // A y in the gap below a page belongs to that page until the halfway point.
    ((y + GAP / 2.0) / stride).floor().min((count - 1) as f32) as usize
}

/// The lowest edge of any region, or 0 when there are none.
pub fn content_bottom(regions: &[Region], sizes: &Sizes) -> f32 {
    regions
        .iter()
        .map(|r| rect_of(r, sizes).max.y)
        .fold(0.0_f32, f32::max)
}

/// A view that shows the page's full width, centred, at no more than 100%.
pub fn fit_width(setup: &PageSetup, viewport: Vec2) -> CanvasView {
    let (w, _) = setup.size();
    let zoom = ((viewport.x - 2.0 * FIT_MARGIN) / w).clamp(MIN_ZOOM, MAX_ZOOM.min(1.0));
    CanvasView {
        zoom,
        offset: Vec2::new((viewport.x - w * zoom) / 2.0, FIT_MARGIN),
    }
}

/// Short lines in the four corners of the margin box, like the crop marks of a
/// print layout. Each is a pair of end points, in document space.
pub fn margin_marks(page: Rect, margin: f32) -> Vec<(Pos2, Pos2)> {
    let inner = page.shrink(margin);
    let tick = (margin * 0.5).clamp(4.0, 16.0);
    let mut marks = Vec::with_capacity(8);
    for (corner, dx, dy) in [
        (inner.left_top(), 1.0, 1.0),
        (inner.right_top(), -1.0, 1.0),
        (inner.left_bottom(), 1.0, -1.0),
        (inner.right_bottom(), -1.0, -1.0),
    ] {
        marks.push((corner, corner + Vec2::new(dx * tick, 0.0)));
        marks.push((corner, corner + Vec2::new(0.0, dy * tick)));
    }
    marks
}

/// Draw the pages that are visible in `viewport`. Returns the screen rectangle of
/// each drawn page, for clipping what is painted on them.
pub fn paint(
    painter: &Painter,
    viewport: Rect,
    view: &CanvasView,
    origin: Pos2,
    setup: &PageSetup,
    count: usize,
    paper: &Theme,
) -> Vec<Rect> {
    let mut drawn = Vec::new();
    for k in 0..count {
        let screen = view.rect_to_screen(page_rect(setup, k), origin);
        if !screen.intersects(viewport) {
            continue;
        }
        let shadow = Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(90),
        };
        painter.add(shadow.as_shape(screen, 0.0));
        painter.rect_filled(screen, 0.0, paper.base);
        let stroke = Stroke::new(1.0_f32, paper.raised);
        for (a, b) in margin_marks(page_rect(setup, k), setup.margin()) {
            painter.line_segment(
                [view.doc_to_screen(a, origin), view.doc_to_screen(b, origin)],
                stroke,
            );
        }
        drawn.push(screen);
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::PaperSize;

    fn a4() -> PageSetup {
        PageSetup::default()
    }

    #[test]
    fn page_rectangles_stack_with_a_gap() {
        let (w, h) = a4().size();
        let (p0, p1) = (page_rect(&a4(), 0), page_rect(&a4(), 1));
        assert_eq!(p0.min, Pos2::ZERO);
        assert_eq!(p0.size(), Vec2::new(w, h));
        assert_eq!(p1.min.y, h + GAP);
        assert_eq!(p1.min.x, 0.0);
    }

    #[test]
    fn pages_are_added_only_when_content_needs_them() {
        let setup = a4();
        let (_, h) = setup.size();
        assert_eq!(page_count(&setup, 0.0), 1);
        assert_eq!(page_count(&setup, -50.0), 1);
        assert_eq!(
            page_count(&setup, h),
            1,
            "exactly the page height still fits"
        );
        assert_eq!(
            page_count(&setup, h + 1.0),
            2,
            "spills into the gap, so page 2 exists"
        );
        assert_eq!(page_count(&setup, h + GAP + 10.0), 2);
        assert_eq!(page_count(&setup, 2.0 * h + GAP), 2);
        assert_eq!(page_count(&setup, 2.0 * h + GAP + 5.0), 3);
        assert_eq!(page_count(&setup, f32::NAN), 1);
        assert_eq!(page_count(&setup, f32::INFINITY), 1);
        // An absurd extent is clamped, not allocated.
        assert!(page_count(&setup, 1e30) <= 10_002);
    }

    #[test]
    fn the_page_in_view_follows_the_y_position() {
        let setup = a4();
        let (_, h) = setup.size();
        assert_eq!(page_at(&setup, 0.0, 3), 0);
        assert_eq!(page_at(&setup, h * 0.5, 3), 0);
        assert_eq!(page_at(&setup, h + GAP + 100.0, 3), 1);
        assert_eq!(page_at(&setup, 1e9, 3), 2, "never past the last page");
        assert_eq!(page_at(&setup, -40.0, 3), 0);
        assert_eq!(page_at(&setup, f32::NAN, 3), 0);
    }

    #[test]
    fn landscape_and_letter_change_the_geometry() {
        let landscape = PageSetup {
            landscape: true,
            ..a4()
        };
        assert!(page_rect(&landscape, 0).width() > page_rect(&a4(), 0).width());
        let letter = PageSetup {
            paper: PaperSize::Letter,
            ..a4()
        };
        assert!(page_rect(&letter, 0).height() < page_rect(&a4(), 0).height());
    }

    #[test]
    fn fitting_shows_the_whole_width_centred_and_never_enlarges() {
        let setup = a4();
        let (w, _) = setup.size();
        let narrow = fit_width(&setup, Vec2::new(500.0, 700.0));
        assert!(narrow.zoom < 1.0);
        let left = narrow.offset.x;
        let right = 500.0 - (left + w * narrow.zoom);
        assert!((left - right).abs() < 0.01 && left >= 0.0, "{left} {right}");
        let wide = fit_width(&setup, Vec2::new(3000.0, 700.0));
        assert_eq!(wide.zoom, 1.0);
        assert!(wide.offset.x > 0.0);
        // A tiny viewport still gives a usable zoom.
        assert!(fit_width(&setup, Vec2::new(10.0, 10.0)).zoom >= MIN_ZOOM);
    }

    #[test]
    fn margin_marks_sit_on_the_margin_box() {
        let page = page_rect(&a4(), 0);
        let margin = 75.0;
        let marks = margin_marks(page, margin);
        assert_eq!(marks.len(), 8);
        let inner = page.shrink(margin);
        for (corner, other) in marks {
            assert!(inner.expand(0.01).contains(corner), "{corner:?}");
            assert!(page.contains(other));
        }
    }
}
