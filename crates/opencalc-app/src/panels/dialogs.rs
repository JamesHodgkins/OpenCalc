//! The modal question or message in front of everything: "save changes?" and
//! error reports. Buttons push commands; nothing here changes state (rule R4).

use eframe::egui::{self, Context, Id, Modal, RichText};

use super::Commands;
use crate::app::filing::{Dialog, Then};
use crate::app::AppState;
use crate::commands::CommandId;
use crate::icons;
use crate::theme::metrics;

/// What a dialog asks, after the Windows convention: an icon beside the question,
/// and the buttons in a row at bottom right, the primary one first.
struct Layout<'a> {
    icon: char,
    title: &'a str,
    body: &'a str,
    note: Option<&'a str>,
    /// Left to right; the first is the default action.
    buttons: &'a [CommandId],
}

const BUTTON: egui::Vec2 = egui::vec2(88.0, 28.0);
/// Every edge of the dialog sits on one margin; the icon gets its own column so the
/// text always starts at the same x, however long it runs.
const WIDTH: f32 = 440.0;
const MARGIN: i8 = 24;
const ICON_SIZE: f32 = 40.0;
const ICON_COLUMN: f32 = ICON_SIZE + 20.0;

fn layout(ui: &mut egui::Ui, state: &AppState, out: &mut Commands, l: Layout) {
    let theme = state.theme;
    let inner = WIDTH - 2.0 * f32::from(MARGIN);
    ui.set_width(WIDTH);
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    // The message: icon column, then the text column.
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: MARGIN,
            right: MARGIN,
            top: 22,
            bottom: 22,
        })
        .show(ui, |ui| {
            ui.set_width(inner);
            ui.horizontal_top(|ui| {
                let colour = if l.icon == icons::WARNING {
                    theme.warning
                } else {
                    theme.text
                };
                // Reserve the whole column so the gap after the icon is fixed.
                let (rect, _) = ui
                    .allocate_exact_size(egui::vec2(ICON_COLUMN, ICON_SIZE), egui::Sense::hover());
                ui.painter().text(
                    rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    l.icon,
                    egui::FontId::proportional(ICON_SIZE),
                    colour,
                );
                ui.vertical(|ui| {
                    ui.set_width(inner - ICON_COLUMN);
                    ui.add(egui::Label::new(RichText::new(l.title).size(15.0).strong()).wrap());
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(RichText::new(l.body).color(theme.text_weak)).wrap());
                    if let Some(note) = l.note {
                        ui.add_space(6.0);
                        ui.label(RichText::new(note).small().color(theme.text_weak));
                    }
                });
            });
        });

    // The actions: a footer band the full width of the dialog, buttons on the right.
    let radius = metrics::WINDOW_RADIUS;
    egui::Frame::new()
        .fill(theme.base)
        .stroke(egui::Stroke::new(1.0_f32, theme.border))
        .corner_radius(egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius,
            se: radius,
        })
        .inner_margin(egui::Margin::symmetric(MARGIN, 12))
        .show(ui, |ui| {
            ui.set_width(inner);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (i, &command) in l.buttons.iter().enumerate().rev() {
                    let text = RichText::new(command.label());
                    let button = if i == 0 {
                        egui::Button::new(text.strong()).fill(theme.raised_high)
                    } else {
                        egui::Button::new(text)
                    };
                    let response = ui.add_sized(BUTTON, button);
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if response.clicked() {
                        out.push(command);
                    }
                }
            });
        });
}

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    let Some(dialog) = &state.dialog else { return };
    let response = Modal::new(Id::new("dialog"))
        .frame(egui::Frame::popup(&ctx.style()).inner_margin(0))
        .show(ctx, |ui| match dialog {
        Dialog::Unsaved { sheet, then } => {
            let name = state.sheets.get(*sheet).map_or("", |s| s.title());
            let title =
                format!("Save changes to \u{201c}{name}\u{201d}?");
            let body = match then {
                Then::Close(_) => "Your changes will be lost if you close without saving.",
                Then::Quit => "Your changes will be lost if you quit without saving.",
            };
            layout(
                ui,
                state,
                out,
                Layout {
                    icon: icons::WARNING,
                    title: &title,
                    body,
                    note: None,
                    buttons: &[
                        CommandId::DialogSave,
                        CommandId::DialogDiscard,
                        CommandId::DialogCancel,
                    ],
                },
            );
        }
        Dialog::Recover(r) => {
            let name = r.original.as_ref().map_or_else(
                || "a worksheet that was never saved".to_owned(),
                |p| format!("\u{201c}{}\u{201d}", p.display()),
            );
            let body = format!(
                "OpenCalc stopped before saving changes to {name}. They were kept in a recovery file."
            );
            layout(
                ui,
                state,
                out,
                Layout {
                    icon: icons::WARNING,
                    title: "Restore unsaved changes?",
                    body: &body,
                    note: Some("Cancel leaves the recovery file for now."),
                    buttons: &[
                        CommandId::DialogRestore,
                        CommandId::DialogDropRecovery,
                        CommandId::DialogCancel,
                    ],
                },
            );
        }
        Dialog::Message { title, text } => layout(
            ui,
            state,
            out,
            Layout {
                icon: icons::INFO,
                title,
                body: text,
                note: None,
                buttons: &[CommandId::DialogOk],
            },
        ),
    });
    // Escape, or a click outside, answers "Cancel" / "OK".
    if response.should_close() {
        out.push(match dialog {
            Dialog::Unsaved { .. } | Dialog::Recover(_) => CommandId::DialogCancel,
            Dialog::Message { .. } => CommandId::DialogOk,
        });
    }
}
