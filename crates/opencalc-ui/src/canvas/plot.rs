//! Painting an X-Y plot region on the canvas: frame, grid, axes (a second one on
//! the right when a series asks for it), ticks, labels, the data series and the
//! legend. Drawn with the painter like every other region, so it scales with the
//! canvas zoom and takes no input of its own (dragging a plot moves the region, as
//! for any other).

mod cartesian;
mod context;
mod geometry;
mod legend;
mod marks;
mod polar;
mod scales;
mod ticks;

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};
use opencalc_core::document::{PlotSpec, Region, RegionId};
use opencalc_core::eval::PlotData;

use super::adapt::vec;
use super::Env;
use crate::theme::{metrics, type_size};
use cartesian::chart;
pub use geometry::{Geometry, View};

pub use context::PlotCtx;
use context::MIN_TEXT;

/// Paint the plot region `r` on the canvas.
pub fn paint(painter: &Painter, env: &Env, r: &Region, spec: &PlotSpec, top_left: Pos2, zoom: f32) {
    let rect = Rect::from_min_size(top_left, vec(r.size) * zoom);
    paint_in(
        painter,
        &env.into(),
        r.id,
        spec,
        rect,
        zoom,
        &View::default(),
    );
}

/// Paint plot `id` into `rect`, showing the part of it that `view` asks for. Gives
/// back where things went, if a chart was drawn (not for an error or an empty plot).
pub fn paint_in(
    painter: &Painter,
    ctx: &PlotCtx,
    id: RegionId,
    spec: &PlotSpec,
    rect: Rect,
    zoom: f32,
    view: &View,
) -> Option<Geometry> {
    let theme = ctx.paper;
    let radius = eframe::egui::CornerRadius::same(metrics::RADIUS);
    painter.rect_filled(rect, radius, theme.surface);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, theme.border),
        StrokeKind::Inside,
    );
    let size = |base: f32| base * zoom;
    let font = |base: f32| FontId::proportional(size(base));
    let text_ok = size(type_size::SMALL) >= MIN_TEXT;
    let pad = size(8.0);

    let mut top = rect.min.y + pad;
    if text_ok && !spec.title.is_empty() {
        painter.text(
            Pos2::new(rect.center().x, top),
            Align2::CENTER_TOP,
            &spec.title,
            font(type_size::BODY),
            theme.text,
        );
        top += size(type_size::BODY + 6.0);
    }

    let note = |text: &str, colour: Color32| {
        if text_ok {
            let galley = painter.layout(
                text.to_owned(),
                font(type_size::SMALL),
                colour,
                (rect.width() - 2.0 * pad).max(1.0),
            );
            let at = Pos2::new(rect.min.x + pad, rect.center().y - galley.size().y / 2.0);
            painter.galley(at, galley, colour);
        }
    };
    match ctx.output.plots.get(&id) {
        None => note("\u{2026}", theme.text_weak),
        Some(Err(e)) => note(&format!("{} {e}", crate::icons::WARNING), theme.error),
        Some(Ok(data)) if data.is_empty() => note(
            "Choose the x values and the series to plot in the Plot window.",
            theme.text_weak,
        ),
        Some(Ok(data)) => {
            let c = Chart {
                spec,
                data,
                rect,
                top,
                zoom,
                view,
            };
            return chart(painter, ctx, &c);
        }
    }
    None
}

/// What `chart` draws: the plot, its data, where it goes and how it is looked at.
#[derive(Clone, Copy)]
struct Chart<'a> {
    spec: &'a PlotSpec,
    data: &'a PlotData,
    rect: Rect,
    /// Where the plotting area starts (below any title).
    top: f32,
    zoom: f32,
    view: &'a View,
}
