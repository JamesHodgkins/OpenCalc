//! The interactive plot view: what a real paint reports about where things are, and
//! the window driven by pointer and wheel events.

#![cfg(test)]

use eframe::egui::{self, Event, Id, Modifiers, MouseWheelUnit, Pos2, RawInput, Rect, Sense};
use opencalc_core::document::{PlotSpec, Point, RegionId, RegionKind, Worksheet};
use opencalc_core::eval::evaluate_all;

use super::{press, release, Harness};
use crate::canvas::plot::{paint_in, Geometry, PlotCtx, View};
use crate::sheet::OpenSheet;
use crate::theme::Theme;

/// A worksheet with the data `w := 1, 2..6` and a plot of `w ^ 2` against it.
fn doc() -> (Worksheet, RegionId) {
    let mut s = OpenSheet::new("t");
    s.add(RegionKind::math("w := 1, 2..6"), Point::new(40.0, 20.0));
    let spec = PlotSpec::default().with_x("w").with_series(0, "w ^ 2");
    let id = s.add(RegionKind::Plot(spec), Point::new(100.0, 100.0));
    (s.doc, id)
}

/// Run one frame that paints the plot into a 400 x 300 area, as the window does.
fn paint(doc: &Worksheet, id: RegionId, view: &View) -> (Rect, Option<Geometry>) {
    let output = evaluate_all(
        &doc.math_regions(),
        &doc.plot_inputs(),
        &doc.units().unwrap(),
    );
    let paper = Theme::paper();
    let ctx = PlotCtx {
        paper: &paper,
        output: &output,
        units: doc.unit_system(),
    };
    let RegionKind::Plot(spec) = &doc.region(id).unwrap().kind else {
        panic!("not a plot")
    };
    let egui_ctx = egui::Context::default();
    let mut result = (Rect::NOTHING, None);
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
        ..Default::default()
    };
    let _ = egui_ctx.run(input, |c| {
        egui::CentralPanel::default().show(c, |ui| {
            let (response, painter) = ui.allocate_painter(egui::vec2(400.0, 300.0), Sense::hover());
            result = (
                response.rect,
                paint_in(&painter, &ctx, id, spec, response.rect, 1.0, view),
            );
        });
    });
    result
}

#[test]
fn a_painted_plot_reports_where_its_area_axes_and_points_are() {
    let (doc, id) = doc();
    let (rect, g) = paint(&doc, id, &View::default());
    let g = g.expect("a chart was drawn");
    assert!(
        rect.contains_rect(g.area),
        "the plotting area is inside the plot"
    );
    assert_eq!(g.screen.len(), 1);
    assert_eq!(g.screen[0].len(), 6);
    assert!(
        g.screen[0].iter().all(|p| g.area.contains(*p)),
        "every point is in the area"
    );
    assert_eq!(
        g.values[0][2],
        (3.0, 9.0),
        "the values behind the third point"
    );
    // The first point is at the bottom left of the data, the last at the top right.
    let (first, last) = (g.screen[0][0], g.screen[0][5]);
    assert!(first.x < last.x && first.y > last.y);
}

#[test]
fn hovering_a_point_finds_it_and_describes_it() {
    let (doc, id) = doc();
    let (_, g) = paint(&doc, id, &View::default());
    let g = g.unwrap();
    let on_third = g.screen[0][2] + egui::vec2(3.0, -2.0);
    let hit = g.nearest(on_third, 14.0).expect("a point is near");
    assert_eq!((hit.series, hit.index), (0, 2));
    assert_eq!(g.describe(&hit), "w ^ 2: x = 3, y = 9");
    let blank = g.area.center() + egui::vec2(60.0, -80.0);
    assert!(g.nearest(blank, 5.0).is_none());
}

#[test]
fn a_view_replaces_the_range_of_the_axes_that_are_painted() {
    let (doc, id) = doc();
    let view = View {
        x: Some((2.0, 4.0)),
        y: Some((0.0, 20.0)),
        y2: None,
    };
    let (_, g) = paint(&doc, id, &view);
    let g = g.unwrap();
    assert_eq!((g.scales.x.lo, g.scales.x.hi), (2.0, 4.0));
    assert_eq!((g.scales.y.lo, g.scales.y.hi), (0.0, 20.0));
    // Points outside the view are off the area, not squeezed into it.
    assert!(
        !g.area.contains(g.screen[0][5]),
        "x = 6 is beyond the right edge"
    );
    assert!(g.area.contains(g.screen[0][2]), "x = 3 is inside");
}

#[test]
fn zooming_in_about_a_point_shows_a_smaller_range_around_it() {
    let (doc, id) = doc();
    let (_, g) = paint(&doc, id, &View::default());
    let g = g.unwrap();
    let at = g.screen[0][3];
    let view = View::default().zoomed(&g, at, 0.5);
    let (_, zoomed) = paint(&doc, id, &view);
    let zoomed = zoomed.unwrap();
    assert!(
        (zoomed.scales.x.hi - zoomed.scales.x.lo) < 0.55 * (g.scales.x.hi - g.scales.x.lo),
        "half the range"
    );
    // The point that was under the pointer is still there.
    assert!(
        zoomed.screen[0][3].distance(at) < 1.0,
        "{:?} vs {:?}",
        zoomed.screen[0][3],
        at
    );
}

// ---- the window --------------------------------------------------------------------

fn first_plot(h: &Harness) -> RegionId {
    h.sheet()
        .doc
        .regions()
        .iter()
        .find(|r| matches!(r.kind, RegionKind::Plot(_)))
        .expect("the sample sheet has a plot")
        .id
}

fn view_of(h: &Harness, id: RegionId) -> View {
    h.ctx
        .data(|d| d.get_temp::<View>(Id::new(("plot-view", id.0))))
        .unwrap_or_default()
}

fn wheel(delta: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: egui::vec2(0.0, delta),
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn the_window_zooms_with_the_wheel_pans_with_a_drag_and_resets_with_a_double_click() {
    let mut h = Harness::new();
    h.settle();
    let id = first_plot(&h);
    crate::panels::plot_view::open(&h.ctx, id);
    for _ in 0..3 {
        h.frame();
    }
    let window = h
        .ctx
        .memory(|m| m.area_rect("plot-view-window"))
        .expect("the window is open");
    let centre = window.center();
    h.frame_with(vec![Event::PointerMoved(centre)]);
    assert!(
        !view_of(&h, id).is_moved(),
        "it starts at the plot's own view"
    );

    // Wheel up zooms in: the x range gets shorter.
    h.frame_with(vec![wheel(100.0)]);
    for _ in 0..8 {
        h.frame();
    }
    let zoomed = view_of(&h, id);
    assert!(zoomed.is_moved(), "the wheel zoomed");
    let (lo, hi) = zoomed.x.expect("the x range is set");

    // A drag to the right moves the view to the left: the data follows the pointer.
    h.frame_with(vec![press(centre)]);
    let mut at = centre;
    for _ in 0..6 {
        at += egui::vec2(10.0, 0.0);
        h.frame_with(vec![Event::PointerMoved(at)]);
    }
    h.frame_with(vec![release(at)]);
    h.frame();
    let panned = view_of(&h, id);
    let (plo, phi) = panned.x.unwrap();
    assert!(
        plo < lo && phi < hi,
        "dragged right: {plo}..{phi} against {lo}..{hi}"
    );
    assert!(
        ((phi - plo) - (hi - lo)).abs() < 1e-9 * (hi - lo).abs().max(1.0),
        "the span is kept"
    );

    // A double click goes back to the plot's own view.
    h.frame_with(vec![Event::PointerMoved(at)]);
    for _ in 0..2 {
        h.frame_with(vec![press(at)]);
        h.frame_with(vec![release(at)]);
    }
    h.frame();
    assert!(!view_of(&h, id).is_moved(), "reset");
}

#[test]
fn the_window_closes_when_its_plot_is_gone_and_nothing_is_saved_by_looking() {
    let mut h = Harness::new();
    h.settle();
    let id = first_plot(&h);
    let before = h.sheet().doc.clone();
    crate::panels::plot_view::open(&h.ctx, id);
    for _ in 0..3 {
        h.frame();
    }
    let centre = h
        .ctx
        .memory(|m| m.area_rect("plot-view-window"))
        .unwrap()
        .center();
    h.frame_with(vec![Event::PointerMoved(centre)]);
    h.frame_with(vec![wheel(100.0)]);
    for _ in 0..5 {
        h.frame();
    }
    assert!(view_of(&h, id).is_moved());
    assert_eq!(h.sheet().doc, before, "zooming does not touch the document");
    assert!(!h.sheet().dirty(), "and does not mark it changed");
    // A region that is not a plot closes the window instead of drawing something odd.
    crate::panels::plot_view::open(&h.ctx, RegionId(1));
    for _ in 0..3 {
        h.frame();
    }
    assert!(
        h.ctx
            .data(|d| d.get_temp::<u64>(Id::new("plot-view-open")))
            .is_none(),
        "the request to look at something that is not a plot is dropped"
    );
}

// ---- a polar plot --------------------------------------------------------------------

fn polar_doc() -> (Worksheet, RegionId) {
    use opencalc_core::document::{PlotKind, PlotOptions};
    let mut s = OpenSheet::new("t");
    s.add(
        RegionKind::math("a := 0, 0.5236..6.2832"),
        Point::new(40.0, 10.0),
    );
    s.add(
        RegionKind::math("r := 3 m + 0 m * a"),
        Point::new(40.0, 40.0),
    );
    let spec = PlotSpec::default()
        .with_x("a")
        .with_series(0, "r")
        .with_options(PlotOptions {
            kind: PlotKind::Polar,
            ..PlotOptions::default()
        });
    let id = s.add(RegionKind::Plot(spec), Point::new(100.0, 100.0));
    (s.doc, id)
}

#[test]
fn a_polar_plot_reports_its_circle_and_where_each_point_is_on_it() {
    let (doc, id) = polar_doc();
    let (_, g) = paint(&doc, id, &View::default());
    let g = g.expect("a polar chart was drawn");
    let polar = g.polar.expect("the geometry knows it is polar");
    assert!(g.area.contains(polar.centre));
    assert!(
        polar.r_max >= 3.0 && polar.r_max <= 5.0,
        "a round outer radius: {}",
        polar.r_max
    );
    // Every point of the constant-radius series is the same distance from the centre.
    let distances: Vec<f32> = g.screen[0]
        .iter()
        .map(|p| p.distance(polar.centre))
        .collect();
    let first = distances[0];
    assert!(
        distances.iter().all(|d| (d - first).abs() < 0.5),
        "{distances:?}"
    );
    assert!(
        (f64::from(first / polar.ring) * polar.r_max - 3.0).abs() < 0.01,
        "3 m on the scale"
    );
}

#[test]
fn hovering_a_polar_point_says_its_angle_and_radius() {
    let (doc, id) = polar_doc();
    let (_, g) = paint(&doc, id, &View::default());
    let g = g.unwrap();
    // The second point is at 30 degrees (0.5236 radians).
    let near = g.screen[0][1] + egui::vec2(2.0, 2.0);
    let hit = g.nearest(near, 14.0).expect("a point is near");
    let text = g.describe(&hit);
    assert!(text.contains("angle = 0.5236 rad (30"), "{text}");
    assert!(text.contains("r = 3 m"), "{text}");
    // The pointer alone: a distance from the centre and a direction.
    let polar = g.polar.unwrap();
    let text = g.describe_cursor(polar.centre + egui::vec2(0.0, -polar.ring / 2.0));
    assert!(text.contains("angle = 90"), "straight up: {text}");
    assert!(text.starts_with("r = "), "{text}");
    assert!(g.describe_cursor(polar.centre).contains("r = 0 m"));
}

#[test]
fn zooming_a_polar_plot_moves_the_outer_ring_and_panning_does_nothing() {
    let (doc, id) = polar_doc();
    let (_, g) = paint(&doc, id, &View::default());
    let g = g.unwrap();
    let before = g.polar.unwrap().r_max;
    let zoomed = View::default().zoomed(&g, g.area.center(), 0.5);
    let (_, gz) = paint(&doc, id, &zoomed);
    let after = gz.unwrap().polar.unwrap().r_max;
    assert!(
        (after - before / 2.0).abs() < 1e-9,
        "{before} became {after}"
    );
    assert_eq!(
        View::default().panned(&g, egui::vec2(30.0, 30.0)),
        View::default()
    );
    // Zooming far enough stops, rather than collapsing to nothing.
    let mut view = View::default();
    for _ in 0..200 {
        let (_, g) = paint(&doc, id, &view);
        view = view.zoomed(&g.unwrap(), Pos2::ZERO, 0.1);
    }
    let (_, g) = paint(&doc, id, &view);
    assert!(g.unwrap().polar.unwrap().r_max > 0.0);
}
