//! The Home tab, one tab per worksheet with a close button, and the new-worksheet
//! button.

use eframe::egui::{self, Align2, CornerRadius, FontId, Id, Rect, Sense, Ui, Vec2};

use super::{Commands, PanelCx};
use crate::commands::CommandId;
use crate::icons;
use crate::ribbon::widgets as w;
use crate::ribbon::CommandSource;
use crate::theme::{metrics, type_size, Theme};

const HOME_WIDTH: f32 = 74.0;
const PLUS_WIDTH: f32 = 28.0;

pub fn show(ui: &mut Ui, cx: &PanelCx, out: &mut Commands) {
    let theme = cx.state.theme;
    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

    let home = tab(
        ui,
        &theme,
        &format!("{} Home", icons::HOME),
        HOME_WIDTH,
        cx.state.active.is_none(),
    );
    if home.1.clicked() {
        out.push(CommandId::TabHome);
    }
    for (i, sheet) in cx.state.sheets.iter().enumerate() {
        let label = sheet.title();
        let width = w::text_width(ui, label, type_size::TAB) + 48.0;
        let (rect, resp) = tab(ui, &theme, label, width, cx.state.active == Some(i));
        let x = Rect::from_center_size(
            egui::pos2(rect.max.x - 14.0, rect.center().y),
            Vec2::splat(16.0),
        );
        let x_resp = ui.interact(x, Id::new(("close-tab", i)), Sense::click());
        if x_resp.hovered() {
            ui.painter()
                .rect_filled(x, CornerRadius::same(metrics::RADIUS), theme.raised_high);
        }
        ui.painter().text(
            x.center(),
            Align2::CENTER_CENTER,
            icons::CLOSE,
            FontId::proportional(10.0),
            theme.text_weak,
        );
        let tip = shortcut_tip(cx, CommandId::FileClose);
        if x_resp.on_hover_text(tip).clicked() {
            out.push(CommandId::CloseSheet(i));
        } else if resp.clicked() {
            out.push(CommandId::SelectSheet(i));
        }
    }
    let (_, plus) = tab(ui, &theme, "+", PLUS_WIDTH, false);
    if plus
        .on_hover_text(shortcut_tip(cx, CommandId::FileNew))
        .clicked()
    {
        out.push(CommandId::FileNew);
    }
}

fn shortcut_tip(cx: &PanelCx, command: CommandId) -> String {
    match cx.registry.shortcut_text(command) {
        Some(s) => format!("{} ({s})", command.label()),
        None => command.label().to_owned(),
    }
}

/// One tab. The active tab takes the canvas tone; the others are raised on hover.
fn tab(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    width: f32,
    active: bool,
) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, metrics::DOC_TAB), Sense::click());
    let fill = if active {
        theme.base
    } else if resp.hovered() {
        theme.raised
    } else {
        theme.surface
    };
    let radius = CornerRadius {
        nw: 4,
        ne: 4,
        sw: 0,
        se: 0,
    };
    ui.painter().rect_filled(rect, radius, fill);
    let colour = if active { theme.text } else { theme.text_weak };
    ui.painter().text(
        egui::pos2(rect.min.x + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(type_size::TAB),
        colour,
    );
    (rect, resp)
}
