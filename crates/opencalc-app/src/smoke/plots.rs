//! Plot styles and options, seen in the pixels the exporter paints: each kind of
//! series puts its colour on the page, and each option visibly changes the result.
//!
//! Antialiased lines and translucent columns never hit a palette colour exactly, so
//! "ink of series k" counts the clearly coloured pixels whose nearest palette
//! colour is k.

#![cfg(test)]

use opencalc_core::document::{
    LegendMode, Marker, MathRegion, PlotOptions, PlotSpec, Point, RegionKind, SeriesKind,
    SeriesStyle, Worksheet,
};
use opencalc_core::eval::evaluate_all;

use crate::export::raster::Image;
use crate::export::{render_pages, Source};
use crate::sheet::OpenSheet;
use crate::theme::{PaletteKey, Theme};

/// A page with the data defined and one plot of `spec` at (100, 100).
fn sheet(spec: PlotSpec) -> Worksheet {
    let mut s = OpenSheet::new("t");
    s.add(RegionKind::math("w := 1, 2..6"), Point::new(40.0, 20.0));
    s.add(RegionKind::Plot(spec), Point::new(100.0, 100.0));
    s.doc
}

fn plot(series: &[&str]) -> PlotSpec {
    let mut spec = PlotSpec::default().with_x("w");
    for (k, s) in series.iter().enumerate() {
        spec = spec.with_series(k, s);
    }
    spec
}

fn render(doc: &Worksheet) -> Image {
    let output = evaluate_all(
        &doc.math_regions(),
        &doc.plot_inputs(),
        &doc.units().unwrap(),
    );
    let theme = Theme::palette(PaletteKey::Neutral);
    let source = Source {
        doc,
        output: &output,
        theme: &theme,
    };
    let mut first = None;
    render_pages(&source, 2.0, |_, image, _| {
        first.get_or_insert(image);
        Ok(())
    })
    .unwrap();
    first.unwrap()
}

/// The plot region on the 2x page: (x0, y0, x1, y1) in pixels.
const PLOT: (usize, usize, usize, usize) = (200, 200, 720, 540);

fn pixels(image: &Image) -> impl Iterator<Item = [u8; 3]> + '_ {
    (PLOT.1..PLOT.3.min(image.height))
        .flat_map(move |y| (PLOT.0..PLOT.2).map(move |x| image.pixel(x, y)))
}

fn distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    (0..3).map(|i| u32::from(a[i].abs_diff(b[i]))).sum()
}

/// A clearly coloured pixel: well away from the greys of text, frame and grid.
fn coloured(p: [u8; 3]) -> bool {
    let (hi, lo) = (p.iter().max().unwrap(), p.iter().min().unwrap());
    hi - lo > 50
}

/// A worksheet page is painted with the paper palette, whatever the app theme.
fn palette() -> [[u8; 3]; 8] {
    Theme::paper().series.map(|c| [c.r(), c.g(), c.b()])
}

/// Coloured pixels in the plot whose nearest palette colour is series `k`.
fn ink(image: &Image, k: usize) -> usize {
    let colours = palette();
    pixels(image)
        .filter(|p| coloured(*p))
        .filter(|p| (0..8).min_by_key(|i| distance(*p, colours[*i])) == Some(k))
        .count()
}

fn is_dark(p: &[u8; 3]) -> bool {
    p.iter().map(|c| u32::from(*c)).sum::<u32>() < 450
}

/// The longest horizontal run of non-white pixels inside the plot region (so not
/// its own border): a chart has long straight lines (its frame and grid), a line of
/// text never does.
fn longest_line(image: &Image) -> usize {
    (PLOT.1 + 10..PLOT.3.min(image.height) - 10)
        .map(|y| {
            let mut best = 0;
            let mut run = 0;
            for x in PLOT.0 + 10..PLOT.2 - 10 {
                if image.pixel(x, y) == [255, 255, 255] {
                    run = 0;
                } else {
                    run += 1;
                    best = best.max(run);
                }
            }
            best
        })
        .max()
        .unwrap_or(0)
}

/// Pixels in the plot that are not white.
fn marked(image: &Image) -> usize {
    pixels(image).filter(|p| *p != [255, 255, 255]).count()
}

fn styled(kind: SeriesKind) -> PlotSpec {
    plot(&["w ^ 2"]).with_style(
        0,
        SeriesStyle {
            kind,
            ..SeriesStyle::default()
        },
    )
}

#[test]
fn every_kind_of_series_puts_its_colour_on_the_page_and_looks_different() {
    let mut seen: Vec<(SeriesKind, usize)> = Vec::new();
    let mut previous: Vec<(SeriesKind, Image)> = Vec::new();
    for kind in SeriesKind::ALL {
        let image = render(&sheet(styled(kind)));
        let n = ink(&image, 0);
        assert!(n > 20, "{kind:?} drew only {n} pixels of its colour");
        for (other, earlier) in &previous {
            let differs = pixels(&image).zip(pixels(earlier)).any(|(a, b)| a != b);
            assert!(differs, "{kind:?} looks the same as {other:?}");
        }
        previous.push((kind, image));
        seen.push((kind, n));
    }
    let ink_of = |kind| seen.iter().find(|(k, _)| *k == kind).unwrap().1;
    assert!(
        ink_of(SeriesKind::Bar) > 3 * ink_of(SeriesKind::Line),
        "columns cover far more than a line: {seen:?}"
    );
    assert!(ink_of(SeriesKind::LineMarkers) > ink_of(SeriesKind::Line));
}

#[test]
fn the_last_bar_is_not_cut_off_by_the_edge_of_the_plot() {
    // The plotting area ends a few pixels inside the region's right edge. A column
    // at the last x must end before it, leaving room for half a column.
    let image = render(&sheet(styled(SeriesKind::Bar)));
    let area_right = (PLOT.2 - 17..PLOT.2 - 12)
        .map(|x| {
            (PLOT.1..PLOT.3)
                .filter(|y| coloured(image.pixel(x, *y)))
                .count()
        })
        .max()
        .unwrap();
    assert_eq!(
        area_right, 0,
        "a column reaches the right edge of the plotting area"
    );
}

#[test]
fn a_chosen_colour_replaces_the_cycle_colour() {
    let own = plot(&["w ^ 2"]).with_style(
        0,
        SeriesStyle {
            colour: Some(3),
            ..SeriesStyle::default()
        },
    );
    let image = render(&sheet(own));
    assert!(ink(&image, 3) > 20);
    // A few antialiased pixels may sit nearer the old colour; the line is not drawn in it.
    assert!(
        ink(&image, 0) * 20 < ink(&image, 3),
        "{} pixels of the cycle colour, {} of the chosen one",
        ink(&image, 0),
        ink(&image, 3)
    );
}

#[test]
fn each_series_takes_its_own_style_and_colour() {
    let spec = plot(&["w", "w ^ 2"])
        .with_style(
            0,
            SeriesStyle {
                kind: SeriesKind::Markers,
                marker: Marker::Square,
                ..SeriesStyle::default()
            },
        )
        .with_style(
            1,
            SeriesStyle {
                kind: SeriesKind::Step,
                ..SeriesStyle::default()
            },
        );
    let image = render(&sheet(spec));
    assert!(ink(&image, 0) > 20, "the markers");
    assert!(ink(&image, 1) > 20, "the steps");
}

#[test]
fn the_grid_can_be_turned_off() {
    let on = render(&sheet(plot(&["w ^ 2"])));
    let off_spec = plot(&["w ^ 2"]).with_options(PlotOptions {
        grid: false,
        ..PlotOptions::default()
    });
    let off = render(&sheet(off_spec));
    // Grid lines are long: with them the plot has hundreds more marked pixels.
    assert!(
        marked(&on) > marked(&off) + 300,
        "marked pixels: {} with the grid, {} without",
        marked(&on),
        marked(&off)
    );
    assert!(ink(&off, 0) > 20, "the data is still drawn");
}

#[test]
fn a_log_axis_changes_the_picture_and_still_draws_the_data() {
    let linear = render(&sheet(plot(&["10 ^ w"])));
    let spec = plot(&["10 ^ w"]).with_options(PlotOptions {
        y_log: true,
        ..PlotOptions::default()
    });
    let log = render(&sheet(spec));
    assert!(ink(&log, 0) > 20);
    assert!(pixels(&log).zip(pixels(&linear)).any(|(a, b)| a != b));
    // 10^w on a log axis is a straight line across the whole height; on a linear
    // axis it hugs the bottom, so far fewer rows hold any of the line.
    let rows = |image: &Image| {
        (PLOT.1..PLOT.3)
            .filter(|y| {
                (PLOT.0..PLOT.2).any(|x| {
                    let p = image.pixel(x, *y);
                    coloured(p) && distance(p, palette()[0]) < 80
                })
            })
            .count()
    };
    assert!(
        rows(&log) > rows(&linear),
        "{} vs {}",
        rows(&log),
        rows(&linear)
    );
}

#[test]
fn a_log_axis_with_a_zero_shows_the_error_instead_of_a_chart() {
    let normal = render(&sheet(plot(&["w"])));
    let spec = plot(&["w - 1"]).with_options(PlotOptions {
        y_log: true,
        ..PlotOptions::default()
    });
    let image = render(&sheet(spec));
    // No chart: the frame and axes are missing, only the message is there.
    assert!(longest_line(&normal) > 200, "a chart has long lines");
    assert!(
        longest_line(&image) < 100,
        "the longest line is {} pixels",
        longest_line(&image)
    );
    assert!(marked(&image) > 50, "the message is painted");
}

#[test]
fn limits_crop_the_data_to_the_chosen_range() {
    let wide = render(&sheet(plot(&["w ^ 2"])));
    let options = PlotOptions {
        x_max: MathRegion::new("3"),
        y_max: MathRegion::new("10"),
        ..PlotOptions::default()
    };
    let cropped = render(&sheet(plot(&["w ^ 2"]).with_options(options)));
    assert!(ink(&cropped, 0) > 20);
    assert!(pixels(&cropped).zip(pixels(&wide)).any(|(a, b)| a != b));
}

#[test]
fn limits_with_the_wrong_units_show_an_error() {
    let normal = render(&sheet(plot(&["w ^ 2"])));
    let options = PlotOptions {
        y_max: MathRegion::new("10 s"),
        ..PlotOptions::default()
    };
    let image = render(&sheet(plot(&["w ^ 2"]).with_options(options)));
    assert!(longest_line(&normal) > 200, "a chart has long lines");
    assert!(longest_line(&image) < 100, "no chart is drawn");
    assert!(marked(&image) > 50, "the message is painted");
}

/// Dark pixels in the top right of the plotting area, where the legend goes.
fn legend_ink(image: &Image) -> usize {
    (230..340)
        .flat_map(|y| (420..715).map(move |x| image.pixel(x, y)))
        .filter(is_dark)
        .count()
}

#[test]
fn a_single_series_has_no_legend_unless_asked_and_two_can_hide_it() {
    let one = render(&sheet(plot(&["w"])));
    let forced = render(&sheet(plot(&["w"]).with_options(PlotOptions {
        legend: LegendMode::Show,
        ..PlotOptions::default()
    })));
    assert!(
        legend_ink(&forced) > legend_ink(&one),
        "a single series gets a legend when asked"
    );
    let two = render(&sheet(plot(&["w", "w ^ 2"])));
    let hidden = render(&sheet(plot(&["w", "w ^ 2"]).with_options(PlotOptions {
        legend: LegendMode::Hide,
        ..PlotOptions::default()
    })));
    assert!(
        legend_ink(&two) > legend_ink(&hidden),
        "hiding removes the legend"
    );
}

#[test]
fn a_legend_name_is_shown_instead_of_the_expression() {
    let named = |name: &str| {
        plot(&["w", "w ^ 2"]).with_style(
            0,
            SeriesStyle {
                name: name.to_owned(),
                ..SeriesStyle::default()
            },
        )
    };
    let short = render(&sheet(named("a")));
    let long = render(&sheet(named("a much longer legend name")));
    assert!(
        legend_ink(&long) > legend_ink(&short),
        "the name is what is drawn"
    );
}

// ---- a second axis and series with their own x ----------------------------------

use opencalc_core::document::YAxis;

/// A page with these math lines and one plot of `spec` at (100, 100).
fn sheet_of(lines: &[&str], spec: PlotSpec) -> Worksheet {
    let mut s = OpenSheet::new("t");
    for (k, line) in lines.iter().enumerate() {
        s.add(
            RegionKind::math(line),
            Point::new(40.0, 20.0 + 26.0 * k as f32),
        );
    }
    s.add(RegionKind::Plot(spec), Point::new(100.0, 140.0));
    s.doc
}

fn series_style(spec: PlotSpec, k: usize, f: impl FnOnce(&mut SeriesStyle)) -> PlotSpec {
    let mut style = spec.style(k);
    f(&mut style);
    spec.with_style(k, style)
}

/// The plot sits at (100, 140) here, 40 units lower than in the other tests.
const LOWER: (usize, usize, usize, usize) = (200, 280, 720, 620);

fn ink_in(image: &Image, k: usize, r: (usize, usize, usize, usize)) -> usize {
    let colours = palette();
    (r.1..r.3.min(image.height))
        .flat_map(|y| (r.0..r.2).map(move |x| image.pixel(x, y)))
        .filter(|p| coloured(*p))
        .filter(|p| (0..8).min_by_key(|i| distance(*p, colours[*i])) == Some(k))
        .count()
}

fn dark_in(image: &Image, r: (usize, usize, usize, usize)) -> usize {
    (r.1..r.3.min(image.height))
        .flat_map(|y| (r.0..r.2).map(move |x| image.pixel(x, y)))
        .filter(is_dark)
        .count()
}

#[test]
fn a_series_with_its_own_x_values_draws_a_closed_curve() {
    // x = cos t and y = sin t: a circle, drawn with nothing in the shared x field.
    let spec = series_style(plot(&["sin(t)"]).with_x(""), 0, |s| s.x = "cos(t)".into());
    let doc = sheet_of(&["t := 0, 0.2..6.4"], spec);
    let image = render(&doc);
    // Ink in all four quarters of the plotting area: it goes all the way round.
    let (x0, y0, x1, y1) = (292, 350, 704, 560);
    let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
    for (name, r) in [
        ("top left", (x0, y0, mx, my)),
        ("top right", (mx, y0, x1, my)),
        ("bottom left", (x0, my, mx, y1)),
        ("bottom right", (mx, my, x1, y1)),
    ] {
        assert!(ink_in(&image, 0, r) > 15, "nothing drawn in the {name}");
    }
}

#[test]
fn a_plot_with_every_series_on_its_own_x_needs_no_shared_x() {
    let spec = series_style(plot(&["t ^ 2"]).with_x(""), 0, |s| s.x = "t".into());
    let image = render(&sheet_of(&["t := 1, 2..6"], spec));
    assert!(ink_in(&image, 0, LOWER) > 20, "the series is drawn");
    assert!(longest_line(&image) > 0);
}

fn units_sheet(spec: PlotSpec) -> Worksheet {
    sheet_of(
        &[
            "w := 1 s, 2 s..6 s",
            "d := 2 m * w / s",
            "F := 5 N * w ^ 2 / s ^ 2",
        ],
        spec,
    )
}

/// Dark pixels in the column where the right axis writes its numbers: just right of
/// the plotting area when there is a right axis (which then leaves 46 points).
fn right_numbers(image: &Image) -> usize {
    dark_in(image, (636, 300, 712, 600))
}

#[test]
fn a_series_on_the_right_axis_gets_its_own_scale_and_labels() {
    let plain = render(&units_sheet(plot(&["d"])));
    let two = plot(&["d", "F"]);
    let on_right = series_style(two.clone(), 1, |s| s.axis = YAxis::Right);
    let dual = render(&units_sheet(on_right));
    assert!(
        ink_in(&dual, 0, LOWER) > 20 && ink_in(&dual, 1, LOWER) > 20,
        "both are drawn"
    );
    // Without a right axis the plotting area runs through that column, so the same
    // column holds its lines and data instead of numbers: it must differ, and with a
    // right axis the area ends before it.
    assert!(
        right_numbers(&dual) > 20,
        "numbers on the right: {}",
        right_numbers(&dual)
    );
    assert_ne!(right_numbers(&dual), right_numbers(&plain));
    let area_edge = |image: &Image| {
        (LOWER.1..LOWER.3)
            .filter(|y| image.pixel(704, *y) != [255, 255, 255])
            .count()
    };
    assert!(
        area_edge(&plain) > 100,
        "the plain plot's frame is at the edge"
    );
    assert!(
        area_edge(&dual) < area_edge(&plain) / 2,
        "the dual plot's frame is not"
    );
}

#[test]
fn series_of_different_kinds_on_one_axis_still_show_the_error() {
    let both_left = render(&units_sheet(plot(&["d", "F"])));
    assert_eq!(ink_in(&both_left, 1, LOWER), 0, "no chart is drawn");
    assert!(longest_line_in(&both_left, LOWER) < 100);
}

/// `longest_line` for a plot at another position.
fn longest_line_in(image: &Image, r: (usize, usize, usize, usize)) -> usize {
    (r.1 + 10..r.3.min(image.height) - 10)
        .map(|y| {
            let (mut best, mut run) = (0, 0);
            for x in r.0 + 10..r.2 - 10 {
                if image.pixel(x, y) == [255, 255, 255] {
                    run = 0;
                } else {
                    run += 1;
                    best = best.max(run);
                }
            }
            best
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn the_right_axis_has_its_own_limits_and_log_scale() {
    let base = series_style(plot(&["d", "F"]), 1, |s| s.axis = YAxis::Right);
    let wide = render(&units_sheet(base.clone()));
    let limited = base.clone().with_options(PlotOptions {
        y2_max: MathRegion::new("200 N"),
        ..PlotOptions::default()
    });
    let cropped = render(&units_sheet(limited));
    assert!(ink_in(&cropped, 1, LOWER) > 20);
    assert!(
        (LOWER.1..LOWER.3)
            .any(|y| (LOWER.0..LOWER.2).any(|x| wide.pixel(x, y) != cropped.pixel(x, y))),
        "a right-axis limit changes the picture"
    );
    // Units are checked against the right axis: metres do not fit a force axis.
    let wrong = base.with_options(PlotOptions {
        y2_max: MathRegion::new("10 m"),
        ..PlotOptions::default()
    });
    let image = render(&units_sheet(wrong));
    assert_eq!(ink_in(&image, 1, LOWER), 0, "no chart is drawn");
}

#[test]
fn the_legend_says_which_series_are_on_the_right() {
    let spec = series_style(plot(&["d", "F"]), 1, |s| s.axis = YAxis::Right);
    let with = render(&units_sheet(spec.clone()));
    let without = render(&units_sheet(series_style(spec, 1, |s| {
        s.axis = YAxis::Left
    })));
    // Both draw something in the top right; the right-axis one writes more text there.
    let corner = |image: &Image| dark_in(image, (420, 300, 715, 400));
    assert!(corner(&with) > 0 || corner(&without) > 0);
}

// ---- polar plots ------------------------------------------------------------------

use opencalc_core::document::PlotKind;

fn polar_spec(series: &[&str]) -> PlotSpec {
    plot(series).with_x("a").with_options(PlotOptions {
        kind: PlotKind::Polar,
        ..PlotOptions::default()
    })
}

const ANGLES: &str = "a := 0, 0.1..6.3";

fn centroid_x(image: &Image, k: usize) -> Option<f64> {
    let colours = palette();
    let (mut sum, mut n) = (0.0, 0.0);
    for y in LOWER.1..LOWER.3.min(image.height) {
        for x in LOWER.0..LOWER.2 {
            let p = image.pixel(x, y);
            if coloured(p) && (0..8).min_by_key(|i| distance(p, colours[*i])) == Some(k) {
                sum += x as f64;
                n += 1.0;
            }
        }
    }
    (n > 0.0).then(|| sum / n)
}

#[test]
fn a_polar_circle_is_drawn_all_the_way_round() {
    let doc = sheet_of(&[ANGLES, "r := 2 m + 0 m * a"], polar_spec(&["r"]));
    let image = render(&doc);
    let (x0, y0, x1, y1) = (216, 320, 704, 580);
    let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
    for (name, r) in [
        ("top left", (x0, y0, mx, my)),
        ("top right", (mx, y0, x1, my)),
        ("bottom left", (x0, my, mx, y1)),
        ("bottom right", (mx, my, x1, y1)),
    ] {
        assert!(ink_in(&image, 0, r) > 15, "nothing drawn in the {name}");
    }
    // Nothing is drawn at the centre: the data is a ring, not a disc.
    assert_eq!(ink_in(&image, 0, (mx - 20, my - 20, mx + 20, my + 20)), 0);
}

#[test]
fn a_positive_radius_points_along_the_angle_and_a_negative_one_the_other_way() {
    // r = 1 + cos a reaches out to the right (angle 0); minus that, to the left.
    let out = render(&sheet_of(
        &[ANGLES, "r := 1 m + 1 m * cos(a)"],
        polar_spec(&["r"]),
    ));
    let back = render(&sheet_of(
        &[ANGLES, "r := -1 m - 1 m * cos(a)"],
        polar_spec(&["r"]),
    ));
    let centre = 460.0;
    let right = centroid_x(&out, 0).expect("drawn");
    let left = centroid_x(&back, 0).expect("drawn");
    assert!(
        right > centre + 15.0,
        "a positive radius reaches right: {right}"
    );
    assert!(
        left < centre - 15.0,
        "a negative radius reaches left: {left}"
    );
}

#[test]
fn the_polar_grid_can_be_turned_off() {
    let on = render(&sheet_of(
        &[ANGLES, "r := 2 m + 0 m * a"],
        polar_spec(&["r"]),
    ));
    let off_spec = polar_spec(&["r"]).with_options(PlotOptions {
        kind: PlotKind::Polar,
        grid: false,
        ..PlotOptions::default()
    });
    let off = render(&sheet_of(&[ANGLES, "r := 2 m + 0 m * a"], off_spec));
    let marked_in = |image: &Image| {
        (LOWER.1..LOWER.3)
            .flat_map(|y| (LOWER.0..LOWER.2).map(move |x| image.pixel(x, y)))
            .filter(|p| *p != [255, 255, 255])
            .count()
    };
    assert!(
        marked_in(&on) > marked_in(&off) + 400,
        "rings and spokes: {} against {}",
        marked_in(&on),
        marked_in(&off)
    );
    assert!(ink_in(&off, 0, LOWER) > 20, "the data is still drawn");
}

#[test]
fn angles_with_a_unit_show_the_error_instead_of_a_chart() {
    let doc = sheet_of(
        &["t := 0 s, 1 s..4 s", "r := 1 m + 0 m * t / s"],
        polar_spec(&["r"]).with_x("t"),
    );
    let image = render(&doc);
    // No chart: no frame, rings or spokes, only the message (which is red text, so
    // it cannot be told from a series by colour).
    assert!(
        longest_line_in(&image, LOWER) < 100,
        "the longest line is {} pixels",
        longest_line_in(&image, LOWER)
    );
}

#[test]
fn a_stem_is_a_spoke_from_the_centre_and_markers_are_not() {
    let styled = |kind| {
        let spec = polar_spec(&["r"]);
        series_style(spec, 0, |s| s.kind = kind)
    };
    let lines = &["a := 0, 1..6", "r := 2 m + 0 m * a"];
    let stems = render(&sheet_of(lines, styled(SeriesKind::Stem)));
    let dots = render(&sheet_of(lines, styled(SeriesKind::Markers)));
    let near_centre = (420, 410, 500, 490);
    assert!(
        ink_in(&stems, 0, near_centre) > 30,
        "spokes reach the middle"
    );
    assert_eq!(ink_in(&dots, 0, near_centre), 0, "markers do not");
}

#[test]
fn a_polar_plot_ignores_the_right_axis_and_log_settings() {
    // The settings carry over from an X-Y plot that had them; they do no harm.
    let spec = polar_spec(&["r", "r"]).with_options(PlotOptions {
        kind: PlotKind::Polar,
        y_log: true,
        x_log: true,
        ..PlotOptions::default()
    });
    let spec = series_style(spec, 1, |s| s.axis = YAxis::Right);
    let image = render(&sheet_of(&[ANGLES, "r := 1 m + 1 m * cos(a)"], spec));
    assert!(ink_in(&image, 0, LOWER) > 20, "drawn");
    assert!(
        ink_in(&image, 1, LOWER) > 20,
        "the second series too, on the same circle"
    );
}
