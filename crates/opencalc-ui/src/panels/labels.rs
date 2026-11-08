//! The Labels window: the labelled regions, one line each. Clicking one goes to it.
//! It reads the worksheet and asks for changes by command (rule R4).

use eframe::egui::{self, Align2, Context, RichText};

use super::Commands;
use crate::app::labels::labelled;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::icons;

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.labels_open {
        return;
    }
    let Some(sheet) = state.active_sheet() else {
        out.push(CommandId::LabelsClose);
        return;
    };
    let list = labelled(&sheet.doc);
    let weak = state.theme.text_weak;
    let mut open = true;
    egui::Window::new("Labels")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(360.0)
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -40.0])
        .show(ctx, |ui| {
            if list.is_empty() {
                ui.label(
                    RichText::new("No regions are labelled. Select one and press Tag.").color(weak),
                );
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for (i, item) in list.iter().enumerate().take(500) {
                        let text = format!("{}  {}\n{}", icons::TAG, item.label, item.about);
                        if ui.add(egui::Button::new(text).frame(false)).clicked() {
                            out.push(CommandId::GoToLabel(i as u16));
                        }
                    }
                });
        });
    if !open {
        out.push(CommandId::LabelsClose);
    }
}
