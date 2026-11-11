//! The plot window: a small floating window next to the canvas that appears while
//! one plot region is selected and holds its fields (title, x values, series and
//! how each is drawn, labels, axis limits, grid, legend and log axes). It reads the
//! document and only asks for edits, like every panel.

use eframe::egui::{self, Context, RichText, Ui};
use opencalc_core::document::{PlotKind, PlotOptions, PlotSpec, RegionId, RegionKind, MAX_SERIES};
use opencalc_core::edit::Edit;

use crate::app::AppState;
use crate::canvas::CanvasCmd;
use crate::theme::type_size;

mod axes;
mod series;

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
    let tallest = (state.viewport.height() - 60.0).max(160.0);
    egui::Window::new("Plot")
        .id(egui::Id::new("plot-window"))
        .fixed_pos(corner)
        .fixed_size([240.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(tallest)
                .show(ui, |ui| plot_editor(ui, state, id, spec, out));
        });
}

/// Replace the plot's spec with `spec`, as one undo step.
fn send(out: &mut Vec<CanvasCmd>, id: RegionId, spec: PlotSpec) {
    out.push(CanvasCmd::Edit(Edit::SetPlot {
        id,
        spec: Box::new(spec),
    }));
    out.push(CanvasCmd::Seal);
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
    let response = ui.add(
        egui::TextEdit::singleline(&mut buffer)
            .char_limit(opencalc_core::persist::MAX_TEXT_CHARS)
            .desired_width(f32::INFINITY),
    );
    if response.changed() {
        out.push(CanvasCmd::Edit(Edit::SetPlot {
            id,
            spec: Box::new(make(&buffer)),
        }));
    }
    if response.lost_focus() {
        out.push(CanvasCmd::Seal);
    }
}

/// A drop-down of `options`; returns the new choice when it changed.
fn choose<T: Copy + PartialEq>(
    ui: &mut Ui,
    salt: impl std::hash::Hash,
    current: T,
    options: &[T],
    text: &dyn Fn(T) -> String,
) -> Option<T> {
    let mut chosen = current;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(text(current))
        .width(150.0)
        .show_ui(ui, |ui| {
            for option in options {
                ui.selectable_value(&mut chosen, *option, text(*option));
            }
        });
    (chosen != current).then_some(chosen)
}

fn small(ui: &mut Ui, state: &AppState, text: &str) {
    ui.label(
        RichText::new(text)
            .size(type_size::SMALL)
            .color(state.theme.text_weak),
    );
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
    let polar = spec.options.kind == PlotKind::Polar;
    small(ui, state, "Plot type");
    if let Some(kind) = choose(
        ui,
        ("plot-type", id.0),
        spec.options.kind,
        &PlotKind::ALL,
        &|k| k.label().to_owned(),
    ) {
        let options = PlotOptions {
            kind,
            ..(*spec.options).clone()
        };
        send(out, id, spec.clone().with_options(options));
    }
    field(ui, state, id, out, "Title", &spec.title, &|t| {
        spec.clone().with_title(t)
    });
    field(
        ui,
        state,
        id,
        out,
        if polar {
            "Angles (radians; 30 deg works)"
        } else {
            "x values"
        },
        spec.x.source(),
        &|t| spec.clone().with_x(t),
    );
    for (k, series) in spec.series.iter().enumerate() {
        field(
            ui,
            state,
            id,
            out,
            &format!("{} {}", if polar { "Radius" } else { "Series" }, k + 1),
            series.source(),
            &|t| spec.clone().with_series(k, t),
        );
        series::style(ui, state, id, spec, k, out);
    }
    ui.horizontal(|ui| {
        if spec.series.len() < MAX_SERIES && ui.button("Add series").clicked() {
            let next = spec.series.len();
            send(out, id, spec.clone().with_series(next, ""));
        }
        if spec.series.len() > 1 && ui.button("Remove last").clicked() {
            let last = spec.series.len() - 1;
            send(out, id, spec.clone().without_series(last));
        }
    });
    field(
        ui,
        state,
        id,
        out,
        if polar { "Angle caption" } else { "x label" },
        &spec.x_label,
        &|t| spec.clone().with_labels(t, &spec.y_label),
    );
    field(
        ui,
        state,
        id,
        out,
        if polar { "Radius caption" } else { "y label" },
        &spec.y_label,
        &|t| spec.clone().with_labels(&spec.x_label, t),
    );
    if !polar && spec.uses_right_axis() {
        field(
            ui,
            state,
            id,
            out,
            "Right axis label",
            &spec.options.y2_label,
            &|t| spec.clone().with_y2_label(t),
        );
    }
    axes::show(ui, state, id, spec, out);
    ui.add_space(4.0);
    if ui
        .button("Explore this plot\u{2026}")
        .on_hover_text("A larger window: hover for values, scroll to zoom, drag to pan")
        .clicked()
    {
        super::plot_view::open(ui.ctx(), id);
    }
}
