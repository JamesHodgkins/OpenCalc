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
