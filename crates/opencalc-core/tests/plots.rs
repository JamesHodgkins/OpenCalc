//! Plot regions: what they evaluate to, how editing them behaves, and that they
//! survive saving. Built through the public API, as the UI uses it.

use opencalc_core::document::{PlotSpec, Point, RegionId, RegionKind, Worksheet};
use opencalc_core::edit::{Edit, History};
use opencalc_core::eval::{evaluate_all, EvalError, Output, PlotData};
use opencalc_core::registry::UnitRegistry;
use opencalc_core::units::Dimension;
use opencalc_core::worker::Worker;

/// A worksheet with these math lines (in this layout order) and one plot.
fn sheet(lines: &[&str], plot: PlotSpec) -> (Worksheet, RegionId) {
    let mut w = Worksheet::new("t");
    for line in lines {
        add(&mut w, RegionKind::math(line));
    }
    let id = add(&mut w, RegionKind::plot());
    w.apply(Edit::SetPlot { id, spec: plot }).unwrap();
    (w, id)
}

fn add(w: &mut Worksheet, kind: RegionKind) -> RegionId {
    let r = w.new_region(kind, Point::default());
    let id = r.id;
    let at = w.regions().len();
    w.apply(Edit::InsertRegions {
        regions: vec![(at, r)],
    })
    .unwrap();
    id
}

fn run(w: &Worksheet) -> Output {
    evaluate_all(
        &w.math_regions(),
        &w.plot_inputs(),
        &UnitRegistry::builtin(),
    )
}

fn plot_of(out: &Output, id: RegionId) -> Result<PlotData, EvalError> {
    out.plots[&id].clone()
}

fn spec(x: &str, series: &[&str]) -> PlotSpec {
    let mut p = PlotSpec::default().with_x(x);
    for (i, s) in series.iter().enumerate() {
        p = p.with_series(i, s);
    }
    p
}

#[test]
fn a_plot_turns_its_expressions_into_points() {
    let (w, id) = sheet(&["x := 0..3"], spec("x", &["x ^ 2", "2 * x + 1"]));
    let data = plot_of(&run(&w), id).unwrap();
    assert_eq!(data.series.len(), 2);
    assert_eq!(
        data.series[0].points,
        [(0.0, 0.0), (1.0, 1.0), (2.0, 4.0), (3.0, 9.0)]
    );
    assert_eq!(
        data.series[1].points,
        [(0.0, 1.0), (1.0, 3.0), (2.0, 5.0), (3.0, 7.0)]
    );
}

#[test]
fn dimensions_are_kept_for_the_axes() {
    let (w, id) = sheet(
        &["x := 0 m, 1 m..2 m", "F := 3 kN"],
        spec("x", &["F * x", "F * x / 2"]),
    );
    let data = plot_of(&run(&w), id).unwrap();
    assert_eq!(data.x_dim, Dimension::LENGTH);
    assert_eq!(data.series[0].y_dim.mass, 1);
    assert_eq!(data.series[1].y_dim, data.series[0].y_dim);
    // Points are in SI: 3000 N * 2 m.
    assert_eq!(data.series[0].points[2], (2.0, 6000.0));
}

#[test]
fn series_that_measure_different_things_cannot_share_the_y_axis() {
    let text = error_text(
        &["x := 0 m, 1 m..2 m", "F := 3 kN"],
        spec("x", &["F * x", "x"]),
    );
    assert!(text.starts_with("in series 2:"), "{text}");
    assert!(text.contains("unit mismatch"), "{text}");
}

#[test]
fn a_plot_does_not_depend_on_where_its_data_is_defined() {
    // The plot is added after the lines here; evaluate with the lines reversed
    // and the answer is the same.
    let lines = ["a := 2", "x := 0..2", "y := a * x"];
    let (w, id) = sheet(&lines, spec("x", &["y"]));
    let forward = plot_of(&run(&w), id).unwrap();
    let mut regions = w.math_regions();
    regions.reverse();
    let out = evaluate_all(&regions, &w.plot_inputs(), &UnitRegistry::builtin());
    assert_eq!(plot_of(&out, id).unwrap(), forward);
    assert_eq!(
        forward.series[0].points,
        [(0.0, 0.0), (1.0, 2.0), (2.0, 4.0)]
    );
}

#[test]
fn vectors_and_function_results_can_be_plotted() {
    let (w, id) = sheet(
        &["x := 0..2", "f(t) := t ^ 3", "v := [5; 6; 7]"],
        spec("x", &["f(x)", "v"]),
    );
    let data = plot_of(&run(&w), id).unwrap();
    assert_eq!(data.series[0].points[2], (2.0, 8.0));
    assert_eq!(data.series[1].points[1], (1.0, 6.0));
}

#[test]
fn a_blank_plot_is_empty_not_an_error_and_blank_series_are_ignored() {
    let (w, id) = sheet(&[], PlotSpec::default());
    let data = plot_of(&run(&w), id).unwrap();
    assert!(data.is_empty());
    let (w, id) = sheet(&["x := 0..2"], spec("x", &["", "x", "  "]));
    assert_eq!(plot_of(&run(&w), id).unwrap().series.len(), 1);
}

fn error_text(lines: &[&str], p: PlotSpec) -> String {
    let (w, id) = sheet(lines, p);
    plot_of(&run(&w), id).unwrap_err().to_string()
}

#[test]
fn problems_are_reported_against_the_part_that_has_them() {
    let x = &["x := 0..2"];
    assert!(error_text(x, spec("", &["x"])).contains("enter the x values"));
    assert!(error_text(x, spec("nope", &["x"])).starts_with("in x:"));
    assert!(error_text(x, spec("x", &["x", "ghost"])).starts_with("in series 2:"));
    assert!(error_text(x, spec("x", &["5"])).starts_with("in series 1:"));
    assert!(error_text(x, spec("3", &["x"])).starts_with("in x:"));
    assert!(error_text(x, spec("x", &["x + i"])).contains("complex"));
    let longer = &["x := 0..2", "y := 0..3"];
    let text = error_text(longer, spec("x", &["y"]));
    assert!(text.contains("4 values but x has 3"), "{text}");
    // A parse error in a source is an error in that part, not a crash.
    assert!(error_text(x, spec("x", &["1 +"])).starts_with("in series 1:"));
}

#[test]
fn plots_are_edited_by_undoable_edits_that_merge_while_typing() {
    let (mut w, id) = sheet(&["x := 0..2"], PlotSpec::default());
    let mut h = History::default();
    let before = w.eval_revision();
    for typed in ["x", "x ^", "x ^ 2"] {
        let spec = PlotSpec::default().with_x("x").with_series(0, typed);
        h.apply(&mut w, Edit::SetPlot { id, spec }).unwrap();
    }
    assert!(
        w.eval_revision() > before,
        "editing a plot needs a recalculation"
    );
    // One undo step for the whole run of typing.
    h.undo(&mut w).unwrap();
    let RegionKind::Plot(p) = &w.region(id).unwrap().kind else {
        panic!("still a plot");
    };
    assert_eq!(p.series[0].source(), "");
    h.redo(&mut w).unwrap();
    let RegionKind::Plot(p) = &w.region(id).unwrap().kind else {
        panic!("still a plot");
    };
    assert_eq!(p.series[0].source(), "x ^ 2");
}

#[test]
fn only_plots_take_plot_edits() {
    let mut w = Worksheet::new("t");
    let m = add(&mut w, RegionKind::math("a := 1"));
    let err = w
        .apply(Edit::SetPlot {
            id: m,
            spec: PlotSpec::default(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("not the right kind"), "{err}");
}

#[test]
fn a_saved_worksheet_keeps_its_plot() {
    let (w, id) = sheet(&["x := 0..2"], spec("x", &["x ^ 2"]).with_title("Squares"));
    let json = serde_json::to_string(&w).unwrap();
    let back: Worksheet = serde_json::from_str(&json).unwrap();
    assert_eq!(back, w);
    let data = plot_of(&run(&back), id).unwrap();
    assert_eq!(data.series[0].points[2], (2.0, 4.0));
}

#[test]
fn the_worker_returns_plot_data_with_the_values() {
    let (w, id) = sheet(&["x := 0..2"], spec("x", &["x + 1"]));
    let worker = Worker::spawn(|| {});
    assert!(worker.request_full(
        1,
        w.math_regions(),
        w.plot_inputs(),
        UnitRegistry::builtin()
    ));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let result = loop {
        if let Some(r) = worker.poll_for(1) {
            break r;
        }
        assert!(std::time::Instant::now() < deadline, "no answer");
        std::thread::yield_now();
    };
    let data = result.output.plots[&id].clone().unwrap();
    assert_eq!(data.series[0].points[2], (2.0, 3.0));
}

// ---- options and styles ---------------------------------------------------------

use opencalc_core::document::{MathRegion, PlotOptions, SeriesKind, SeriesStyle};

fn with_options(base: PlotSpec, options: PlotOptions) -> PlotSpec {
    base.with_options(options)
}

fn limits(x_min: &str, x_max: &str, y_min: &str, y_max: &str) -> PlotOptions {
    PlotOptions {
        x_min: MathRegion::new(x_min),
        x_max: MathRegion::new(x_max),
        y_min: MathRegion::new(y_min),
        y_max: MathRegion::new(y_max),
        ..PlotOptions::default()
    }
}

#[test]
fn axis_limits_are_evaluated_in_si_and_blank_ones_are_automatic() {
    let base = spec("t", &["F"]);
    let (w, id) = sheet(
        &["t := 0 s, 1 s..5 s", "F := 2 N * t / s"],
        with_options(base, limits("", "3 s", "0 N", "")),
    );
    let data = plot_of(&run(&w), id).unwrap();
    assert_eq!(data.limits, [None, Some(3.0), Some(0.0), None]);
    assert!(!data.x_log && !data.y_log);
}

#[test]
fn a_limit_may_read_a_variable() {
    let (w, id) = sheet(
        &["x := 1..4", "top := 10"],
        with_options(spec("x", &["x ^ 2"]), limits("", "", "", "top")),
    );
    assert_eq!(plot_of(&run(&w), id).unwrap().limits[3], Some(10.0));
}

#[test]
fn a_limit_with_the_wrong_units_is_an_error_naming_it() {
    let (w, id) = sheet(
        &["t := 0 s, 1 s..3 s", "F := 1 N * t / s"],
        with_options(spec("t", &["F"]), limits("", "3 m", "", "")),
    );
    match plot_of(&run(&w), id) {
        Err(EvalError::InPlot { part, error }) => {
            assert_eq!(part, "x max");
            assert!(matches!(*error, EvalError::Unit(_)), "{error}");
        }
        other => panic!("{other:?}"),
    }
    // A plain number is not a force either.
    let (w, id) = sheet(
        &["t := 0 s, 1 s..3 s", "F := 1 N * t / s"],
        with_options(spec("t", &["F"]), limits("", "", "", "10")),
    );
    assert!(matches!(
        plot_of(&run(&w), id),
        Err(EvalError::InPlot { part, .. }) if part == "y max"
    ));
}

#[test]
fn a_limit_that_is_not_a_real_number_or_does_not_evaluate_is_an_error() {
    let (w, id) = sheet(
        &["x := 1..3"],
        with_options(spec("x", &["x"]), limits("", "", "", "2 + i")),
    );
    assert!(
        matches!(plot_of(&run(&w), id), Err(EvalError::InPlot { part, .. }) if part == "y max")
    );
    let (w, id) = sheet(
        &["x := 1..3"],
        with_options(spec("x", &["x"]), limits("", "", "nowhere", "")),
    );
    assert!(
        matches!(plot_of(&run(&w), id), Err(EvalError::InPlot { part, .. }) if part == "y min")
    );
}

#[test]
fn a_minimum_must_be_below_its_maximum() {
    let (w, id) = sheet(
        &["x := 1..3"],
        with_options(spec("x", &["x"]), limits("5", "2", "", "")),
    );
    assert!(matches!(
        plot_of(&run(&w), id),
        Err(EvalError::InPlot { part, error }) if part == "x limits" && matches!(*error, EvalError::Invalid(_))
    ));
}

#[test]
fn a_log_axis_needs_positive_data_and_positive_limits() {
    let log_y = PlotOptions {
        y_log: true,
        ..PlotOptions::default()
    };
    let (w, id) = sheet(
        &["x := 1..3"],
        with_options(spec("x", &["x - 1"]), log_y.clone()),
    );
    assert!(matches!(
        plot_of(&run(&w), id),
        Err(EvalError::InPlot { part, .. }) if part == "series 1"
    ));
    let (w, id) = sheet(&["x := 1..3"], with_options(spec("x", &["x"]), log_y));
    let data = plot_of(&run(&w), id).unwrap();
    assert!(data.y_log && !data.x_log);
    let log_x = PlotOptions {
        x_log: true,
        ..PlotOptions::default()
    };
    let (w, id) = sheet(&["x := 0..3"], with_options(spec("x", &["x"]), log_x));
    assert!(matches!(
        plot_of(&run(&w), id),
        Err(EvalError::InPlot { part, .. }) if part == "x"
    ));
    let bad_limit = PlotOptions {
        y_log: true,
        y_min: MathRegion::new("0"),
        ..PlotOptions::default()
    };
    let (w, id) = sheet(&["x := 1..3"], with_options(spec("x", &["x"]), bad_limit));
    assert!(matches!(
        plot_of(&run(&w), id),
        Err(EvalError::InPlot { part, .. }) if part == "y min"
    ));
}

#[test]
fn a_style_belongs_to_its_series_and_follows_it_when_one_is_removed() {
    let bar = SeriesStyle {
        kind: SeriesKind::Bar,
        ..SeriesStyle::default()
    };
    let stem = SeriesStyle {
        kind: SeriesKind::Stem,
        ..SeriesStyle::default()
    };
    let p = spec("x", &["a", "b", "c"])
        .with_style(0, bar.clone())
        .with_style(2, stem.clone());
    assert_eq!(p.styles.len(), 3);
    assert_eq!(
        (p.style(0), p.style(1), p.style(2)),
        (bar.clone(), SeriesStyle::default(), stem.clone())
    );
    let p = p.without_series(0);
    assert_eq!(p.series.len(), 2);
    assert_eq!((p.style(0), p.style(1)), (SeriesStyle::default(), stem));
    // Removing the style leaves nothing behind: equal to a plot never styled.
    let plain = spec("x", &["a", "b"]);
    assert_eq!(plain.clone().with_style(1, SeriesStyle::default()), plain);
    // A style for a series that does not exist is ignored.
    assert_eq!(plain.clone().with_style(5, bar), plain);
}

#[test]
fn options_and_styles_change_by_undoable_edits_and_restore_exactly() {
    let (mut w, id) = sheet(&["x := 1..3"], spec("x", &["x"]));
    let before = w.region(id).unwrap().kind.clone();
    let styled = spec("x", &["x"])
        .with_options(PlotOptions {
            grid: false,
            ..PlotOptions::default()
        })
        .with_style(
            0,
            SeriesStyle {
                kind: SeriesKind::Stem,
                name: "load".into(),
                ..SeriesStyle::default()
            },
        );
    let mut history = History::default();
    history
        .apply(
            &mut w,
            Edit::SetPlot {
                id,
                spec: styled.clone(),
            },
        )
        .unwrap();
    assert_eq!(w.region(id).unwrap().kind, RegionKind::Plot(styled));
    history.undo(&mut w).unwrap();
    assert_eq!(w.region(id).unwrap().kind, before);
}

#[test]
fn a_blank_series_does_not_shift_the_styles_of_the_ones_after_it() {
    // Series 0 is blank (not evaluated); the style of series 1 still applies to it.
    let stem = SeriesStyle {
        kind: SeriesKind::Stem,
        ..SeriesStyle::default()
    };
    let p = spec("x", &["", "x"]).with_style(1, stem.clone());
    assert_eq!(p.style(1), stem);
    let (w, id) = sheet(&["x := 1..3"], p);
    let data = plot_of(&run(&w), id).unwrap();
    assert_eq!(
        data.series.len(),
        1,
        "only the non-blank series is evaluated"
    );
}
