//! The axes of a plot: what range each covers and in which unit it is shown,
//! worked out from the data, the series styles and the limits the user gave.

use opencalc_core::document::{SeriesKind, SeriesStyle};
use opencalc_core::eval::PlotData;
use opencalc_core::units::Dimension;

use super::marks;
use super::ticks::Axis;
use crate::canvas::Env;
use crate::format::axis_unit;

/// The x axis, the left y axis and, when a series is on it, the right y axis, each
/// with the unit its numbers are written in and the SI value of one such unit.
pub struct Scales {
    pub x: Axis,
    pub y: Axis,
    pub y2: Option<Axis>,
    pub x_unit: String,
    pub y_unit: String,
    pub y2_unit: String,
    pub x_scale: f64,
    pub y_scale: f64,
    pub y2_scale: f64,
}

fn extent(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    })
}

impl Scales {
    /// `styles[k]` is the style of `data.series[k]`.
    pub fn new(env: &Env, data: &PlotData, styles: &[SeriesStyle]) -> Scales {
        let points = || data.series.iter().flat_map(|s| s.points.iter());
        let x_peak = points().fold(0.0_f64, |m, p| m.max(p.0.abs()));
        let (x_unit, x_scale) = axis_unit(env.units, data.x_dim, x_peak);

        let kind_of = |k: usize| styles[k].kind;
        let has_bars = (0..styles.len()).any(|k| kind_of(k) == SeriesKind::Bar);
        // A marker or the head of a stem at the first or last x would be half cut off
        // by the edge of the area, so those kinds get a little room; bars get room
        // for half a column beyond the first and last.
        let has_markers = (0..styles.len()).any(|k| {
            matches!(
                kind_of(k),
                SeriesKind::Markers | SeriesKind::LineMarkers | SeriesKind::Stem
            )
        });
        let (mut x_lo, mut x_hi) = extent(points().map(|p| p.0 / x_scale));
        if has_bars {
            let gap = marks::min_gap(points().map(|p| p.0 / x_scale));
            let half = gap.unwrap_or_else(|| x_lo.abs().max(1.0) * 0.5) / 2.0;
            x_lo -= half;
            x_hi += half;
        }
        let limit = |i: usize, scale: f64| data.limits[i].map(|v| v / scale);
        let x = Axis::new(
            x_lo,
            x_hi,
            if has_markers && !has_bars { 0.03 } else { 0.0 },
            data.x_log,
            limit(0, x_scale),
            limit(1, x_scale),
        );

        // One vertical axis: the series on it, in a unit that reads well.
        let vertical = |right: bool, dim: Dimension, log: bool, lo: usize| {
            let on = || data.series.iter().filter(move |s| s.right == right);
            let peak = on()
                .flat_map(|s| s.points.iter())
                .fold(0.0_f64, |m, p| m.max(p.1.abs()));
            let (unit, scale) = axis_unit(env.units, dim, peak);
            let (mut y_lo, mut y_hi) =
                extent(on().flat_map(|s| s.points.iter()).map(|p| p.1 / scale));
            // Stems and bars grow from zero, so zero is kept in view.
            let grows = data.series.iter().zip(styles).any(|(s, st)| {
                s.right == right && matches!(st.kind, SeriesKind::Bar | SeriesKind::Stem)
            });
            if grows && !log {
                y_lo = y_lo.min(0.0);
                y_hi = y_hi.max(0.0);
            }
            let axis = Axis::new(
                y_lo,
                y_hi,
                0.05,
                log,
                limit(lo, scale),
                limit(lo + 1, scale),
            );
            (axis, unit, scale)
        };
        let (y, y_unit, y_scale) = vertical(false, data.left_dim(), data.y_log, 2);
        let (y2, y2_unit, y2_scale) = match data.y2_dim {
            Some(dim) => {
                let (axis, unit, scale) = vertical(true, dim, data.y2_log, 4);
                (Some(axis), unit, scale)
            }
            None => (None, String::new(), 1.0),
        };
        Scales {
            x,
            y,
            y2,
            x_unit,
            y_unit,
            y2_unit,
            x_scale,
            y_scale,
            y2_scale,
        }
    }

    /// The vertical axis a series is on, and the SI value of its unit.
    pub fn vertical(&self, right: bool) -> (&Axis, f64) {
        match (&self.y2, right) {
            (Some(axis), true) => (axis, self.y2_scale),
            _ => (&self.y, self.y_scale),
        }
    }
}
