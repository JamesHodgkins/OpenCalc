//! Painting an X-Y plot region on the canvas: frame, grid, axes (a second one on
//! the right when a series asks for it), ticks, labels, the data series and the
//! legend. Drawn with the painter like every other region, so it scales with the
//! canvas zoom and takes no input of its own (dragging a plot moves the region, as
//! for any other).

mod context;
mod geometry;
mod legend;
mod marks;
mod scales;
mod ticks;

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};
use opencalc_core::document::{LegendMode, PlotSpec, Region, RegionId, SeriesKind, SeriesStyle};
use opencalc_core::eval::PlotData;

use super::adapt::vec;
use super::Env;
use crate::theme::{metrics, type_size};
pub use geometry::{Geometry, View};
use legend::Entry;
use marks::{Frame, Look};
use scales::Scales;

pub use context::PlotCtx;
use context::{caption, series_name, used_series, MIN_TEXT};

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

fn chart(painter: &Painter, ctx: &PlotCtx, c: &Chart) -> Option<Geometry> {
    let Chart {
        spec,
        data,
        rect,
        top,
        zoom,
        view,
    } = *c;
    let theme = ctx.paper;
    let size = |base: f32| base * zoom;
    let text_ok = size(type_size::SMALL) >= MIN_TEXT;
    let font = FontId::proportional(size(type_size::SMALL));
    let pad = size(8.0);
    let used = used_series(spec);
    let styles: Vec<SeriesStyle> = (0..data.series.len())
        .map(|k| spec.style(used.get(k).copied().unwrap_or(k)))
        .collect();
    let scales = Scales::new(ctx, data, &styles, view);
    let has_right = scales.y2.is_some();

    // The plotting area leaves room for tick labels and the axis captions.
    let left = if text_ok { size(46.0) } else { pad };
    let right = if text_ok && has_right {
        size(46.0)
    } else {
        pad
    };
    let bottom = if text_ok { size(34.0) } else { pad };
    let heading = if text_ok { size(16.0) } else { 0.0 };
    let area = Rect::from_min_max(
        Pos2::new(rect.min.x + left, top + heading),
        Pos2::new(rect.max.x - right, rect.max.y - bottom),
    );
    if area.width() < 4.0 || area.height() < 4.0 {
        return None;
    }
    // A position from values already in the axes' units, on the left or right axis.
    let to_screen = |x: f64, y: f64, on_right: bool| {
        let (axis, _) = scales.vertical(on_right);
        Pos2::new(
            area.min.x + scales.x.fraction(x) as f32 * area.width(),
            area.max.y - axis.fraction(y) as f32 * area.height(),
        )
    };

    let x_ticks = scales
        .x
        .ticks((area.width() / size(48.0)).max(2.0) as usize);
    let y_target = (area.height() / size(36.0)).max(2.0) as usize;
    let y_ticks = scales.y.ticks(y_target);
    let grid = Stroke::new(1.0_f32, theme.raised);
    let tick = Stroke::new(1.0_f32, theme.border_strong);
    let tick_len = size(4.0);
    for &t in &x_ticks.values {
        let at = to_screen(t, scales.y.lo, false);
        if spec.options.grid {
            painter.vline(at.x, area.y_range(), grid);
        } else {
            painter.vline(at.x, (area.max.y - tick_len)..=area.max.y, tick);
        }
        if text_ok {
            painter.text(
                Pos2::new(at.x, area.max.y + size(3.0)),
                Align2::CENTER_TOP,
                x_ticks.label(t),
                font.clone(),
                theme.text_weak,
            );
        }
    }
    for &t in &y_ticks.values {
        let at = to_screen(scales.x.lo, t, false);
        if spec.options.grid {
            painter.hline(area.x_range(), at.y, grid);
        } else {
            painter.hline(area.min.x..=(area.min.x + tick_len), at.y, tick);
        }
        if text_ok {
            painter.text(
                Pos2::new(area.min.x - size(4.0), at.y),
                Align2::RIGHT_CENTER,
                y_ticks.label(t),
                font.clone(),
                theme.text_weak,
            );
        }
    }
    // The right axis has its own ticks and labels but no grid lines of its own: two
    // sets of lines would not agree.
    if let Some(axis) = &scales.y2 {
        let ticks = axis.ticks(y_target);
        for &t in &ticks.values {
            let at = to_screen(scales.x.lo, t, true);
            painter.hline((area.max.x - tick_len)..=area.max.x, at.y, tick);
            if text_ok {
                painter.text(
                    Pos2::new(area.max.x + size(4.0), at.y),
                    Align2::LEFT_CENTER,
                    ticks.label(t),
                    font.clone(),
                    theme.text_weak,
                );
            }
        }
    }
    painter.rect_stroke(
        area,
        0.0,
        Stroke::new(1.0_f32, theme.border_strong),
        StrokeKind::Inside,
    );

    // Data, clipped to the plotting area.
    let clipped = painter.with_clip_rect(area);
    let screen: Vec<Vec<Pos2>> = data
        .series
        .iter()
        .map(|s| {
            let (_, y_scale) = scales.vertical(s.right);
            s.points
                .iter()
                .map(|p| to_screen(p.0 / scales.x_scale, p.1 / y_scale, s.right))
                .collect()
        })
        .collect();
    // Stems and bars grow from zero (or the bottom of a log axis), on their own axis.
    let baseline = |on_right: bool| {
        let (axis, _) = scales.vertical(on_right);
        let value = if axis.log {
            axis.lo
        } else {
            0.0_f64.clamp(axis.lo, axis.hi)
        };
        to_screen(scales.x.lo, value, on_right).y
    };
    let bar_series: Vec<usize> = (0..styles.len())
        .filter(|k| styles[*k].kind == SeriesKind::Bar)
        .collect();
    let slot = marks::bar_slot(
        bar_series
            .iter()
            .flat_map(|k| screen[*k].iter().map(|p| p.x)),
        size(24.0),
    );
    let bar_width = (0.8 * slot / bar_series.len().max(1) as f32).max(1.0);
    for (k, line) in screen.iter().enumerate() {
        let style = &styles[k];
        let colour = ctx.paper.series[style.colour.map_or(k, usize::from) % 8];
        let place = bar_series.iter().position(|b| *b == k).unwrap_or(0);
        let frame = Frame {
            baseline: baseline(data.series[k].right),
            bar_width,
            bar_offset: (place as f32 - (bar_series.len().max(1) - 1) as f32 / 2.0) * bar_width,
        };
        let look = Look {
            colour,
            width: (1.6 * zoom).max(1.0),
            radius: (3.0 * zoom).max(2.0),
        };
        marks::draw(&clipped, style.kind, style.marker, line, look, &frame);
    }

    if text_ok {
        painter.text(
            Pos2::new(area.center().x, rect.max.y - size(4.0)),
            Align2::CENTER_BOTTOM,
            caption(&spec.x_label, &scales.x_unit),
            font.clone(),
            theme.text_weak,
        );
        painter.text(
            Pos2::new(rect.min.x + pad, area.min.y - size(3.0)),
            Align2::LEFT_BOTTOM,
            caption(&spec.y_label, &scales.y_unit),
            font.clone(),
            theme.text_weak,
        );
        if has_right {
            painter.text(
                Pos2::new(rect.max.x - pad, area.min.y - size(3.0)),
                Align2::RIGHT_BOTTOM,
                caption(&spec.options.y2_label, &scales.y2_unit),
                font.clone(),
                theme.text_weak,
            );
        }
        let wanted = match spec.options.legend {
            LegendMode::Hide => false,
            LegendMode::Show => !data.series.is_empty(),
            LegendMode::Auto => data.series.len() >= 2,
        };
        if wanted {
            let entries: Vec<Entry> = styles
                .iter()
                .enumerate()
                .map(|(k, style)| {
                    let at = used.get(k).copied().unwrap_or(k);
                    let base = series_name(spec, at, style);
                    Entry {
                        name: if has_right && data.series[k].right {
                            format!("{base} (right)")
                        } else {
                            base
                        },
                        style,
                        colour: ctx.paper.series[style.colour.map_or(k, usize::from) % 8],
                    }
                })
                .collect();
            legend::draw(painter, ctx, &entries, area, &font, zoom);
        }
    }
    let values = data
        .series
        .iter()
        .map(|s| {
            let (_, y_scale) = scales.vertical(s.right);
            s.points
                .iter()
                .map(|p| (p.0 / scales.x_scale, p.1 / y_scale))
                .collect()
        })
        .collect();
    let names = styles
        .iter()
        .enumerate()
        .map(|(k, style)| series_name(spec, used.get(k).copied().unwrap_or(k), style))
        .collect();
    Some(Geometry {
        area,
        scales,
        screen,
        values,
        names,
        right: data.series.iter().map(|s| s.right).collect(),
    })
}
