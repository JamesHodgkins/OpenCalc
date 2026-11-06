//! The Home page, shown when no worksheet is open.

use eframe::egui::{RichText, Ui};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;

pub fn show(ui: &mut Ui, state: &AppState, out: &mut Commands) {
    let theme = state.theme;
    ui.add_space(24.0);
    ui.label(RichText::new("OpenCalc").size(26.0).color(theme.text));
    ui.label(
        RichText::new("Engineering worksheets with units and live recalculation")
            .color(theme.text_weak),
    );
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        for command in [CommandId::FileNew, CommandId::FileExample] {
            if ui.button(command.label()).clicked() {
                out.push(command);
            }
        }
    });
}
