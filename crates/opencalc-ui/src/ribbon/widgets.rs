//! Custom-painted ribbon controls (spec sections 4.8 and 4.9).

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

use crate::icons;
use crate::theme::{focus_ring, metrics, type_size, Theme};

pub const SMALL_H: f32 = 22.0;
pub const LARGE_H: f32 = 66.0;
pub const LARGE_MIN_W: f32 = 48.0;
pub const ICON_SMALL: f32 = 14.0;
pub const ICON_LARGE: f32 = 32.0;
pub const ARROW_W: f32 = 14.0;
const LABEL_PITCH: f32 = 13.5;

#[derive(Clone, Copy)]
pub struct Look {
    pub enabled: bool,
    /// Toggled on.
    pub active: bool,
}

impl Look {
    pub fn sense(self) -> Sense {
        if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        }
    }

    fn fg(self, theme: &Theme) -> Color32 {
        if self.enabled {
            theme.text
        } else {
            theme.text_disabled
        }
    }
}

pub fn text_width(ui: &Ui, s: &str, size: f32) -> f32 {
    ui.fonts(|f| {
        f.layout_no_wrap(s.to_owned(), FontId::proportional(size), Color32::WHITE)
            .rect
            .width()
    })
}

/// Split a label over at most two lines, at the space nearest the middle.
pub fn two_lines(label: &str) -> (String, Option<String>) {
    let mid = label.len() / 2;
    let split = label
        .match_indices(' ')
        .map(|(i, _)| i)
        .min_by_key(|i| i.abs_diff(mid));
    match split {
        Some(i) => (label[..i].to_owned(), Some(label[i + 1..].to_owned())),
        None => (label.to_owned(), None),
    }
}

/// Background for a control in its current state.
pub fn paint_state(ui: &Ui, rect: Rect, resp: &Response, theme: &Theme, look: Look) {
    let r = CornerRadius::same(metrics::RADIUS);
    let painter = ui.painter();
    if !look.enabled {
        return;
    }
    if resp.is_pointer_button_down_on() {
        painter.rect_filled(rect, r, theme.pressed);
        painter.rect_stroke(
            rect,
            r,
            Stroke::new(1.0_f32, theme.border_strong),
            StrokeKind::Inside,
        );
    } else if look.active {
        painter.rect_filled(rect, r, theme.pressed);
    } else if resp.hovered() {
        painter.rect_filled(rect, r, theme.raised_high);
        painter.rect_stroke(
            rect,
            r,
            Stroke::new(1.0_f32, theme.border),
            StrokeKind::Inside,
        );
    }
    if resp.has_focus() {
        focus_ring(painter, rect, theme);
    }
}

/// Width of a small button.
pub fn small_width(ui: &Ui, icon: bool, label: Option<&str>) -> f32 {
    match (icon, label) {
        (true, None) => SMALL_H,
        (true, Some(l)) => {
            4.0 + ICON_SMALL + 5.0 + text_width(ui, l, type_size::RIBBON_LABEL) + 6.0
        }
        (false, Some(l)) => 6.0 + text_width(ui, l, type_size::RIBBON_LABEL) + 6.0,
        (false, None) => 0.0,
    }
}

/// Paint a small icon and label inside `rect`.
pub fn paint_small(
    ui: &Ui,
    rect: Rect,
    theme: &Theme,
    look: Look,
    icon: Option<char>,
    label: Option<&str>,
) {
    let colour = look.fg(theme);
    let painter = ui.painter();
    let cy = rect.center().y;
    match (icon, label) {
        (Some(ic), None) => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                ic,
                FontId::proportional(ICON_SMALL),
                colour,
            );
        }
        (Some(ic), Some(l)) => {
            let ix = rect.min.x + 4.0 + ICON_SMALL / 2.0;
            painter.text(
                egui::pos2(ix, cy),
                Align2::CENTER_CENTER,
                ic,
                FontId::proportional(ICON_SMALL),
                colour,
            );
            let lx = rect.min.x + 4.0 + ICON_SMALL + 5.0;
            painter.text(
                egui::pos2(lx, cy),
                Align2::LEFT_CENTER,
                l,
                FontId::proportional(type_size::RIBBON_LABEL),
                colour,
            );
        }
        (None, Some(l)) => {
            painter.text(
                egui::pos2(rect.min.x + 6.0, cy),
                Align2::LEFT_CENTER,
                l,
                FontId::proportional(type_size::RIBBON_LABEL),
                colour,
            );
        }
        (None, None) => {}
    }
}

/// A small button: icon, label, or both.
pub fn small_button(
    ui: &mut Ui,
    theme: &Theme,
    look: Look,
    icon: Option<char>,
    label: Option<&str>,
    width: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, SMALL_H), look.sense());
    paint_state(ui, rect, &resp, theme, look);
    paint_small(ui, rect, theme, look, icon, label);
    resp
}

pub fn large_width(ui: &Ui, label: &str) -> f32 {
    let (a, b) = two_lines(label);
    let w = text_width(ui, &a, type_size::RIBBON_LABEL)
        .max(b.map_or(0.0, |b| text_width(ui, &b, type_size::RIBBON_LABEL)));
    LARGE_MIN_W.max(w + 12.0)
}

/// Paint a large icon over a two-line label. `arrow` adds a chevron after the label.
pub fn paint_large(
    ui: &Ui,
    rect: Rect,
    theme: &Theme,
    look: Look,
    icon: char,
    label: &str,
    arrow: bool,
) {
    let colour = look.fg(theme);
    let painter = ui.painter();
    let icon_c = egui::pos2(rect.center().x, rect.min.y + 4.0 + ICON_LARGE / 2.0);
    painter.text(
        icon_c,
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(ICON_LARGE * 0.8),
        colour,
    );
    let (a, b) = two_lines(label);
    let y0 = rect.min.y + 4.0 + ICON_LARGE + 3.0;
    let font = FontId::proportional(type_size::RIBBON_LABEL);
    let first = if b.is_none() && arrow {
        format!("{a} {}", icons::CHEVRON_DOWN)
    } else {
        a
    };
    painter.text(
        egui::pos2(rect.center().x, y0 + LABEL_PITCH / 2.0),
        Align2::CENTER_CENTER,
        first,
        font.clone(),
        colour,
    );
    if let Some(b) = b {
        let second = if arrow {
            format!("{b} {}", icons::CHEVRON_DOWN)
        } else {
            b
        };
        painter.text(
            egui::pos2(rect.center().x, y0 + LABEL_PITCH * 1.5),
            Align2::CENTER_CENTER,
            second,
            font,
            colour,
        );
    }
}

/// A large button.
pub fn large_button(
    ui: &mut Ui,
    theme: &Theme,
    look: Look,
    icon: char,
    label: &str,
    width: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, LARGE_H), look.sense());
    paint_state(ui, rect, &resp, theme, look);
    paint_large(ui, rect, theme, look, icon, label, false);
    resp
}

pub fn paint_arrow(ui: &Ui, rect: Rect, theme: &Theme, look: Look) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icons::CHEVRON_DOWN,
        FontId::proportional(10.0),
        look.fg(theme),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_lines_splits_near_the_middle() {
        assert_eq!(two_lines("Paste"), ("Paste".to_owned(), None));
        assert_eq!(
            two_lines("Math region"),
            ("Math".to_owned(), Some("region".to_owned()))
        );
        let (a, b) = two_lines("Define custom unit");
        assert_eq!(a, "Define");
        assert_eq!(b.as_deref(), Some("custom unit"));
    }
}
