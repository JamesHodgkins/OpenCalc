//! Where a painted plot ended up on screen, and what to do with that: find the
//! point under the pointer, write down what it is, and turn a drag or a scroll into
//! a new view of the axes.
//!
//! The view (which part of the data is shown) is a way of looking at the plot, not
//! part of the document, so it is never saved and has no undo.

use eframe::egui::{Pos2, Rect, Vec2};

use super::scales::Scales;

/// The ranges of the axes the user has chosen to look at, in SI values; `None`
/// leaves an axis to the plot's own limits or to the data.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    pub x: Option<(f64, f64)>,
    pub y: Option<(f64, f64)>,
    /// The right-hand axis.
    pub y2: Option<(f64, f64)>,
}

/// A data point near the pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub series: usize,
    pub index: usize,
    /// Where the point is on screen.
    pub at: Pos2,
    /// Its values in the units the axes are written in.
    pub x: f64,
    pub y: f64,
}

/// A painted plot: the plotting area, the axes with their units, and each series'
/// points in screen positions and in the axes' units.
#[derive(Clone)]
pub struct Geometry {
    pub area: Rect,
    pub scales: Scales,
    /// Screen position of every point, by series.
    pub screen: Vec<Vec<Pos2>>,
    /// The values behind them, in the axes' units, by series.
    pub values: Vec<Vec<(f64, f64)>>,
    /// Display name of each series, and whether it is on the right axis.
    pub names: Vec<String>,
    pub right: Vec<bool>,
}

/// `v` to five significant digits without trailing zeros: `3.1416`, `120`,
/// `0.00012346`, `1.2346e-7`.
pub fn significant(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return if v == 0.0 {
            "0".to_owned()
        } else {
            v.to_string()
        };
    }
    let magnitude = v.abs().log10().floor() as i32;
    let trim = |s: String| {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        }
    };
    if (-4..9).contains(&magnitude) {
        let decimals = (4 - magnitude).max(0) as usize;
        trim(format!("{v:.decimals$}"))
    } else {
        let text = format!("{v:.4e}");
        match text.split_once('e') {
            Some((mantissa, exponent)) => format!("{}e{exponent}", trim(mantissa.to_owned())),
            None => text,
        }
    }
}

fn with_unit(value: f64, unit: &str) -> String {
    if unit.is_empty() {
        significant(value)
    } else {
        format!("{} {unit}", significant(value))
    }
}

impl Geometry {
    pub fn contains(&self, p: Pos2) -> bool {
        self.area.contains(p)
    }

    /// Where `p` is in the plotting area as fractions of its width (from the left)
    /// and height (from the bottom).
    pub fn fractions(&self, p: Pos2) -> (f64, f64) {
        (
            f64::from((p.x - self.area.min.x) / self.area.width()),
            f64::from((self.area.max.y - p.y) / self.area.height()),
        )
    }

    /// The values under `p`: x, the left y and, if there is one, the right y.
    pub fn cursor(&self, p: Pos2) -> (f64, f64, Option<f64>) {
        let (fx, fy) = self.fractions(p);
        (
            self.scales.x.value_at(fx),
            self.scales.y.value_at(fy),
            self.scales.y2.as_ref().map(|a| a.value_at(fy)),
        )
    }

    /// The point nearest to `p` within `reach` pixels, if any.
    pub fn nearest(&self, p: Pos2, reach: f32) -> Option<Hit> {
        let mut best: Option<(f32, Hit)> = None;
        for (series, line) in self.screen.iter().enumerate() {
            for (index, at) in line.iter().enumerate() {
                let d = at.distance(p);
                if d <= reach && best.as_ref().is_none_or(|(b, _)| d < *b) {
                    let (x, y) = self.values[series][index];
                    best = Some((
                        d,
                        Hit {
                            series,
                            index,
                            at: *at,
                            x,
                            y,
                        },
                    ));
                }
            }
        }
        best.map(|(_, hit)| hit)
    }

    /// What a hit is, in words: `load (right): x = 3 s, y = 12.5 N`.
    pub fn describe(&self, hit: &Hit) -> String {
        let y_unit = if self.right[hit.series] {
            &self.scales.y2_unit
        } else {
            &self.scales.y_unit
        };
        format!(
            "{}: x = {}, y = {}",
            self.names[hit.series],
            with_unit(hit.x, &self.scales.x_unit),
            with_unit(hit.y, y_unit),
        )
    }

    /// The values under the pointer, for a status line.
    pub fn describe_cursor(&self, p: Pos2) -> String {
        let (x, y, y2) = self.cursor(p);
        let mut text = format!(
            "x = {}, y = {}",
            with_unit(x, &self.scales.x_unit),
            with_unit(y, &self.scales.y_unit)
        );
        if let Some(y2) = y2 {
            text.push_str(&format!(
                ", right = {}",
                with_unit(y2, &self.scales.y2_unit)
            ));
        }
        text
    }
}

impl View {
    /// Whether the user has moved from the plot's own view.
    pub fn is_moved(&self) -> bool {
        *self != View::default()
    }

    /// Zoom by `factor` (below 1 zooms in) about the screen position `at`, on all
    /// axes: the data under the pointer stays under it.
    pub fn zoomed(&self, g: &Geometry, at: Pos2, factor: f64) -> View {
        let (fx, fy) = g.fractions(at);
        let s = &g.scales;
        let to_si = |(lo, hi): (f64, f64), scale: f64| Some((lo * scale, hi * scale));
        View {
            x: to_si(s.x.zoomed(fx, factor), s.x_scale),
            y: to_si(s.y.zoomed(fy, factor), s.y_scale),
            y2: s
                .y2
                .as_ref()
                .and_then(|a| to_si(a.zoomed(fy, factor), s.y2_scale)),
        }
    }

    /// Move the view with a drag of `delta` screen pixels: the data follows the
    /// pointer.
    pub fn panned(&self, g: &Geometry, delta: Vec2) -> View {
        let (dx, dy) = (
            -f64::from(delta.x / g.area.width()),
            f64::from(delta.y / g.area.height()),
        );
        let s = &g.scales;
        let to_si = |(lo, hi): (f64, f64), scale: f64| Some((lo * scale, hi * scale));
        View {
            x: to_si(s.x.panned(dx), s.x_scale),
            y: to_si(s.y.panned(dy), s.y_scale),
            y2: s.y2.as_ref().and_then(|a| to_si(a.panned(dy), s.y2_scale)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::plot::ticks::Axis;

    fn geometry() -> Geometry {
        let scales = Scales {
            x: Axis::new(0.0, 10.0, 0.0, false, None, None),
            y: Axis::new(0.0, 100.0, 0.0, false, None, None),
            y2: Some(Axis::new(0.0, 5.0, 0.0, false, None, None)),
            x_unit: "s".into(),
            y_unit: "m".into(),
            y2_unit: "N".into(),
            x_scale: 1.0,
            y_scale: 0.001,
            y2_scale: 1.0,
        };
        let area = Rect::from_min_max(Pos2::new(100.0, 50.0), Pos2::new(500.0, 250.0));
        let at = |x: f32, y: f32| Pos2::new(100.0 + x * 40.0, 250.0 - y * 2.0);
        Geometry {
            area,
            scales,
            screen: vec![
                vec![at(0.0, 0.0), at(5.0, 50.0), at(10.0, 100.0)],
                vec![at(5.0, 40.0)],
            ],
            values: vec![
                vec![(0.0, 0.0), (5.0, 50.0), (10.0, 100.0)],
                vec![(5.0, 2.0)],
            ],
            names: vec!["d".into(), "load".into()],
            right: vec![false, true],
        }
    }

    #[test]
    fn five_significant_digits_without_trailing_zeros() {
        assert_eq!(significant(1.23456789), "1.2346");
        assert_eq!(significant(120.0), "120");
        assert_eq!(significant(0.5), "0.5");
        assert_eq!(significant(-12.5), "-12.5");
        assert_eq!(significant(0.000123456), "0.00012346");
        assert_eq!(significant(1234567.0), "1234567");
        assert_eq!(significant(1.23456789e-7), "1.2346e-7");
        assert_eq!(significant(2.5e12), "2.5e12");
        assert_eq!(significant(0.0), "0");
        assert_eq!(significant(f64::NAN), "NaN");
        assert_eq!(significant(100000.0), "100000");
    }

    #[test]
    fn the_pointer_position_maps_to_values_on_every_axis() {
        let g = geometry();
        let (x, y, y2) = g.cursor(Pos2::new(300.0, 150.0));
        assert!((x - 5.0).abs() < 1e-9 && (y - 50.0).abs() < 1e-9);
        assert!((y2.unwrap() - 2.5).abs() < 1e-9);
        assert_eq!(g.fractions(Pos2::new(100.0, 250.0)), (0.0, 0.0));
        assert_eq!(g.fractions(Pos2::new(500.0, 50.0)), (1.0, 1.0));
        assert!(g.contains(Pos2::new(300.0, 150.0)) && !g.contains(Pos2::new(99.0, 150.0)));
    }

    #[test]
    fn the_nearest_point_within_reach_is_found_and_nothing_beyond_it() {
        let g = geometry();
        let hit = g.nearest(Pos2::new(302.0, 152.0), 12.0).unwrap();
        assert_eq!(
            (hit.series, hit.index),
            (0, 1),
            "the point on series 0, at (5, 50)"
        );
        assert_eq!((hit.x, hit.y), (5.0, 50.0));
        // A second series' point at (300, 170) is nearer this pointer.
        let hit = g.nearest(Pos2::new(300.0, 168.0), 12.0).unwrap();
        assert_eq!(hit.series, 1);
        assert!(g.nearest(Pos2::new(400.0, 60.0), 12.0).is_none());
        assert!(
            g.nearest(Pos2::new(302.0, 152.0), 0.5).is_none(),
            "outside a tiny reach"
        );
    }

    #[test]
    fn a_hit_is_described_with_its_series_and_the_unit_of_its_axis() {
        let g = geometry();
        let left = g.nearest(Pos2::new(300.0, 150.0), 12.0).unwrap();
        assert_eq!(g.describe(&left), "d: x = 5 s, y = 50 m");
        let right = g.nearest(Pos2::new(300.0, 168.0), 12.0).unwrap();
        assert_eq!(g.describe(&right), "load: x = 5 s, y = 2 N");
        assert_eq!(
            g.describe_cursor(Pos2::new(300.0, 150.0)),
            "x = 5 s, y = 50 m, right = 2.5 N"
        );
    }

    #[test]
    fn zooming_stores_si_ranges_and_keeps_the_point_under_the_pointer() {
        let g = geometry();
        let at = Pos2::new(300.0, 150.0);
        let view = View::default().zoomed(&g, at, 0.5);
        let (x_lo, x_hi) = view.x.unwrap();
        assert!((x_hi - x_lo - 5.0).abs() < 1e-9, "half the x range");
        assert!(
            (x_lo - 2.5).abs() < 1e-9,
            "centred on x = 5 as the pointer is"
        );
        // y is shown in mm, so the SI range is a thousandth of the displayed one.
        let (y_lo, y_hi) = view.y.unwrap();
        assert!((y_hi - y_lo - 0.05).abs() < 1e-12, "{y_lo}..{y_hi}");
        let (z_lo, z_hi) = view.y2.unwrap();
        assert!(
            (z_hi - z_lo - 2.5).abs() < 1e-9,
            "the right axis zooms with it"
        );
    }

    #[test]
    fn dragging_moves_the_data_with_the_pointer() {
        let g = geometry();
        // Drag 40 pixels right (a tenth of the width): the data moves right, so the
        // view moves left by a tenth of its range.
        let view = View::default().panned(&g, Vec2::new(40.0, 0.0));
        let (lo, hi) = view.x.unwrap();
        assert!(
            (lo + 1.0).abs() < 1e-6 && (hi - 9.0).abs() < 1e-6,
            "{lo}..{hi}"
        );
        // Drag 20 pixels down (a tenth of the height): the view moves up.
        let view = View::default().panned(&g, Vec2::new(0.0, 20.0));
        let (lo, _) = view.y.unwrap();
        assert!((lo - 0.01).abs() < 1e-9, "{lo}");
    }

    #[test]
    fn a_view_that_has_not_moved_is_the_default() {
        assert!(!View::default().is_moved());
        let g = geometry();
        assert!(View::default()
            .zoomed(&g, Pos2::new(300.0, 150.0), 0.9)
            .is_moved());
    }
}
