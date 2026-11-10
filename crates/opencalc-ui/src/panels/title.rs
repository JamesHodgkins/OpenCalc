//! Title row: the app button with its File menu, the Quick Access Toolbar, and the
//! centred worksheet title.

use eframe::egui::{
    pos2, vec2, Align2, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use super::{Commands, PanelCx};
use crate::commands::CommandId;
use crate::ribbon::widgets::{self as w, Look};
use crate::ribbon::CommandSource;
use crate::theme::{focus_ring, metrics, type_size};

/// The text for the window's title bar and the centred title: modified sheets
/// are marked with a dot.
pub fn window_title(state: &crate::app::AppState) -> String {
    match state.active_sheet() {
        Some(sheet) => format!("{} \u{2013} OpenCalc", super::doc_tabs::label(sheet)),
        None => "OpenCalc".to_owned(),
    }
}

pub(crate) const QUICK_ACCESS: [CommandId; 5] = [
    CommandId::FileNew,
    CommandId::FileOpen,
    CommandId::FileSave,
    CommandId::EditUndo,
    CommandId::EditRedo,
];

pub fn show(ui: &mut Ui, cx: &PanelCx, out: &mut Commands) {
    let theme = cx.state.theme;
    let title = window_title(cx.state);
    // Centred worksheet title, painted first so controls sit on top.
    ui.painter().text(
        ui.max_rect().center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(type_size::TAB),
        theme.text_weak,
    );

    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
        let button = app_button(ui, cx);
        if button.clicked() {
            out.push(CommandId::AppMenu);
        }
        super::app_menu::show(ui.ctx(), cx, out, button.rect);

        for command in QUICK_ACCESS {
            let look = Look {
                enabled: cx.registry.is_enabled(command),
                active: false,
            };
            let tip = match cx.registry.shortcut_text(command) {
                Some(s) => format!("{} ({s})", command.label()),
                None => command.label().to_owned(),
            };
            let button = w::small_button(ui, &theme, look, Some(command.icon()), None, w::SMALL_H);
            if button.on_hover_text(tip).clicked() && look.enabled {
                out.push(command);
            }
        }
    });
}

/// The app button: the logo and "File". It stays pressed while the menu is open.
fn app_button(ui: &mut Ui, cx: &PanelCx) -> Response {
    let theme = &cx.state.theme;
    const LOGO: f32 = 18.0;
    const LABEL: &str = "File";
    let width = 7.0 + LOGO + 7.0 + w::text_width(ui, LABEL, type_size::TAB) + 10.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, metrics::CONTROL_HEIGHT), Sense::click());
    let open = cx.state.app_menu_open;
    let painter = ui.painter();
    let r = CornerRadius::same(metrics::RADIUS);
    if resp.is_pointer_button_down_on() || open {
        painter.rect_filled(rect, r, theme.pressed);
        painter.rect_stroke(
            rect,
            r,
            Stroke::new(1.0_f32, theme.border_strong),
            StrokeKind::Inside,
        );
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
    let logo = Rect::from_center_size(
        pos2(rect.left() + 7.0 + LOGO / 2.0, rect.center().y),
        Vec2::splat(LOGO),
    );
    crate::logo::image(ui.ctx(), LOGO).paint_at(ui, logo);
    ui.painter().text(
        pos2(logo.right() + 7.0, rect.center().y),
        Align2::LEFT_CENTER,
        LABEL,
        FontId::proportional(type_size::TAB),
        theme.text,
    );
    let tip = match cx.registry.shortcut_text(CommandId::AppMenu) {
        Some(keys) => format!("File menu ({keys})"),
        None => "File menu".to_owned(),
    };
    if open {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}
