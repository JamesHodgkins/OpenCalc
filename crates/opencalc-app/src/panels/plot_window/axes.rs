//! The axes section of the plot window: limits, grid, legend and log axes.

use eframe::egui::{self, Ui};
use opencalc_core::document::{LegendMode, MathRegion, PlotKind, PlotOptions, PlotSpec, RegionId};

use super::{choose, field, send, small};
use crate::app::AppState;
use crate::canvas::CanvasCmd;

/// Axis limits, grid, legend and log axes.
pub(super) fn show(
    ui: &mut Ui,
    state: &AppState,
    id: RegionId,
    spec: &PlotSpec,
    out: &mut Vec<CanvasCmd>,
) {
    let o: &PlotOptions = &spec.options;
    let polar = o.kind == PlotKind::Polar;
    egui::CollapsingHeader::new("Axes and grid")
        .id_salt(("plot-axes", id.0))
        .show(ui, |ui| {
            let limits: [(&str, &MathRegion); 6] = [
                ("x minimum (blank: automatic)", &o.x_min),
                ("x maximum", &o.x_max),
                ("y minimum", &o.y_min),
                (
                    if polar {
                        "Outer radius (blank: automatic)"
                    } else {
                        "y maximum"
                    },
                    &o.y_max,
                ),
                ("Right axis minimum", &o.y2_min),
                ("Right axis maximum", &o.y2_max),
            ];
            for (n, (label, current)) in limits.into_iter().enumerate() {
                // A polar plot has one limit: how far out the outer ring goes.
                if polar && n != 3 {
                    continue;
                }
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
            if !polar {
                ui.checkbox(&mut x_log, "Logarithmic x axis");
                ui.checkbox(&mut y_log, "Logarithmic y axis");
                ui.checkbox(&mut y2_log, "Logarithmic right axis");
            }
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
