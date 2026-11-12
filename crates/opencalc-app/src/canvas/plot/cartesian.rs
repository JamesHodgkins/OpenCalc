//! The X-Y chart: plotting area, tick marks and labels, the data series, the axis
//! captions and the legend. (A polar chart is drawn by `polar`.)

use eframe::egui::{Align2, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};
use opencalc_core::document::{PlotSpec, SeriesKind, SeriesStyle};
use opencalc_core::eval::PlotData;

use super::context::{caption, series_name, used_series, MIN_TEXT};
use super::marks::{self, Frame, Look};
use super::scales::Scales;
use super::{legend, polar, Chart, Geometry, PlotCtx};
use crate::theme::{type_size, Theme};

/// Draw the chart and give back where things went, or `None` when the plotting area
/// would be too small to see.
pub(super) fn chart(painter: &Painter, ctx: &PlotCtx, c: &Chart) -> Option<Geometry> {
    let Chart {
        spec,
        data,
        rect,
        top,
        zoom,
        view,
    } = *c;
    let size = |base: f32| base * zoom;
    let text_ok = size(type_size::SMALL) >= MIN_TEXT;
    let used = used_series(spec);
    let styles: Vec<SeriesStyle> = (0..data.series.len())
        .map(|k| spec.style(used.get(k).copied().unwrap_or(k)))
        .collect();
    if data.polar {
        return polar::draw(painter, ctx, c, &styles, &used);
    }
    let scales = Scales::new(ctx, data, &styles, view);
    let has_right = scales.y2.is_some();
    let area = plotting_area(rect, top, zoom, text_ok, has_right)?;
    let plane = Plane {
        painter,
        theme: ctx.paper,
        scales: &scales,
        area,
        zoom,
        text_ok,
        font: FontId::proportional(size(type_size::SMALL)),
    };
    plane.ticks(spec.options.grid);
    painter.rect_stroke(
        area,
        0.0,
        Stroke::new(1.0_f32, ctx.paper.border_strong),
        StrokeKind::Inside,
    );
    let screen = plane.series(ctx, data, &styles);
    if text_ok {
        plane.captions(rect, spec);
        if let Some(entries) = legend::entries(ctx, spec, data, &styles, &used, has_right) {
            legend::draw(painter, ctx, &entries, area, &plane.font, zoom);
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
        polar: None,
    })
}

/// The plotting area: the chart's box less room for tick labels, the axis captions
/// and the heading. `None` when too little is left.
fn plotting_area(rect: Rect, top: f32, zoom: f32, text_ok: bool, has_right: bool) -> Option<Rect> {
    let size = |base: f32| base * zoom;
    let pad = size(8.0);
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
    (area.width() >= 4.0 && area.height() >= 4.0).then_some(area)
}

/// What the drawing steps share: where the plotting area is and how it is scaled.
struct Plane<'a> {
    painter: &'a Painter,
    theme: &'a Theme,
    scales: &'a Scales,
    area: Rect,
    zoom: f32,
    text_ok: bool,
    font: FontId,
}

impl Plane<'_> {
    fn size(&self, base: f32) -> f32 {
        base * self.zoom
    }

    /// A position from values already in the axes' units, on the left or right axis.
    fn to_screen(&self, x: f64, y: f64, on_right: bool) -> Pos2 {
        let (axis, _) = self.scales.vertical(on_right);
        Pos2::new(
            self.area.min.x + self.scales.x.fraction(x) as f32 * self.area.width(),
            self.area.max.y - axis.fraction(y) as f32 * self.area.height(),
        )
    }

    fn label(&self, at: Pos2, align: Align2, text: String) {
        self.painter
            .text(at, align, text, self.font.clone(), self.theme.text_weak);
    }

    /// Tick marks (or grid lines) and tick labels on the bottom, left and, when
    /// there is one, right axis.
    fn ticks(&self, grid_lines: bool) {
        let (area, scales) = (self.area, self.scales);
        let x_ticks = scales
            .x
            .ticks((area.width() / self.size(48.0)).max(2.0) as usize);
        let y_target = (area.height() / self.size(36.0)).max(2.0) as usize;
        let y_ticks = scales.y.ticks(y_target);
        let grid = Stroke::new(1.0_f32, self.theme.raised);
        let tick = Stroke::new(1.0_f32, self.theme.border_strong);
        let tick_len = self.size(4.0);
        for &t in &x_ticks.values {
            let at = self.to_screen(t, scales.y.lo, false);
            if grid_lines {
                self.painter.vline(at.x, area.y_range(), grid);
            } else {
                self.painter
                    .vline(at.x, (area.max.y - tick_len)..=area.max.y, tick);
            }
            if self.text_ok {
                let below = Pos2::new(at.x, area.max.y + self.size(3.0));
                self.label(below, Align2::CENTER_TOP, x_ticks.label(t));
            }
        }
        for &t in &y_ticks.values {
            let at = self.to_screen(scales.x.lo, t, false);
            if grid_lines {
                self.painter.hline(area.x_range(), at.y, grid);
            } else {
                self.painter
                    .hline(area.min.x..=(area.min.x + tick_len), at.y, tick);
            }
            if self.text_ok {
                let beside = Pos2::new(area.min.x - self.size(4.0), at.y);
                self.label(beside, Align2::RIGHT_CENTER, y_ticks.label(t));
            }
        }
        // The right axis has its own ticks and labels but no grid lines of its own:
        // two sets of lines would not agree.
        if let Some(axis) = &scales.y2 {
            let ticks = axis.ticks(y_target);
            for &t in &ticks.values {
                let at = self.to_screen(scales.x.lo, t, true);
                self.painter
                    .hline((area.max.x - tick_len)..=area.max.x, at.y, tick);
                if self.text_ok {
                    let beside = Pos2::new(area.max.x + self.size(4.0), at.y);
                    self.label(beside, Align2::LEFT_CENTER, ticks.label(t));
                }
            }
        }
    }

    /// Draw the series, clipped to the plotting area. Gives back the screen position
    /// of every point, by series.
    fn series(&self, ctx: &PlotCtx, data: &PlotData, styles: &[SeriesStyle]) -> Vec<Vec<Pos2>> {
        let scales = self.scales;
        let clipped = self.painter.with_clip_rect(self.area);
        let screen: Vec<Vec<Pos2>> = data
            .series
            .iter()
            .map(|s| {
                let (_, y_scale) = scales.vertical(s.right);
                s.points
                    .iter()
                    .map(|p| self.to_screen(p.0 / scales.x_scale, p.1 / y_scale, s.right))
                    .collect()
            })
            .collect();
        // Stems and bars grow from zero (or the bottom of a log axis), on their own
        // axis.
        let baseline = |on_right: bool| {
            let (axis, _) = scales.vertical(on_right);
            let value = if axis.log {
                axis.lo
            } else {
                0.0_f64.clamp(axis.lo, axis.hi)
            };
            self.to_screen(scales.x.lo, value, on_right).y
        };
        let bar_series: Vec<usize> = (0..styles.len())
            .filter(|k| styles[*k].kind == SeriesKind::Bar)
            .collect();
        let slot = marks::bar_slot(
            bar_series
                .iter()
                .flat_map(|k| screen[*k].iter().map(|p| p.x)),
            self.size(24.0),
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
                width: (1.6 * self.zoom).max(1.0),
                radius: (3.0 * self.zoom).max(2.0),
            };
            marks::draw(&clipped, style.kind, style.marker, line, look, &frame);
        }
        screen
    }

    /// The axis captions: x under the chart, y (and y2) above their axes.
    fn captions(&self, rect: Rect, spec: &PlotSpec) {
        let (area, scales) = (self.area, self.scales);
        let pad = self.size(8.0);
        self.label(
            Pos2::new(area.center().x, rect.max.y - self.size(4.0)),
            Align2::CENTER_BOTTOM,
            caption(&spec.x_label, &scales.x_unit),
        );
        self.label(
            Pos2::new(rect.min.x + pad, area.min.y - self.size(3.0)),
            Align2::LEFT_BOTTOM,
            caption(&spec.y_label, &scales.y_unit),
        );
        if scales.y2.is_some() {
            self.label(
                Pos2::new(rect.max.x - pad, area.min.y - self.size(3.0)),
                Align2::RIGHT_BOTTOM,
                caption(&spec.options.y2_label, &scales.y2_unit),
            );
        }
    }
}
