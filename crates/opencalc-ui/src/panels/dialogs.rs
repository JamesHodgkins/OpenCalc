//! The modal question or message in front of everything: "save changes?" and
//! error reports. Buttons push commands; nothing here changes state (rule R4).

use eframe::egui::{self, Context, Id, Modal, RichText};

use super::Commands;
use crate::app::filing::{Dialog, Then};
use crate::app::AppState;
use crate::commands::CommandId;

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    let Some(dialog) = &state.dialog else { return };
    let response = Modal::new(Id::new("dialog")).show(ctx, |ui| {
        ui.set_width(380.0);
        match dialog {
            Dialog::Unsaved { sheet, then } => {
                let name = state.sheets.get(*sheet).map_or("", |s| s.title());
                ui.label(
                    RichText::new(format!("Save changes to \u{201c}{name}\u{201d}?")).strong(),
                );
                let consequence = match then {
                    Then::Close(_) => "Your changes will be lost if you close without saving.",
                    Then::Quit => "Your changes will be lost if you quit without saving.",
                };
                ui.label(consequence);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for (command, default) in [
                        (CommandId::DialogSave, true),
                        (CommandId::DialogDiscard, false),
                        (CommandId::DialogCancel, false),
                    ] {
                        let button = egui::Button::new(command.label());
                        let button = if default {
                            button.fill(state.theme.accent)
                        } else {
                            button
                        };
                        if ui.add(button).clicked() {
                            out.push(command);
                        }
                    }
                });
            }
            Dialog::Recover(r) => {
                let name = r.original.as_ref().map_or_else(
                    || "a worksheet that was never saved".to_owned(),
                    |p| format!("\u{201c}{}\u{201d}", p.display()),
                );
                ui.label(RichText::new("Restore unsaved changes?").strong());
                ui.label(format!(
                    "OpenCalc stopped before saving changes to {name}. They were kept in a recovery file."
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for (command, default) in [
                        (CommandId::DialogRestore, true),
                        (CommandId::DialogDropRecovery, false),
                        (CommandId::DialogCancel, false),
                    ] {
                        let button = egui::Button::new(command.label());
                        let button = if default {
                            button.fill(state.theme.accent)
                        } else {
                            button
                        };
                        if ui.add(button).clicked() {
                            out.push(command);
                        }
                    }
                });
                ui.label(
                    RichText::new("Cancel leaves the recovery file for now.")
                        .small()
                        .color(state.theme.text_weak),
                );
            }
            Dialog::Message { title, text } => {
                ui.label(RichText::new(title).strong());
                ui.label(text);
                ui.add_space(8.0);
                if ui.button(CommandId::DialogOk.label()).clicked() {
                    out.push(CommandId::DialogOk);
                }
            }
        }
    });
    // Escape, or a click outside, answers "Cancel" / "OK".
    if response.should_close() {
        out.push(match dialog {
            Dialog::Unsaved { .. } | Dialog::Recover(_) => CommandId::DialogCancel,
            Dialog::Message { .. } => CommandId::DialogOk,
        });
    }
}
