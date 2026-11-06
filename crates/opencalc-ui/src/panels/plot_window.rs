//! The plot window: a small floating window next to the canvas that appears while
//! one plot region is selected and holds its fields (title, x values, series,
//! labels). It reads the document and only asks for edits, like every panel.

use eframe::egui::{self, Context, RichText, Ui};
use opencalc_core::document::{PlotSpec, RegionId, RegionKind, MAX_SERIES};
use opencalc_core::edit::Edit;

use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::theme::type_size;

/// The single selected plot region, if that is what is selected.
pub fn selected_plot(state: &AppState) -> Option<(RegionId, &PlotSpec)> {
    let sheet = state.active_sheet()?;
    let [id] = sheet.canvas.selection.as_slice() else {
        return None;
    };
    match &sheet.doc.region(*id)?.kind {
        RegionKind::Plot(spec) => Some((*id, spec)),
        _ => None,
    }
}

/// Show the window for the selected plot, if there is one.
pub fn show(ctx: &Context, state: &AppState, out: &mut Vec<CanvasCmd>) {
    let Some((id, spec)) = selected_plot(state) else {
        return;
    };
    // Top right of the canvas, clear of the page.
    let corner = state.viewport.right_top() + egui::vec2(-264.0, 12.0);
    egui::Window::new("Plot")
        .id(egui::Id::new("plot-window"))
        .fixed_pos(corner)
        .fixed_size([240.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| plot_editor(ui, state, id, spec, out));
}

/// One text field of the plot editor. Each change is one edit; losing focus ends
/// the run, so a whole field is one undo step (see `History`).
fn field(
    ui: &mut Ui,
    state: &AppState,
    id: RegionId,
    out: &mut Vec<CanvasCmd>,
    label: &str,
    text: &str,
    make: &dyn Fn(&str) -> PlotSpec,
) {
    ui.label(
        RichText::new(label)
            .size(type_size::SMALL)
            .color(state.theme.text_weak),
    );
    let mut buffer = text.to_owned();
    let response = ui.add(egui::TextEdit::singleline(&mut buffer).desired_width(f32::INFINITY));
    if response.changed() {
        out.push(CanvasCmd::Edit(Edit::SetPlot {
            id,
            spec: make(&buffer),
        }));
    }
    if response.lost_focus() {
        out.push(CanvasCmd::Seal);
    }
}

/// Fields for what a plot shows. Each change is an edit of the whole spec, built
/// from the current one, so the panel keeps no state of its own.
fn plot_editor(
    ui: &mut Ui,
    state: &AppState,
    id: RegionId,
    spec: &PlotSpec,
    out: &mut Vec<CanvasCmd>,
) {
    field(ui, state, id, out, "Title", &spec.title, &|t| {
        spec.clone().with_title(t)
    });
    field(ui, state, id, out, "x values", spec.x.source(), &|t| {
        spec.clone().with_x(t)
    });
    for (k, series) in spec.series.iter().enumerate() {
        field(
            ui,
            state,
            id,
            out,
            &format!("Series {}", k + 1),
            series.source(),
            &|t| spec.clone().with_series(k, t),
        );
    }
    ui.horizontal(|ui| {
        if spec.series.len() < MAX_SERIES && ui.button("Add series").clicked() {
            let next = spec.series.len();
            out.push(CanvasCmd::Edit(Edit::SetPlot {
                id,
                spec: spec.clone().with_series(next, ""),
            }));
            out.push(CanvasCmd::Seal);
        }
        if spec.series.len() > 1 && ui.button("Remove last").clicked() {
            let last = spec.series.len() - 1;
            out.push(CanvasCmd::Edit(Edit::SetPlot {
                id,
                spec: spec.clone().without_series(last),
            }));
            out.push(CanvasCmd::Seal);
        }
    });
    field(ui, state, id, out, "x label", &spec.x_label, &|t| {
        spec.clone().with_labels(t, &spec.y_label)
    });
    field(ui, state, id, out, "y label", &spec.y_label, &|t| {
        spec.clone().with_labels(&spec.x_label, t)
    });
}
