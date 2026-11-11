//! Painting a polar plot: rings for the radius, spokes for the angle, and each
//! series as radius against angle.
//!
//! The angle is counted anticlockwise from the right, in radians in the data and in
//! degrees on the spokes. A negative radius is drawn on the opposite side, as it
//! would be by hand. Zero is the centre; the outer ring is the largest radius shown
//! (a round number above the data, or the plot's own y maximum).

use eframe::egui::{Align, Align2, FontId, Painter, Pos2, Rect, Stroke};
use opencalc_core::document::SeriesKind;
use opencalc_core::eval::PlotData;

use super::context::{caption, MIN_TEXT};
use super::geometry::{Geometry, Polar};
use super::marks::{self, Frame, Look};
use super::scales::Scales;
use super::ticks::{nice_ticks, Axis};
use super::{legend, Chart, PlotCtx};
use crate::format::axis_unit;
use crate::theme::{type_size, Theme};

/// Spokes (and angle labels) are this many degrees apart.
const SPOKE_DEGREES: u32 = 30;

/// The outer radius for data reaching out to `peak`: the first round number from
/// 1, 2, 5 times a power of ten that is not below it. A flat zero gets 1.
pub fn outer_radius(peak: f64) -> f64 {
    if !(peak.is_finite() && peak > 0.0) {
        return 1.0;
    }
    let (ticks, step) = nice_ticks(0.0, peak, 4);
    match ticks.last() {
        Some(last) if *last >= peak * (1.0 - 1e-9) => *last,
        Some(last) => last + step,
        None => peak,
    }
}

/// Where the point at `angle` (radians) and `radius` lands, for a centre, an outer
/// ring of `ring` pixels standing for `r_max`.
pub fn polar_position(centre: Pos2, ring: f32, r_max: f64, angle: f64, radius: f64) -> Pos2 {
    let distance = (radius / r_max) as f32 * ring;
    Pos2::new(
        centre.x + distance * angle.cos() as f32,
        centre.y - distance * angle.sin() as f32,
    )
}

/// How a label at `angle` (radians) is anchored to the point just outside the ring:
/// on the right it starts there, on the left it ends there, above it sits on it and
/// below it hangs from it, so wide text never lies across the ring.
pub fn label_anchor(angle: f64) -> Align2 {
    let (c, s) = (angle.cos(), angle.sin());
    let h = if c > 0.3 {
        Align::LEFT
    } else if c < -0.3 {
        Align::RIGHT
    } else {
        Align::Center
    };
    let v = if s > 0.3 {
        Align::BOTTOM
    } else if s < -0.3 {
        Align::TOP
    } else {
        Align::Center
    };
    Align2([h, v])
}

/// The angle in degrees, from 0 up to but not including 360, of the direction from
/// `centre` to `p` (anticlockwise from the right, as the plot is drawn).
pub fn angle_degrees(centre: Pos2, p: Pos2) -> f64 {
    let degrees = f64::from(centre.y - p.y)
        .atan2(f64::from(p.x - centre.x))
        .to_degrees();
    let wrapped = if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    };
    if wrapped >= 360.0 {
        0.0
    } else {
        wrapped
    }
}

/// Draw the polar chart and give back where things went, or `None` when the radius
/// or the room for the ring is too small.
pub(super) fn draw(
    painter: &Painter,
    ctx: &PlotCtx,
    c: &Chart,
    styles: &[opencalc_core::document::SeriesStyle],
    used: &[usize],
) -> Option<Geometry> {
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
    let pad = size(8.0);

    // The radius is shown in a readable unit of its dimension.
    let points = || data.series.iter().flat_map(|s| s.points.iter());
    let peak = points().fold(0.0_f64, |m, p| m.max(p.1.abs()));
    let (r_unit, r_scale) = axis_unit(ctx.units, data.left_dim(), peak);
    let r_max = view
        .y
        .map(|(_, hi)| hi)
        .or(data.limits[3])
        .map_or_else(|| outer_radius(peak / r_scale), |v| v / r_scale);
    if !(r_max.is_finite() && r_max > 0.0) {
        return None;
    }

    let bottom = if text_ok { size(22.0) } else { pad };
    let heading = if text_ok { size(16.0) } else { 0.0 };
    let area = Rect::from_min_max(
        Pos2::new(rect.min.x + pad, top + heading),
        Pos2::new(rect.max.x - pad, rect.max.y - bottom),
    );
    let margin = if text_ok { size(20.0) } else { pad };
    let ring = area.width().min(area.height()) / 2.0 - margin;
    if ring < 8.0 {
        return None;
    }
    let disc = Disc {
        painter,
        theme: ctx.paper,
        centre: area.center(),
        ring,
        r_max,
        zoom,
        text_ok,
        font: FontId::proportional(size(type_size::SMALL)),
    };
    disc.rings_and_spokes(spec.options.grid);
    let screen = disc.series(area, data, styles, r_scale);

    if text_ok {
        disc.label(
            Pos2::new(rect.min.x + pad, top + size(2.0)),
            Align2::LEFT_TOP,
            caption(&spec.y_label, &r_unit),
        );
        disc.label(
            Pos2::new(rect.center().x, rect.max.y - size(4.0)),
            Align2::CENTER_BOTTOM,
            caption(&spec.x_label, ""),
        );
        if let Some(entries) = legend::entries(ctx, spec, data, styles, used, false) {
            legend::draw(painter, ctx, &entries, area, &disc.font, zoom);
        }
    }
    let names = styles
        .iter()
        .enumerate()
        .map(|(k, style)| {
            super::context::series_name(spec, used.get(k).copied().unwrap_or(k), style)
        })
        .collect();
    let values = data
        .series
        .iter()
        .map(|s| s.points.iter().map(|p| (p.0, p.1 / r_scale)).collect())
        .collect();
    Some(Geometry {
        area,
        scales: stand_in_scales(r_max, r_unit, r_scale),
        screen,
        values,
        names,
        right: vec![false; data.series.len()],
        polar: Some(Polar {
            centre: disc.centre,
            ring,
            r_max,
        }),
    })
}

/// The axes are stand-ins: the angle runs round once, the radius out to the ring.
fn stand_in_scales(r_max: f64, r_unit: String, r_scale: f64) -> Scales {
    Scales {
        x: Axis {
            lo: 0.0,
            hi: std::f64::consts::TAU,
            log: false,
        },
        y: Axis {
            lo: 0.0,
            hi: r_max,
            log: false,
        },
        y2: None,
        x_unit: "rad".to_owned(),
        y_unit: r_unit,
        y2_unit: String::new(),
        x_scale: 1.0,
        y_scale: r_scale,
        y2_scale: 1.0,
    }
}

/// What the drawing steps share: the circle the data is drawn on.
struct Disc<'a> {
    painter: &'a Painter,
    theme: &'a Theme,
    centre: Pos2,
    ring: f32,
    r_max: f64,
    zoom: f32,
    text_ok: bool,
    font: FontId,
}

impl Disc<'_> {
    fn size(&self, base: f32) -> f32 {
        base * self.zoom
    }

    fn label(&self, at: Pos2, align: Align2, text: String) {
        self.painter
            .text(at, align, text, self.font.clone(), self.theme.text_weak);
    }

    /// Rings (the grid), the outer ring, spokes and their labels.
    fn rings_and_spokes(&self, grid_lines: bool) {
        let (painter, centre, ring, r_max) = (self.painter, self.centre, self.ring, self.r_max);
        let grid = Stroke::new(1.0_f32, self.theme.raised);
        let strong = Stroke::new(1.0_f32, self.theme.border_strong);
        let (ticks, step) = nice_ticks(0.0, r_max, ((ring / self.size(40.0)).max(2.0)) as usize);
        for t in ticks.iter().filter(|t| **t > 0.0) {
            let radius = (*t / r_max) as f32 * ring;
            if grid_lines {
                painter.circle_stroke(centre, radius, grid);
            } else {
                let at = Pos2::new(centre.x + radius, centre.y);
                let half = self.size(3.0);
                painter.vline(at.x, (at.y - half)..=(at.y + half), strong);
            }
            if self.text_ok && *t < r_max * (1.0 - 1e-9) {
                self.label(
                    Pos2::new(centre.x + radius, centre.y + self.size(2.0)),
                    Align2::CENTER_TOP,
                    super::ticks::tick_text(*t, step),
                );
            }
        }
        painter.circle_stroke(centre, ring, strong);
        for degrees in (0..360).step_by(SPOKE_DEGREES as usize) {
            let angle = f64::from(degrees).to_radians();
            if grid_lines {
                painter.line_segment(
                    [centre, polar_position(centre, ring, 1.0, angle, 1.0)],
                    grid,
                );
            }
            if self.text_ok {
                self.label(
                    polar_position(centre, ring + self.size(4.0), 1.0, angle, 1.0),
                    label_anchor(angle),
                    format!("{degrees}\u{b0}"),
                );
            }
        }
    }

    /// Draw the series, clipped to the plotting area. Gives back the screen position
    /// of every point, by series.
    fn series(
        &self,
        area: Rect,
        data: &PlotData,
        styles: &[opencalc_core::document::SeriesStyle],
        r_scale: f64,
    ) -> Vec<Vec<Pos2>> {
        let (centre, ring, r_max) = (self.centre, self.ring, self.r_max);
        let clipped = self.painter.with_clip_rect(area);
        let screen: Vec<Vec<Pos2>> = data
            .series
            .iter()
            .map(|s| {
                s.points
                    .iter()
                    .map(|p| polar_position(centre, ring, r_max, p.0, p.1 / r_scale))
                    .collect()
            })
            .collect();
        for (k, line) in screen.iter().enumerate() {
            let style = &styles[k];
            let colour = self.theme.series[style.colour.map_or(k, usize::from) % 8];
            let look = Look {
                colour,
                width: (1.6 * self.zoom).max(1.0),
                radius: (3.0 * self.zoom).max(2.0),
            };
            match style.kind {
                // A stem or a column is a spoke from the centre to the point.
                SeriesKind::Stem | SeriesKind::Bar => {
                    for p in line {
                        clipped.line_segment([centre, *p], Stroke::new(look.width, colour));
                        marks::marker(&clipped, style.marker, *p, look.radius, colour);
                    }
                }
                kind => {
                    let frame = Frame {
                        baseline: centre.y,
                        bar_width: 0.0,
                        bar_offset: 0.0,
                    };
                    // Steps have no meaning round a circle: draw them as lines.
                    let kind = if kind == SeriesKind::Step {
                        SeriesKind::Line
                    } else {
                        kind
                    };
                    marks::draw(&clipped, kind, style.marker, line, look, &frame);
                }
            }
        }
        screen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Vec2;

    #[test]
    fn the_outer_radius_is_a_round_number_not_below_the_data() {
        assert_eq!(outer_radius(7.0), 8.0);
        assert_eq!(outer_radius(8.0), 8.0);
        assert_eq!(outer_radius(0.9), 1.0);
        assert_eq!(outer_radius(1.0), 1.0);
        assert_eq!(outer_radius(130.0), 150.0);
        assert_eq!(outer_radius(0.0), 1.0, "a plot of zeros still has a ring");
        assert_eq!(outer_radius(f64::NAN), 1.0);
        assert_eq!(outer_radius(-3.0), 1.0);
        for peak in [1e-9, 3.3e-5, 12.0, 4.4e8] {
            let outer = outer_radius(peak);
            assert!(
                outer >= peak * (1.0 - 1e-9) && outer < 3.0 * peak,
                "{peak} -> {outer}"
            );
        }
    }

    #[test]
    fn angles_go_anticlockwise_from_the_right_with_up_on_the_screen() {
        let c = Pos2::new(100.0, 100.0);
        let at = |angle: f64, radius: f64| polar_position(c, 50.0, 1.0, angle, radius);
        let right = at(0.0, 1.0);
        assert!((right.x - 150.0).abs() < 1e-4 && (right.y - 100.0).abs() < 1e-4);
        let up = at(std::f64::consts::FRAC_PI_2, 1.0);
        assert!(
            (up.x - 100.0).abs() < 1e-4 && (up.y - 50.0).abs() < 1e-4,
            "90 degrees is up"
        );
        let left = at(std::f64::consts::PI, 0.5);
        assert!((left.x - 75.0).abs() < 1e-4);
    }

    #[test]
    fn a_negative_radius_is_on_the_opposite_side() {
        let c = Pos2::new(0.0, 0.0);
        let p = polar_position(c, 10.0, 1.0, 0.0, 1.0);
        let q = polar_position(c, 10.0, 1.0, 0.0, -1.0);
        assert!((p.x + q.x).abs() < 1e-4 && (p.y + q.y).abs() < 1e-4);
        let centre = polar_position(c, 10.0, 1.0, 1.234, 0.0);
        assert!(centre.x.abs() < 1e-6 && centre.y.abs() < 1e-6);
    }

    #[test]
    fn labels_are_anchored_so_they_sit_outside_the_ring() {
        use std::f64::consts::{FRAC_PI_2, PI};
        assert_eq!(label_anchor(0.0), Align2([Align::LEFT, Align::Center]));
        assert_eq!(label_anchor(PI), Align2([Align::RIGHT, Align::Center]));
        assert_eq!(
            label_anchor(FRAC_PI_2),
            Align2([Align::Center, Align::BOTTOM])
        );
        assert_eq!(
            label_anchor(3.0 * FRAC_PI_2),
            Align2([Align::Center, Align::TOP])
        );
        assert_eq!(label_anchor(PI / 4.0), Align2([Align::LEFT, Align::BOTTOM]));
        assert_eq!(
            label_anchor(5.0 * PI / 4.0),
            Align2([Align::RIGHT, Align::TOP])
        );
    }

    #[test]
    fn the_screen_direction_gives_back_the_angle_in_degrees() {
        let c = Pos2::new(100.0, 100.0);
        for degrees in [0.0, 30.0, 90.0, 135.0, 180.0, 269.0, 359.0] {
            let p = polar_position(c, 50.0, 1.0, f64::to_radians(degrees), 1.0);
            let back = angle_degrees(c, p);
            assert!(
                (back - degrees).abs() < 1e-3,
                "{degrees} came back as {back}"
            );
        }
        assert_eq!(angle_degrees(c, c + Vec2::new(1.0, 0.0)), 0.0);
        assert!(angle_degrees(c, c + Vec2::new(1.0, -0.0001)) < 1.0);
        assert!(
            (angle_degrees(c, c + Vec2::new(0.0, 5.0)) - 270.0).abs() < 1e-9,
            "down is 270"
        );
    }
}
