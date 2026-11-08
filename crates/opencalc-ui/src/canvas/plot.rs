//! Painting an X-Y plot region on the canvas: frame, grid, axes, ticks, labels,
//! the data series and the legend. Drawn with the painter like every other region,
//! so it scales with the canvas zoom and takes no input of its own (dragging a plot
//! moves the region, as for any other).

mod marks;
mod ticks;

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use opencalc_core::document::{LegendMode, PlotSpec, Region, SeriesKind, SeriesStyle};
use opencalc_core::eval::PlotData;

use super::adapt::vec;
use super::Env;
use crate::format::axis_unit;
use crate::theme::{metrics, type_size};
use marks::{Frame, Look};
use ticks::Axis;

/// Below this on-screen text size, text is left out rather than drawn as noise.
const MIN_TEXT: f32 = 5.0;

fn data_extent(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    })
}

/// The positions in `spec.series` of the series that have data. A blank series
/// is not evaluated, so the k-th series drawn is `used_series(spec)[k]` of the spec.
fn used_series(spec: &PlotSpec) -> Vec<usize> {
    spec.series
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.source().trim().is_empty())
        .map(|(i, _)| i)
        .collect()
}

pub fn paint(painter: &Painter, env: &Env, r: &Region, spec: &PlotSpec, top_left: Pos2, zoom: f32) {
    let theme = env.paper;
    let rect = Rect::from_min_size(top_left, vec(r.size) * zoom);
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
    match env.output.plots.get(&r.id) {
        None => note("\u{2026}", theme.text_weak),
        Some(Err(e)) => note(&format!("{} {e}", crate::icons::WARNING), theme.error),
        Some(Ok(data)) if data.is_empty() => note(
            "Choose the x values and the series to plot in the Plot window.",
            theme.text_weak,
        ),
        Some(Ok(data)) => chart(painter, env, spec, data, rect, top, zoom),
    }
}

fn chart(
    painter: &Painter,
    env: &Env,
    spec: &PlotSpec,
    data: &PlotData,
    rect: Rect,
    top: f32,
    zoom: f32,
) {
    let theme = env.paper;
    let size = |base: f32| base * zoom;
    let text_ok = size(type_size::SMALL) >= MIN_TEXT;
    let font = FontId::proportional(size(type_size::SMALL));
    let pad = size(8.0);
    let used = used_series(spec);
    let styles: Vec<_> = (0..data.series.len())
        .map(|k| spec.style(used.get(k).copied().unwrap_or(k)))
        .collect();

    // Axis units: shown in a readable unit of the data's dimension.
    let points = || data.series.iter().flat_map(|s| s.points.iter());
    let x_peak = points().fold(0.0_f64, |m, p| m.max(p.0.abs()));
    let y_peak = points().fold(0.0_f64, |m, p| m.max(p.1.abs()));
    let y_dim = data.series.first().map_or(data.x_dim, |s| s.y_dim);
    let (x_unit, x_scale) = axis_unit(env.units, data.x_dim, x_peak);
    let (y_unit, y_scale) = axis_unit(env.units, y_dim, y_peak);

    // Stems and bars grow from zero, so zero is kept in view; bars also need room
    // beside the first and last.
    let grows_from_zero = styles
        .iter()
        .any(|s| matches!(s.kind, SeriesKind::Bar | SeriesKind::Stem));
    let has_bars = styles.iter().any(|s| s.kind == SeriesKind::Bar);
    // A marker or the head of a stem at the first or last x would be half cut off by
    // the edge of the area, so those kinds get a little room.
    let has_markers = styles.iter().any(|s| {
        matches!(
            s.kind,
            SeriesKind::Markers | SeriesKind::LineMarkers | SeriesKind::Stem
        )
    });
    let (mut x_lo, mut x_hi) = data_extent(points().map(|p| p.0 / x_scale));
    if has_bars {
        // Room for half a bar beyond the first and last, so neither is cut off.
        let gap = marks::min_gap(
            data.series
                .iter()
                .flat_map(|s| s.points.iter().map(|p| p.0 / x_scale)),
        );
        let half = gap.unwrap_or_else(|| x_lo.abs().max(1.0) * 0.5) / 2.0;
        x_lo -= half;
        x_hi += half;
    }
    let (mut y_lo, mut y_hi) = data_extent(points().map(|p| p.1 / y_scale));
    if grows_from_zero && !data.y_log {
        y_lo = y_lo.min(0.0);
        y_hi = y_hi.max(0.0);
    }
    let limit = |i: usize, scale: f64| data.limits[i].map(|v| v / scale);
    let x_axis = Axis::new(
        x_lo,
        x_hi,
        if has_markers && !has_bars { 0.03 } else { 0.0 },
        data.x_log,
        limit(0, x_scale),
        limit(1, x_scale),
    );
    let y_axis = Axis::new(
        y_lo,
        y_hi,
        0.05,
        data.y_log,
        limit(2, y_scale),
        limit(3, y_scale),
    );

    // The plotting area leaves room for tick labels and the axis captions.
    let left = if text_ok { size(46.0) } else { pad };
    let bottom = if text_ok { size(34.0) } else { pad };
    let heading = if text_ok { size(16.0) } else { 0.0 };
    let area = Rect::from_min_max(
        Pos2::new(rect.min.x + left, top + heading),
        Pos2::new(rect.max.x - pad, rect.max.y - bottom),
    );
    if area.width() < 4.0 || area.height() < 4.0 {
        return;
    }
    let to_screen = |x: f64, y: f64| {
        Pos2::new(
            area.min.x + x_axis.fraction(x) as f32 * area.width(),
            area.max.y - y_axis.fraction(y) as f32 * area.height(),
        )
    };

    let x_ticks = x_axis.ticks((area.width() / size(48.0)).max(2.0) as usize);
    let y_ticks = y_axis.ticks((area.height() / size(36.0)).max(2.0) as usize);
    let grid = Stroke::new(1.0_f32, theme.raised);
    let tick = Stroke::new(1.0_f32, theme.border_strong);
    let tick_len = size(4.0);
    for &t in &x_ticks.values {
        let at = to_screen(t, y_axis.lo);
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
        let at = to_screen(x_axis.lo, t);
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
            s.points
                .iter()
                .map(|p| to_screen(p.0 / x_scale, p.1 / y_scale))
                .collect()
        })
        .collect();
    let baseline_value = if data.y_log {
        y_axis.lo
    } else {
        0.0_f64.clamp(y_axis.lo, y_axis.hi)
    };
    let baseline = to_screen(x_axis.lo, baseline_value).y;
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
        let colour = env.paper.series[style.colour.map_or(k, usize::from) % 8];
        let place = bar_series.iter().position(|b| *b == k).unwrap_or(0);
        let frame = Frame {
            baseline,
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
        let caption = |user: &str, unit: &str| match (user.is_empty(), unit.is_empty()) {
            (true, true) => String::new(),
            (true, false) => unit.to_owned(),
            (false, true) => user.to_owned(),
            (false, false) => format!("{user} [{unit}]"),
        };
        painter.text(
            Pos2::new(area.center().x, rect.max.y - size(4.0)),
            Align2::CENTER_BOTTOM,
            caption(&spec.x_label, &x_unit),
            font.clone(),
            theme.text_weak,
        );
        painter.text(
            Pos2::new(rect.min.x + pad, area.min.y - size(3.0)),
            Align2::LEFT_BOTTOM,
            caption(&spec.y_label, &y_unit),
            font.clone(),
            theme.text_weak,
        );
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
                    Entry {
                        name: if style.name.trim().is_empty() {
                            spec.series.get(at).map_or("", |s| s.source())
                        } else {
                            style.name.as_str()
                        },
                        style,
                        colour: env.paper.series[style.colour.map_or(k, usize::from) % 8],
                    }
                })
                .collect();
            legend(painter, env, &entries, area, &font, zoom);
        }
    }
}

/// One legend row: a series' name, how it is drawn and in what colour.
struct Entry<'a> {
    name: &'a str,
    style: &'a SeriesStyle,
    colour: Color32,
}

/// Which series is which: shown when asked for, or by default when there is more
/// than one. The name is the series' own, or its expression.
fn legend(painter: &Painter, env: &Env, entries: &[Entry], area: Rect, font: &FontId, zoom: f32) {
    let row = font.size * 1.4;
    let x_right = area.max.x - 6.0 * zoom;
    // Lay the names out first so a backing panel can sit behind all of them: the
    // legend usually lands on top of the data.
    let texts: Vec<_> = entries
        .iter()
        .map(|e| painter.layout_no_wrap(e.name.to_owned(), font.clone(), env.paper.text))
        .collect();
    let widest = texts.iter().map(|g| g.size().x).fold(0.0_f32, f32::max);
    let top = area.min.y + 4.0 * zoom;
    let panel = Rect::from_min_max(
        Pos2::new(x_right - widest - 24.0 * zoom, top - 2.0 * zoom),
        Pos2::new(x_right + 3.0 * zoom, top + row * entries.len() as f32),
    );
    painter.rect_filled(panel, 3.0 * zoom, env.paper.surface.gamma_multiply(0.85));
    let mut y = top;
    for (entry, text) in entries.iter().zip(texts) {
        let x_text = x_right - text.size().x;
        painter.galley(Pos2::new(x_text, y), text, env.paper.text);
        let swatch = Rect::from_min_size(
            Pos2::new(x_text - 20.0 * zoom, y + row * 0.35),
            Vec2::new(14.0 * zoom, (2.0 * zoom).max(1.5)),
        );
        let radius = (3.0 * zoom).max(2.0);
        let (style, colour) = (entry.style, entry.colour);
        match style.kind {
            SeriesKind::Bar => {
                painter.rect_filled(swatch.expand2(Vec2::new(0.0, 3.0 * zoom)), 0.0, colour);
            }
            SeriesKind::Markers => {
                marks::marker(painter, style.marker, swatch.center(), radius, colour);
            }
            kind => {
                painter.rect_filled(swatch, 0.0, colour);
                if matches!(kind, SeriesKind::LineMarkers | SeriesKind::Stem) {
                    marks::marker(painter, style.marker, swatch.center(), radius, colour);
                }
            }
        }
        y += row;
    }
}
