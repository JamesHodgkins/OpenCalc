//! The plot view: a larger, interactive look at one plot. Hover for the values (the
//! nearest data point, in the units of its axis), scroll to zoom about the pointer,
//! drag to pan, double-click to go back to the plot's own view.
//!
//! Which part of the plot is shown is only a way of looking at it, so it is kept
//! here, in the window's own memory, and never reaches the document: nothing is
//! saved, nothing is undone, the plot on the page is unchanged.

use eframe::egui::{self, Context, FontId, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use opencalc_core::document::{PlotSpec, RegionId, RegionKind};

use crate::app::AppState;
use crate::canvas::plot::{paint_in, Geometry, PlotCtx, View};
use crate::sheet::OpenSheet;
use crate::theme::{type_size, Theme};

/// How close (in screen pixels) the pointer must be to a point to pick it.
const REACH: f32 = 14.0;
/// Zoom per scroll point: a notch of the wheel is about 50.
const ZOOM_PER_POINT: f64 = 0.002;

/// Where the open window's plot is remembered (the region number), if one is open.
fn open_key() -> Id {
    Id::new("plot-view-open")
}

fn view_key(id: RegionId) -> Id {
    Id::new(("plot-view", id.0))
}

/// Open the view of plot `id`, replacing any other.
pub fn open(ctx: &Context, id: RegionId) {
    ctx.data_mut(|d| d.insert_temp(open_key(), id.0));
}

fn close(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<u64>(open_key()));
}

fn plot_of(sheet: &OpenSheet, id: RegionId) -> Option<&PlotSpec> {
    match &sheet.doc.region(id)?.kind {
        RegionKind::Plot(spec) => Some(spec),
        _ => None,
    }
}

/// Show the window if one is open and its plot still exists.
pub fn show(ctx: &Context, state: &AppState) {
    let Some(raw) = ctx.data(|d| d.get_temp::<u64>(open_key())) else {
        return;
    };
    let id = RegionId(raw);
    let Some((sheet, spec)) = state
        .active_sheet()
        .and_then(|s| plot_of(s, id).map(|p| (s, p)))
    else {
        close(ctx);
        return;
    };
    let mut open = true;
    egui::Window::new("Plot view")
        .id(Id::new("plot-view-window"))
        .open(&mut open)
        .default_size([680.0, 470.0])
        .min_size([320.0, 240.0])
        .resizable(true)
        .show(ctx, |ui| content(ui, state, sheet, id, spec));
    if !open {
        close(ctx);
    }
}

fn content(ui: &mut Ui, state: &AppState, sheet: &OpenSheet, id: RegionId, spec: &PlotSpec) {
    let ctx = ui.ctx().clone();
    let paper = Theme::paper();
    let plot_ctx = PlotCtx {
        paper: &paper,
        output: &state.output,
        units: sheet.doc.unit_system(),
    };
    let mut view = ctx
        .data(|d| d.get_temp::<View>(view_key(id)))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Scroll to zoom, drag to pan, double-click to reset")
                .size(type_size::SMALL)
                .color(state.theme.text_weak),
        );
        if ui
            .add_enabled(view.is_moved(), egui::Button::new("Reset view"))
            .clicked()
        {
            view = View::default();
        }
    });
    let status_height = 22.0;
    let size = Vec2::new(
        ui.available_width(),
        (ui.available_height() - status_height).max(120.0),
    );
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let geometry = paint_in(&painter, &plot_ctx, id, spec, response.rect, 1.0, &view);
    let mut status = String::from("Move over the plot for values.");
    if let Some(g) = &geometry {
        let before = view;
        if response.double_clicked() {
            view = View::default();
        } else {
            if response.dragged() {
                view = view.panned(g, response.drag_delta());
            }
            if let Some(factor) = zoom_factor(ui, &response) {
                let at = response
                    .hover_pos()
                    .map_or(g.area.center(), |p| clamp_to(g.area, p));
                view = view.zoomed(g, at, factor);
            }
        }
        if view != before {
            ctx.request_repaint();
        }
        if let Some(p) = response.hover_pos().filter(|p| g.contains(*p)) {
            status = g.describe_cursor(p);
            readout(&painter, &paper, g, p);
        }
    }
    ui.label(
        egui::RichText::new(status)
            .size(type_size::SMALL)
            .color(state.theme.text_weak),
    );
    ctx.data_mut(|d| d.insert_temp(view_key(id), view));
}

/// The zoom asked for this frame (below 1 zooms in), from the wheel or a pinch.
fn zoom_factor(ui: &Ui, response: &egui::Response) -> Option<f64> {
    if !response.hovered() {
        return None;
    }
    let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
    let factor = f64::from(pinch) * (-f64::from(scroll) * ZOOM_PER_POINT).exp();
    ((factor - 1.0).abs() > 1e-9).then_some(factor)
}

fn clamp_to(area: Rect, p: Pos2) -> Pos2 {
    Pos2::new(
        p.x.clamp(area.min.x, area.max.x),
        p.y.clamp(area.min.y, area.max.y),
    )
}

/// A crosshair at the pointer and, when a data point is near, a ring round it and
/// what it is.
fn readout(painter: &egui::Painter, paper: &Theme, g: &Geometry, p: Pos2) {
    let clipped = painter.with_clip_rect(g.area);
    let faint = Stroke::new(1.0_f32, paper.border_strong.gamma_multiply(0.6));
    clipped.vline(p.x, g.area.y_range(), faint);
    clipped.hline(g.area.x_range(), p.y, faint);
    let Some(hit) = g.nearest(p, REACH) else {
        return;
    };
    painter.circle_stroke(hit.at, 6.0, Stroke::new(2.0_f32, paper.accent));
    let text = painter.layout_no_wrap(
        g.describe(&hit),
        FontId::proportional(type_size::SMALL),
        paper.text,
    );
    // Beside the point, kept inside the plot: on the other side near the right or top.
    let mut at = hit.at + Vec2::new(12.0, -text.size().y - 12.0);
    if at.x + text.size().x + 8.0 > g.area.max.x {
        at.x = hit.at.x - text.size().x - 12.0;
    }
    if at.y < g.area.min.y + 4.0 {
        at.y = hit.at.y + 12.0;
    }
    let panel = Rect::from_min_size(at, text.size()).expand(4.0);
    painter.rect_filled(panel, 3.0, paper.surface);
    painter.rect_stroke(
        panel,
        3.0,
        Stroke::new(1.0_f32, paper.border_strong),
        egui::StrokeKind::Inside,
    );
    painter.galley(at, text, paper.text);
}
