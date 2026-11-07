//! The Home page, shown when no worksheet is open.

use eframe::egui::{self, RichText, Ui};

use super::Commands;
use crate::app::AppState;
use crate::commands::CommandId;

pub fn show(ui: &mut Ui, state: &AppState, out: &mut Commands) {
    // The page is drawn edge to edge, so give its content a margin of its own.
    egui::Frame::new()
        .inner_margin(32)
        .show(ui, |ui| content(ui, state, out));
}

fn content(ui: &mut Ui, state: &AppState, out: &mut Commands) {
    let theme = state.theme;
    ui.horizontal(|ui| {
        ui.add(crate::logo::image(ui.ctx(), 40.0));
        ui.label(RichText::new("OpenCalc").size(26.0).color(theme.text));
    });
    ui.label(
        RichText::new("Engineering worksheets with units and live recalculation")
            .color(theme.text_weak),
    );
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        for command in [
            CommandId::FileNew,
            CommandId::FileOpen,
            CommandId::FileExample,
        ] {
            if ui.button(command.label()).clicked() {
                out.push(command);
            }
        }
    });
    if !state.recent.is_empty() {
        ui.add_space(20.0);
        ui.label(RichText::new("Recent").size(16.0).color(theme.text));
        for (n, path) in state.recent.iter().enumerate().take(u8::MAX as usize) {
            let name = path.file_name().map_or_else(
                || path.to_string_lossy().into_owned(),
                |f| f.to_string_lossy().into_owned(),
            );
            let folder = path.parent().map(|p| p.display().to_string());
            let response = ui.link(name).on_hover_text(path.display().to_string());
            if let Some(folder) = folder {
                ui.label(RichText::new(folder).small().color(theme.text_weak));
            }
            if response.clicked() {
                out.push(CommandId::OpenRecent(n as u8));
            }
        }
    }
}
