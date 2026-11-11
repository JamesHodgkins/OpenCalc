//! The Keyboard shortcuts window. The command shortcuts are read from the key
//! bindings themselves (`commands::keys`), so a key listed here is a key that
//! works. The editing keys and mouse actions are handled where the text is edited
//! or the canvas is read, so they are listed here by hand.

use eframe::egui::{self, Align2, Context, RichText, Ui};

use super::Commands;
use crate::app::AppState;
use crate::commands::{keys, CommandId};
use crate::theme::type_size;

/// Keys that work while a math or text region is being edited.
const EDITING: &[(&str, &str)] = &[
    ("Enter", "Finish, and start the next input below"),
    ("Escape", "Finish editing (or close the suggestions)"),
    ("Tab / Enter", "Accept the highlighted suggestion"),
    ("Space", "Select the next larger part of the expression"),
    (
        "\\name then Space or Tab",
        "Greek letter (\\alpha becomes \u{3b1})",
    ),
    (
        "Ctrl+G",
        "Swap the letter before the caret, or the selection, between Latin and Greek",
    ),
    (":", "Types :=, to define a name"),
    ("Arrow at the edge", "Leave the region for the crosshair"),
    (
        "Ctrl+Enter",
        "Finish a text region and start the next input",
    ),
    ("Ctrl+A", "Select everything in the region"),
];

/// Mouse and wheel actions on the worksheet.
const MOUSE: &[(&str, &str)] = &[
    (
        "Click",
        "Select a region (Shift or Ctrl adds to the selection)",
    ),
    (
        "Double-click",
        "Edit a region, or select a part of the expression",
    ),
    ("Drag", "Move the selected regions, or select a group"),
    ("Alt + drag", "Move without snapping to the grid"),
    ("Wheel", "Scroll"),
    ("Ctrl + wheel", "Zoom"),
    ("Middle drag", "Pan"),
];

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.shortcuts_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Keyboard shortcuts")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(480.0)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(super::default_pos(ctx, Align2::CENTER_CENTER, [0.0, 0.0]))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    let weak = state.theme.text_weak;
                    let commands: Vec<(String, String)> = keys::listed()
                        .into_iter()
                        .map(|(c, keys)| (c.label().to_owned(), keys))
                        .collect();
                    section(ui, "COMMANDS", &commands, weak);
                    section(ui, "WHILE EDITING", &owned(EDITING), weak);
                    section(ui, "MOUSE", &owned(MOUSE), weak);
                });
        });
    if !open {
        out.push(CommandId::HelpShortcutsClose);
    }
}

fn owned(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

fn section(ui: &mut Ui, title: &str, rows: &[(String, String)], weak: egui::Color32) {
    ui.label(RichText::new(title).size(type_size::SMALL).color(weak));
    egui::Grid::new(title)
        .num_columns(2)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            for (left, right) in rows {
                // Commands read "Save   Ctrl+S"; the other tables read "Key   what it does".
                if title == "COMMANDS" {
                    ui.label(left);
                    ui.label(RichText::new(right).strong());
                } else {
                    ui.label(RichText::new(left).strong());
                    ui.label(right);
                }
                ui.end_row();
            }
        });
    ui.add_space(8.0);
}
