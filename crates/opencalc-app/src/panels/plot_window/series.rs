//! The style section of one series in the plot window.

use eframe::egui::{self, Ui};
use opencalc_core::document::{
    Marker, PlotKind, PlotSpec, RegionId, SeriesKind, SeriesStyle, YAxis, PALETTE_SIZE,
};

use super::{choose, field, send, small};
use crate::app::AppState;
use crate::canvas::CanvasCmd;

/// How series `k` is drawn: type, marker, colour and legend name.
pub(super) fn style(
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
            let polar = spec.options.kind == PlotKind::Polar;
            if !polar {
                small(ui, state, "Measured on");
                if let Some(axis) =
                    choose(ui, ("plot-axis", id.0, k), style.axis, &YAxis::ALL, &|o| {
                        o.label().to_owned()
                    })
                {
                    let new = SeriesStyle {
                        axis,
                        ..style.clone()
                    };
                    send(out, id, spec.clone().with_style(k, new));
                }
            }
            field(
                ui,
                state,
                id,
                out,
                if polar {
                    "Own angles (blank: the plot's angles)"
                } else {
                    "Own x values (blank: the plot's x values)"
                },
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
