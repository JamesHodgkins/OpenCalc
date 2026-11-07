//! Title row: the app button with its menu, the Quick Access Toolbar, and the
//! centred worksheet title.

use eframe::egui::{
    self, popup::popup_below_widget, Align2, CornerRadius, FontId, Id, PopupCloseBehavior, Sense,
    Ui, Vec2,
};

use super::{Commands, PanelCx};
use crate::commands::CommandId;
use crate::ribbon::widgets::{self as w, Look};
use crate::ribbon::CommandSource;
use crate::theme::{metrics, type_size};

/// The text for the window's title bar and the centred title: modified sheets
/// are marked with a dot.
pub fn window_title(state: &crate::app::AppState) -> String {
    match state.active_sheet() {
        Some(sheet) => format!("{} \u{2013} OpenCalc", super::doc_tabs::label(sheet)),
        None => "OpenCalc".to_owned(),
    }
}

pub(crate) const MENU: [CommandId; 9] = [
    CommandId::FileNew,
    CommandId::FileOpen,
    CommandId::FileSave,
    CommandId::FileSaveAs,
    CommandId::FileExportPdf,
    CommandId::FileExportPng,
    CommandId::AppOptions,
    CommandId::HelpAbout,
    CommandId::AppQuit,
];
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
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::splat(metrics::CONTROL_HEIGHT), Sense::click());
        let fill = if resp.is_pointer_button_down_on() {
            theme.accent_pressed
        } else if resp.hovered() {
            theme.accent_hover
        } else {
            theme.accent
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::RADIUS), fill);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "O",
            FontId::proportional(15.0),
            theme.text_on_accent,
        );
        let menu = Id::new("app-menu");
        if resp.on_hover_text("Application menu").clicked() {
            ui.memory_mut(|m| m.toggle_popup(menu));
        }
        let anchor = ui.interact(rect, Id::new("app-menu-anchor"), Sense::hover());
        popup_below_widget(ui, menu, &anchor, PopupCloseBehavior::CloseOnClick, |ui| {
            ui.set_min_width(160.0);
            for command in MENU {
                let enabled = cx.registry.is_enabled(command);
                let label = match cx.registry.shortcut_text(command) {
                    Some(keys) => format!("{}    {keys}", command.label()),
                    None => command.label().to_owned(),
                };
                let item = egui::Button::new(label)
                    .frame(false)
                    .min_size(Vec2::new(150.0, 22.0));
                if ui.add_enabled(enabled, item).clicked() {
                    out.push(command);
                }
            }
        });

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
