//! The table window: a small floating window beside the canvas that appears while
//! one table region is selected. It holds the table's name, the heading and unit
//! of each column and every cell, and buttons to add and remove rows and columns.
//! Like every panel it reads the document and only asks for edits.

use eframe::egui::{self, Context, RichText, Ui};
use opencalc_core::document::{RegionId, RegionKind, TableSpec, MAX_COLUMNS};
use opencalc_core::edit::Edit;

use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::theme::type_size;

/// The single selected table region, if that is what is selected.
pub fn selected_table(state: &AppState) -> Option<(RegionId, &TableSpec)> {
    let sheet = state.active_sheet()?;
    let [id] = sheet.canvas.selection.as_slice() else {
        return None;
    };
    match &sheet.doc.region(*id)?.kind {
        RegionKind::Table(spec) => Some((*id, spec)),
        _ => None,
    }
}

/// Show the window for the selected table, if there is one.
pub fn show(ctx: &Context, state: &AppState, out: &mut Vec<CanvasCmd>) {
    let Some((id, spec)) = selected_table(state) else {
        return;
    };
    let corner = state.viewport.right_top() + egui::vec2(-364.0, 12.0);
    let tallest = (state.viewport.height() - 60.0).max(160.0);
    egui::Window::new("Table")
        .id(egui::Id::new("table-window"))
        .fixed_pos(corner)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::ScrollArea::both()
                .max_height(tallest)
                .max_width(340.0)
                .show(ui, |ui| editor(ui, state, id, spec, out));
        });
}

/// Replace the table with `spec`, as one undo step.
fn send(out: &mut Vec<CanvasCmd>, id: RegionId, spec: TableSpec) {
    out.push(CanvasCmd::Edit(Edit::SetTable {
        id,
        spec: Box::new(spec),
    }));
    out.push(CanvasCmd::Seal);
}

/// A one-line text box. Each keystroke is an edit; losing focus ends the run, so a
/// whole entry is one undo step.
fn entry(
    ui: &mut Ui,
    out: &mut Vec<CanvasCmd>,
    id: RegionId,
    text: &str,
    width: f32,
    hint: &str,
    make: &dyn Fn(&str) -> TableSpec,
) {
    let mut buffer = text.to_owned();
    let response = ui.add(
        egui::TextEdit::singleline(&mut buffer)
            .desired_width(width)
            .hint_text(hint)
            .char_limit(opencalc_core::document::MAX_TABLE_TEXT),
    );
    if response.changed() {
        out.push(CanvasCmd::Edit(Edit::SetTable {
            id,
            spec: Box::new(make(&buffer)),
        }));
    }
    if response.lost_focus() {
        out.push(CanvasCmd::Seal);
    }
}

fn editor(ui: &mut Ui, state: &AppState, id: RegionId, spec: &TableSpec, out: &mut Vec<CanvasCmd>) {
    let weak = |ui: &mut Ui, text: &str| {
        ui.label(
            RichText::new(text)
                .size(type_size::SMALL)
                .color(state.theme.text_weak),
        );
    };
    weak(ui, "Name (the table defines this)");
    entry(ui, out, id, &spec.name, 180.0, "e.g. loads", &|t| {
        spec.clone().with_name(t)
    });
    ui.add_space(6.0);
    weak(ui, "Heading and unit of each column, then the numbers");
    egui::Grid::new(("table-grid", id.0))
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for (j, c) in spec.columns.iter().enumerate() {
                ui.vertical(|ui| {
                    entry(ui, out, id, &c.header, 64.0, "heading", &|t| {
                        spec.clone().with_header(j, t)
                    });
                    entry(ui, out, id, &c.unit, 64.0, "unit", &|t| {
                        spec.clone().with_unit(j, t)
                    });
                });
            }
            ui.end_row();
            for (i, row) in spec.rows.iter().enumerate() {
                for (j, cell) in row.iter().enumerate() {
                    entry(ui, out, id, cell, 64.0, "", &|t| {
                        spec.clone().with_cell(i, j, t)
                    });
                }
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let full = spec.rows.len() * spec.cols() >= opencalc_core::document::MAX_CELLS;
        if ui
            .add_enabled(!full, egui::Button::new("Add row"))
            .clicked()
        {
            send(out, id, spec.clone().with_row_added());
        }
        if ui
            .add_enabled(spec.row_count() > 1, egui::Button::new("Remove last row"))
            .clicked()
        {
            send(out, id, spec.clone().with_row_removed(spec.row_count() - 1));
        }
        if ui
            .add_enabled(
                spec.cols() < MAX_COLUMNS && !full,
                egui::Button::new("Add column"),
            )
            .clicked()
        {
            send(out, id, spec.clone().with_column_added());
        }
        if ui
            .add_enabled(spec.cols() > 1, egui::Button::new("Remove last column"))
            .clicked()
        {
            send(out, id, spec.clone().with_column_removed(spec.cols() - 1));
        }
    });
}
