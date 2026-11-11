//! The snap grid drawn on the page.

use eframe::egui::{Painter, Pos2, Rect, Stroke};

use super::view::CanvasView;
use crate::settings::GridSpacing;
use crate::theme::Theme;

/// Minor lines at the snap grid, a stronger line every inch (or centimetre, for a
/// millimetre grid). When zoomed out far enough that lines would crowd, the
/// spacing steps up by five.
pub fn grid(
    painter: &Painter,
    rect: Rect,
    view: &CanvasView,
    origin: Pos2,
    theme: &Theme,
    spacing: GridSpacing,
) {
    let mut step = spacing.step() * view.zoom;
    let mut major_every = spacing.major_every();
    while step < 6.0 {
        step *= 5.0;
        major_every *= 5;
    }
    let base = origin + view.offset;
    let minor = Stroke::new(1.0_f32, theme.raised);
    // A little fainter than the border colour: halfway to the minor lines.
    let major = Stroke::new(1.0_f32, theme.raised.lerp_to_gamma(theme.border, 0.5));

    let first = ((rect.min.x - base.x) / step).floor() as i64;
    let last = ((rect.max.x - base.x) / step).ceil() as i64;
    for k in first..=last {
        let stroke = if k.rem_euclid(major_every) == 0 {
            major
        } else {
            minor
        };
        painter.vline(base.x + k as f32 * step, rect.y_range(), stroke);
    }
    let first = ((rect.min.y - base.y) / step).floor() as i64;
    let last = ((rect.max.y - base.y) / step).ceil() as i64;
    for k in first..=last {
        let stroke = if k.rem_euclid(major_every) == 0 {
            major
        } else {
            minor
        };
        painter.hline(rect.x_range(), base.y + k as f32 * step, stroke);
    }
}
