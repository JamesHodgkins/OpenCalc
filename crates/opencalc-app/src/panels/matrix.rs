//! The Insert Matrix window: how many rows and columns. It uses buttons rather than
//! typed numbers, so it never takes the keyboard from a region being edited. The
//! sizes live in egui's memory (they are not document state); Insert goes out as a
//! command (rule R4).

use eframe::egui::{self, Align2, Context, Id, Ui};

use super::Commands;
use crate::app::inserting::{MAX_CELLS, MAX_SIDE};
use crate::app::AppState;
use crate::commands::CommandId;

pub fn show(ctx: &Context, state: &AppState, out: &mut Commands) {
    if !state.matrix_open {
        return;
    }
    if state.active.is_none() {
        out.push(CommandId::MatrixClose);
        return;
    }
    let id = Id::new("matrix-size");
    let (mut rows, mut cols) = ctx.data(|d| d.get_temp::<(u8, u8)>(id)).unwrap_or((2, 2));
    let mut open = true;
    egui::Window::new("Insert Matrix")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(super::default_pos(ctx, Align2::CENTER_CENTER, [0.0, 0.0]))
        .show(ctx, |ui| {
            let (r, c) = (rows, cols);
            stepper(ui, "Rows", &mut rows, |n| n * c <= MAX_CELLS);
            stepper(ui, "Columns", &mut cols, |n| r * n <= MAX_CELLS);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Insert").clicked() {
                    out.push(CommandId::InsertMatrixSize(rows, cols));
                }
                if ui.button("Cancel").clicked() {
                    out.push(CommandId::MatrixClose);
                }
            });
        });
    ctx.data_mut(|d| d.insert_temp(id, (rows, cols)));
    if !open {
        out.push(CommandId::MatrixClose);
    }
}

/// A number with minus and plus buttons; plus stops where `allowed` says no.
fn stepper(ui: &mut Ui, label: &str, value: &mut u8, allowed: impl Fn(u8) -> bool) {
    ui.horizontal(|ui| {
        ui.label(format!("{label:8}"));
        if ui.button("\u{2212}").clicked() {
            *value = value.saturating_sub(1).max(1);
        }
        ui.label(value.to_string());
        let more = ui.add_enabled(
            *value < MAX_SIDE && allowed(*value + 1),
            egui::Button::new("+"),
        );
        if more.clicked() {
            *value += 1;
        }
    });
}
