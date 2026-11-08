//! The plot window: a small floating window next to the canvas that appears while
//! one plot region is selected and holds its fields (title, x values, series and
//! how each is drawn, labels, axis limits, grid, legend and log axes). It reads the
//! document and only asks for edits, like every panel.

use eframe::egui::{self, Context, RichText, Ui};
use opencalc_core::document::{
    LegendMode, Marker, MathRegion, PlotOptions, PlotSpec, RegionId, RegionKind, SeriesKind,
    SeriesStyle, YAxis, MAX_SERIES, PALETTE_SIZE,
};
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
    out.push(CanvasCmd::Edit(Edit::SetPlot { id, spec }));
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
            spec: make(&buffer),
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

/// How series `k` is drawn: type, marker, colour and legend name.
fn series_style(
    ui: &mut Ui,
    state: &AppState,
    id: RegionId,
    spec: &PlotSpec,
    k: usize,
    out: &mut Vec<CanvasCmd>,
) {
    let style = spec.style(k);
    egui::CollapsingHeader::new(format!("Style of series {}", k + 1))
        .id_salt(("plot-style", id.0, k))
        .show(ui, |ui| {
            small(ui, state, "Type");
            if let Some(kind) = choose(
                ui,
                ("plot-kind", id.0, k),
                style.kind,
                &SeriesKind::ALL,
                &|o| o.label().to_owned(),
            ) {
                let new = SeriesStyle {
                    kind,
                    ..style.clone()
                };
                send(out, id, spec.clone().with_style(k, new));
            }
            if matches!(
                style.kind,
                SeriesKind::Markers | SeriesKind::LineMarkers | SeriesKind::Stem
            ) {
                small(ui, state, "Marker");
                if let Some(marker) = choose(
                    ui,
                    ("plot-marker", id.0, k),
                    style.marker,
                    &Marker::ALL,
                    &|o| o.label().to_owned(),
                ) {
                    let new = SeriesStyle {
                        marker,
                        ..style.clone()
                    };
                    send(out, id, spec.clone().with_style(k, new));
                }
            }
            small(ui, state, "Colour");
            let colours: Vec<Option<u8>> = std::iter::once(None)
                .chain((0..PALETTE_SIZE).map(Some))
                .collect();
            let describe = |c: Option<u8>| match c {
                None => "Automatic".to_owned(),
                Some(n) => format!("\u{25a0} Colour {}", n + 1),
            };
            if let Some(colour) = choose(
                ui,
                ("plot-colour", id.0, k),
                style.colour,
                &colours,
                &describe,
            ) {
                let new = SeriesStyle {
                    colour,
                    ..style.clone()
                };
                send(out, id, spec.clone().with_style(k, new));
            }
            field(ui, state, id, out, "Legend name", &style.name, &|t| {
                let new = SeriesStyle {
                    name: t.to_owned(),
                    ..style.clone()
                };
                spec.clone().with_style(k, new)
            });
            small(ui, state, "Measured on");
            if let Some(axis) = choose(ui, ("plot-axis", id.0, k), style.axis, &YAxis::ALL, &|o| {
                o.label().to_owned()
            }) {
                let new = SeriesStyle {
                    axis,
                    ..style.clone()
                };
                send(out, id, spec.clone().with_style(k, new));
            }
            field(
                ui,
                state,
                id,
                out,
                "Own x values (blank: the plot's x values)",
                &style.x,
                &|t| {
                    let new = SeriesStyle {
                        x: t.to_owned(),
                        ..style.clone()
                    };
                    spec.clone().with_style(k, new)
                },
            );
        });
}

/// Axis limits, grid, legend and log axes.
fn axes(ui: &mut Ui, state: &AppState, id: RegionId, spec: &PlotSpec, out: &mut Vec<CanvasCmd>) {
    let o: &PlotOptions = &spec.options;
    egui::CollapsingHeader::new("Axes and grid")
        .id_salt(("plot-axes", id.0))
        .show(ui, |ui| {
            let limits: [(&str, &MathRegion); 6] = [
                ("x minimum (blank: automatic)", &o.x_min),
                ("x maximum", &o.x_max),
                ("y minimum", &o.y_min),
                ("y maximum", &o.y_max),
                ("Right axis minimum", &o.y2_min),
                ("Right axis maximum", &o.y2_max),
            ];
            for (n, (label, current)) in limits.into_iter().enumerate() {
                field(ui, state, id, out, label, current.source(), &|t| {
                    let m = MathRegion::new(t);
                    let mut options = o.clone();
                    match n {
                        0 => options.x_min = m,
                        1 => options.x_max = m,
                        2 => options.y_min = m,
                        3 => options.y_max = m,
                        4 => options.y2_min = m,
                        _ => options.y2_max = m,
                    }
                    spec.clone().with_options(options)
                });
            }
            let mut grid = o.grid;
            let (mut x_log, mut y_log, mut y2_log) = (o.x_log, o.y_log, o.y2_log);
            ui.checkbox(&mut grid, "Grid");
            ui.checkbox(&mut x_log, "Logarithmic x axis");
            ui.checkbox(&mut y_log, "Logarithmic y axis");
            ui.checkbox(&mut y2_log, "Logarithmic right axis");
            if (grid, x_log, y_log, y2_log) != (o.grid, o.x_log, o.y_log, o.y2_log) {
                let options = PlotOptions {
                    grid,
                    x_log,
                    y_log,
                    y2_log,
                    ..o.clone()
                };
                send(out, id, spec.clone().with_options(options));
            }
            small(ui, state, "Legend");
            if let Some(legend) = choose(
                ui,
                ("plot-legend", id.0),
                o.legend,
                &LegendMode::ALL,
                &|m| m.label().to_owned(),
            ) {
                let options = PlotOptions {
                    legend,
                    ..o.clone()
                };
                send(out, id, spec.clone().with_options(options));
            }
        });
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
        series_style(ui, state, id, spec, k, out);
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
    field(ui, state, id, out, "x label", &spec.x_label, &|t| {
        spec.clone().with_labels(t, &spec.y_label)
    });
    field(ui, state, id, out, "y label", &spec.y_label, &|t| {
        spec.clone().with_labels(&spec.x_label, t)
    });
    if spec.uses_right_axis() {
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
    axes(ui, state, id, spec, out);
    ui.add_space(4.0);
    if ui
        .button("Explore this plot\u{2026}")
        .on_hover_text("A larger window: hover for values, scroll to zoom, drag to pan")
        .clicked()
    {
        super::plot_view::open(ui.ctx(), id);
    }
}
