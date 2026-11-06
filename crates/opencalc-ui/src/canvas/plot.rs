//! Painting an X-Y plot region on the canvas: frame, grid, axes, ticks, labels
//! and the data lines. Drawn with the painter like every other region, so it
//! scales with the canvas zoom and takes no input of its own (dragging a plot
//! moves the region, as for any other).

use eframe::egui::{
    epaint::PathStroke, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
    Vec2,
};
use opencalc_core::document::{PlotSpec, Region};
use opencalc_core::eval::PlotData;

use super::adapt::vec;
use super::Env;
use crate::format::axis_unit;
use crate::theme::{metrics, type_size};

/// Below this on-screen text size, text is left out rather than drawn as noise.
const MIN_TEXT: f32 = 5.0;

/// Tick positions for an axis from `min` to `max`: about `target` of them, at
/// multiples of 1, 2 or 5 times a power of ten, all inside the range.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> (Vec<f64>, f64) {
    let span = max - min;
    if !(span.is_finite() && span > 0.0) || target == 0 {
        return (Vec::new(), 0.0);
    }
    let rough = span / target as f64;
    let power = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| *s >= rough)
        .unwrap_or(10.0 * power);
    // A little slack, so an end that is itself a round number is not lost to
    // rounding (3e-9 / 5e-10 is 6.000000000000001).
    let first = (min / step - 1e-9).ceil() as i64;
    let last = (max / step + 1e-9).floor() as i64;
    let ticks = (first..=last).map(|k| k as f64 * step).collect();
    (ticks, step)
}

/// A tick value with just enough decimals for the spacing, and a clean zero.
pub fn tick_text(value: f64, step: f64) -> String {
    if value.abs() < step * 1e-9 {
        return "0".to_owned();
    }
    // The fewest decimals that show the step exactly (0.25 needs two).
    let decimals = (0..=8)
        .find(|d| {
            let scaled = step * 10f64.powi(*d);
            (scaled - scaled.round()).abs() < 1e-6 * scaled.abs().max(1.0)
        })
        .unwrap_or(8) as usize;
    format!("{value:.decimals$}")
}

/// The range an axis should cover: the data's, padded, and never empty.
pub fn axis_range(lo: f64, hi: f64, pad: f64) -> (f64, f64) {
    if hi > lo {
        let margin = (hi - lo) * pad;
        return (lo - margin, hi + margin);
    }
    let around = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
    (lo - around, lo + around)
}

fn data_extent(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    })
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
            "Choose the x values and the series to plot in the properties panel.",
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

    // Axis units: shown in a readable unit of the data's dimension.
    let points = || data.series.iter().flat_map(|s| s.points.iter());
    let x_peak = points().fold(0.0_f64, |m, p| m.max(p.0.abs()));
    let y_peak = points().fold(0.0_f64, |m, p| m.max(p.1.abs()));
    let y_dim = data.series.first().map_or(data.x_dim, |s| s.y_dim);
    let (x_unit, x_scale) = axis_unit(env.units, data.x_dim, x_peak);
    let (y_unit, y_scale) = axis_unit(env.units, y_dim, y_peak);

    let (x_lo, x_hi) = data_extent(points().map(|p| p.0 / x_scale));
    let (y_lo, y_hi) = data_extent(points().map(|p| p.1 / y_scale));
    let (x_lo, x_hi) = axis_range(x_lo, x_hi, 0.0);
    let (y_lo, y_hi) = axis_range(y_lo, y_hi, 0.05);

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
        let fx = ((x - x_lo) / (x_hi - x_lo)) as f32;
        let fy = ((y - y_lo) / (y_hi - y_lo)) as f32;
        Pos2::new(
            area.min.x + fx * area.width(),
            area.max.y - fy * area.height(),
        )
    };

    let (x_ticks, x_step) = nice_ticks(x_lo, x_hi, (area.width() / size(70.0)).max(2.0) as usize);
    let (y_ticks, y_step) = nice_ticks(y_lo, y_hi, (area.height() / size(36.0)).max(2.0) as usize);
    let grid = Stroke::new(1.0_f32, theme.raised);
    for &t in &x_ticks {
        let at = to_screen(t, y_lo);
        painter.vline(at.x, area.y_range(), grid);
        if text_ok {
            painter.text(
                Pos2::new(at.x, area.max.y + size(3.0)),
                Align2::CENTER_TOP,
                tick_text(t, x_step),
                font.clone(),
                theme.text_weak,
            );
        }
    }
    for &t in &y_ticks {
        let at = to_screen(x_lo, t);
        painter.hline(area.x_range(), at.y, grid);
        if text_ok {
            painter.text(
                Pos2::new(area.min.x - size(4.0), at.y),
                Align2::RIGHT_CENTER,
                tick_text(t, y_step),
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
    let width = (1.6 * zoom).max(1.0);
    for (k, series) in data.series.iter().enumerate() {
        let colour = env.paper.series[k % 8];
        let line: Vec<Pos2> = series
            .points
            .iter()
            .map(|p| to_screen(p.0 / x_scale, p.1 / y_scale))
            .collect();
        if line.len() == 1 {
            clipped.circle_filled(line[0], width * 1.8, colour);
        } else {
            clipped.add(Shape::line(line, PathStroke::new(width, colour)));
        }
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
        legend(painter, env, spec, data, area, &font, zoom);
    }
}

/// Which line is which, when there is more than one.
fn legend(
    painter: &Painter,
    env: &Env,
    spec: &PlotSpec,
    data: &PlotData,
    area: Rect,
    font: &FontId,
    zoom: f32,
) {
    if data.series.len() < 2 {
        return;
    }
    let names: Vec<&str> = spec
        .series
        .iter()
        .map(|s| s.source())
        .filter(|s| !s.trim().is_empty())
        .collect();
    let row = font.size * 1.4;
    let mut y = area.min.y + 4.0 * zoom;
    for (k, name) in names.iter().enumerate().take(data.series.len()) {
        let colour = env.paper.series[k % 8];
        let x_right = area.max.x - 6.0 * zoom;
        painter.text(
            Pos2::new(x_right, y),
            Align2::RIGHT_TOP,
            *name,
            font.clone(),
            env.paper.text,
        );
        let swatch = Rect::from_min_size(
            Pos2::new(x_right - 80.0 * zoom, y + row * 0.35),
            Vec2::new(14.0 * zoom, (2.0 * zoom).max(1.5)),
        );
        painter.rect_filled(swatch, 0.0, colour);
        y += row;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_are_round_numbers_inside_the_range() {
        let (ticks, step) = nice_ticks(0.0, 10.0, 5);
        assert_eq!(step, 2.0);
        assert_eq!(ticks, [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        let (ticks, step) = nice_ticks(-0.3, 0.7, 5);
        assert!((step - 0.2).abs() < 1e-12, "{step}");
        assert!(ticks.iter().all(|t| *t >= -0.3 - 1e-9 && *t <= 0.7 + 1e-9));
        assert!(ticks.contains(&0.0));
    }

    #[test]
    fn ticks_work_at_any_scale_and_never_run_away() {
        for (lo, hi) in [
            (1e-9, 3e-9),
            (1e9, 5e9),
            (-1e300, 1e300),
            (123.4, 123.9),
            (0.0, 1e-300),
        ] {
            let (ticks, step) = nice_ticks(lo, hi, 6);
            assert!(step > 0.0, "{lo}..{hi}");
            assert!(ticks.len() <= 20, "{lo}..{hi}: {} ticks", ticks.len());
            assert!(ticks.windows(2).all(|w| w[1] > w[0]));
            let slack = (hi - lo) * 1e-8;
            assert!(ticks.iter().all(|t| *t >= lo - slack && *t <= hi + slack));
        }
    }

    #[test]
    fn an_empty_or_invalid_range_has_no_ticks() {
        assert!(nice_ticks(1.0, 1.0, 5).0.is_empty());
        assert!(nice_ticks(2.0, 1.0, 5).0.is_empty());
        assert!(nice_ticks(0.0, f64::NAN, 5).0.is_empty());
        assert!(nice_ticks(0.0, 1.0, 0).0.is_empty());
    }

    #[test]
    fn tick_labels_have_just_enough_decimals() {
        assert_eq!(tick_text(4.0, 2.0), "4");
        assert_eq!(tick_text(0.25, 0.25), "0.25");
        assert_eq!(tick_text(0.5, 0.1), "0.5");
        assert_eq!(tick_text(1200.0, 200.0), "1200");
        // Rounding noise around zero reads as zero.
        assert_eq!(tick_text(1e-17, 0.5), "0");
    }

    #[test]
    fn a_flat_series_still_gets_a_range_to_draw_in() {
        let (lo, hi) = axis_range(5.0, 5.0, 0.05);
        assert!(lo < 5.0 && hi > 5.0);
        let (lo, hi) = axis_range(0.0, 0.0, 0.05);
        assert!(lo < 0.0 && hi > 0.0);
        let (lo, hi) = axis_range(0.0, 10.0, 0.1);
        assert_eq!((lo, hi), (-1.0, 11.0));
    }
}
