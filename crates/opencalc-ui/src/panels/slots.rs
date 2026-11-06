//! Stand-ins for controls the ribbon leaves room for (`Cell::Slot`).

use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, Ui};

use crate::theme::{metrics, type_size, Theme};

/// A sunken, disabled-looking well where the font-size control will go (TD-18).
pub fn font_size(ui: &Ui, theme: &Theme, rect: Rect) {
    let r = CornerRadius::same(metrics::RADIUS);
    ui.painter().rect_filled(rect, r, theme.base);
    ui.painter().rect_stroke(
        rect,
        r,
        egui::Stroke::new(1.0_f32, theme.border),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::pos2(rect.min.x + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        "12 pt",
        FontId::proportional(type_size::RIBBON_LABEL),
        theme.text_disabled,
    );
}
