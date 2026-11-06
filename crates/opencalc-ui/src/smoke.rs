//! Headless end-to-end tests: build the whole app, run real frames with
//! synthetic input, and check what the user would see. No window needed.
//!
//! These guard the integration the unit tests cannot: that every command can
//! run, every region type paints, pointer and keyboard input reach the canvas,
//! undo works through the real input path, and the worker's results make it back
//! into the document view.

#![cfg(test)]

use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use opencalc_core::document::{PlotSpec, Point, RegionId, RegionKind};
use opencalc_core::edit::Edit;

use crate::app::{App, AppState};
use crate::canvas::adapt::{pos, rect_of};
use crate::commands::CommandId;
use crate::sheet::OpenSheet;
use crate::theme::PaletteKey;

struct Harness {
    ctx: egui::Context,
    app: App,
    time: f64,
}

impl Harness {
    fn new() -> Harness {
        let ctx = egui::Context::default();
        let app = App::with_state(ctx.clone(), AppState::with_sample(PaletteKey::Neutral));
        Harness {
            ctx,
            app,
            time: 0.0,
        }
    }

    fn frame_with(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 780.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let _ = self.ctx.run(input, |ctx| app.frame(ctx));
    }

    fn frame(&mut self) {
        self.frame_with(Vec::new());
    }

    /// Run frames until the worker's answer for the current revision is applied.
    fn settle(&mut self) {
        let end = Instant::now() + Duration::from_secs(10);
        for _ in 0..4 {
            self.frame();
        }
        while Instant::now() < end {
            self.frame();
            if self.app.state.status_text().contains("up to date") {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!(
            "worker result never arrived: {}",
            self.app.state.status_text()
        );
    }

    fn command(&mut self, command: CommandId) {
        self.app.queue.push(command);
        self.frame();
        self.frame();
    }

    fn sheet(&self) -> &OpenSheet {
        &self.app.state.sheets[self.app.state.active.expect("a worksheet is open")]
    }

    fn region_pos(&self, id: RegionId) -> Point {
        self.sheet().doc.region(id).expect("region exists").pos
    }

    /// Screen position of the middle of a region, from the canvas view.
    fn screen_centre(&self, id: RegionId) -> Pos2 {
        let sheet = self.sheet();
        let r = sheet.doc.region(id).expect("region exists");
        let centre = rect_of(r, &sheet.canvas.sizes).center();
        sheet
            .view
            .doc_to_screen(centre, self.app.state.viewport.min)
    }

    /// Press on a region, drag it 100 px right and 60 px down in steps, release.
    fn drag(&mut self, id: RegionId) {
        let from = self.screen_centre(id);
        self.frame_with(vec![Event::PointerMoved(from)]);
        self.frame_with(vec![press(from)]);
        let mut at = from;
        for _ in 0..10 {
            at += egui::vec2(10.0, 6.0);
            self.frame_with(vec![Event::PointerMoved(at)]);
        }
        self.frame_with(vec![release(at)]);
        self.frame();
    }

    fn click(&mut self, id: RegionId) {
        let p = self.screen_centre(id);
        self.frame_with(vec![Event::PointerMoved(p)]);
        self.frame_with(vec![press(p)]);
        self.frame_with(vec![release(p)]);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame_with(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame();
    }
}

fn press(pos: Pos2) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }
}

fn release(pos: Pos2) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }
}

/// The "F := 10 kN" region of the sample sheet: the third one created, after the
/// title text and the stress equation.
const F: RegionId = RegionId(3);

#[test]
fn switching_unit_system_redraws_results_without_recalculating() {
    let mut h = Harness::new();
    h.settle();
    let applied = h.app.state.calc.applied;
    for command in [
        CommandId::UnitsUscs,
        CommandId::UnitsCgs,
        CommandId::UnitsSi,
    ] {
        h.command(command);
        h.settle();
    }
    // Display only: the worker was not asked for anything.
    assert_eq!(h.app.state.calc.applied, applied);
    assert!(h.app.state.output.values.values().all(Result::is_ok));
}

#[test]
fn the_sample_sheet_renders_and_every_region_evaluates() {
    let mut h = Harness::new();
    h.settle();
    let sheet = h.sheet();
    let math = sheet.doc.math_regions().len();
    assert_eq!(math, 7);
    assert_eq!(h.app.state.output.values.len(), math);
    assert!(h.app.state.output.values.values().all(Result::is_ok));
    assert!(h.app.state.output.warnings.is_empty());
}

#[test]
fn every_command_runs_without_panicking() {
    let mut all: Vec<CommandId> = CommandId::ALL.to_vec();
    // The parameterised commands, in range and out of range.
    all.extend([
        CommandId::SelectSheet(0),
        CommandId::SelectSheet(9),
        CommandId::CloseSheet(0),
        CommandId::CloseSheet(9),
        CommandId::InsertSymbol('x'),
    ]);
    for command in all {
        let mut h = Harness::new();
        h.settle();
        h.app.queue.push(command);
        for _ in 0..3 {
            h.frame();
        }
    }
}

#[test]
fn inserting_each_region_type_adds_and_selects_it() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    let commands = [
        (CommandId::InsertMath, "Math"),
        (CommandId::InsertText, "Text"),
        (CommandId::InsertArea, "Area"),
        (CommandId::InsertPlot, "Plot"),
        (CommandId::InsertTable, "Table"),
        (CommandId::InsertPicture, "Picture"),
    ];
    for (n, (command, label)) in commands.iter().enumerate() {
        h.command(*command);
        let sheet = h.sheet();
        assert_eq!(sheet.doc.regions().len(), before + n + 1, "{command:?}");
        let newest = sheet.doc.regions().last().expect("a region");
        assert_eq!(newest.kind.label(), *label);
        assert_eq!(sheet.canvas.selection, vec![newest.id], "{command:?}");
    }
    h.settle();
}

#[test]
fn the_real_app_opens_one_blank_untitled_worksheet_with_no_panels() {
    let ctx = egui::Context::default();
    let app = App::new(ctx, PaletteKey::Neutral);
    assert_eq!(app.state.sheets.len(), 1);
    assert_eq!(app.state.sheets[0].title(), "Untitled");
    assert!(app.state.sheets[0].doc.regions().is_empty());
    assert!(!app.state.show_functions && !app.state.show_props);
}

#[test]
fn the_functions_window_opens_and_stays_within_its_width() {
    let mut h = Harness::new();
    h.settle();
    for _ in 0..3 {
        h.frame();
    }
    let without = h.app.state.viewport.min.x;
    h.command(CommandId::ViewFunctions);
    for _ in 0..4 {
        h.frame();
    }
    assert!(h.app.state.show_functions);
    let with = h.app.state.viewport.min.x;
    assert!(
        with > without + 100.0,
        "the window takes room: {without} -> {with}"
    );
    assert!(
        with <= 440.0,
        "the window stays within its width range: canvas at {with}"
    );
    h.command(CommandId::ViewFunctions);
    h.frame();
    assert!(!h.app.state.show_functions);
}

#[test]
fn the_plot_window_appears_only_while_a_plot_is_selected() {
    use crate::panels::plot_window::selected_plot;
    let mut h = Harness::new();
    h.settle();
    assert!(selected_plot(&h.app.state).is_none());
    let id = plot_id(&h);
    h.app.state.sheets[0].canvas.selection = vec![id];
    h.frame();
    h.frame();
    assert_eq!(selected_plot(&h.app.state).map(|(i, _)| i), Some(id));
    // Two regions selected, or a region that is not a plot: no plot window.
    let math = h.sheet().doc.regions()[0].id;
    h.app.state.sheets[0].canvas.selection = vec![id, math];
    assert!(selected_plot(&h.app.state).is_none());
    h.app.state.sheets[0].canvas.selection = vec![math];
    assert!(selected_plot(&h.app.state).is_none());
    h.frame();
}

/// The sample sheet's plot region.
fn plot_id(h: &Harness) -> RegionId {
    h.sheet()
        .doc
        .regions()
        .iter()
        .find(|r| matches!(r.kind, RegionKind::Plot(_)))
        .expect("the sample has a plot")
        .id
}

fn edit_plot(h: &mut Harness, spec: PlotSpec) {
    let id = plot_id(h);
    let i = h.app.state.active.expect("a sheet");
    let cmds = vec![
        crate::canvas::CanvasCmd::Edit(Edit::SetPlot { id, spec }),
        crate::canvas::CanvasCmd::Seal,
    ];
    let fx = crate::app::dispatch::apply_canvas(&mut h.app.state, i, cmds);
    assert_eq!(fx, vec![crate::app::dispatch::Effect::Recalculate]);
    h.app.recalculate();
    h.settle();
}

fn plot_points(h: &Harness) -> Vec<(f64, f64)> {
    let data = h.app.state.output.plots[&plot_id(h)]
        .as_ref()
        .expect("plot data");
    data.series[0].points.clone()
}

#[test]
fn the_sample_plot_is_evaluated_drawn_and_editable_with_undo() {
    let mut h = Harness::new();
    h.settle();
    // 11 widths from 50 mm to 150 mm, stress F / w^2 for F = 10 kN.
    let points = plot_points(&h);
    assert_eq!(points.len(), 11);
    assert!((points[0].0 - 0.05).abs() < 1e-12);
    assert!((points[0].1 - 10_000.0 / (0.05 * 0.05)).abs() < 1e-3);

    // Selecting it shows its editor in the properties panel; frames must not panic.
    let id = plot_id(&h);
    h.app.state.sheets[0].canvas.selection = vec![id];
    h.frame();
    h.frame();

    // Change a series; the worker recalculates and the drawn data follows.
    let RegionKind::Plot(spec) = h.sheet().doc.region(id).unwrap().kind.clone() else {
        panic!("a plot");
    };
    edit_plot(&mut h, spec.clone().with_series(0, "w"));
    let (x, y) = plot_points(&h)[3];
    assert!((x - 0.08).abs() < 1e-12 && (y - 0.08).abs() < 1e-12);

    // A bad source is an error shown in the region, not a crash.
    edit_plot(&mut h, spec.clone().with_series(0, "nope"));
    assert!(h.app.state.output.plots[&id].is_err());
    h.frame();

    // Undo brings back the previous plot, then the original.
    h.command(CommandId::EditUndo);
    h.settle();
    h.command(CommandId::EditUndo);
    h.settle();
    assert_eq!(plot_points(&h).len(), 11);
    assert!((plot_points(&h)[0].1 - 10_000.0 / (0.05 * 0.05)).abs() < 1e-3);
}

#[test]
fn a_new_plot_is_blank_and_shows_a_hint_without_error() {
    let mut h = Harness::new();
    h.command(CommandId::InsertPlot);
    h.settle();
    let id = h.sheet().canvas.selection[0];
    let data = h.app.state.output.plots[&id]
        .as_ref()
        .expect("blank is not an error");
    assert!(data.is_empty());
    h.frame();
}

#[test]
fn clicking_selects_and_dragging_moves_a_region() {
    let mut h = Harness::new();
    h.settle();
    let start = h.region_pos(F);
    h.drag(F);

    assert_eq!(
        h.sheet().canvas.selection,
        vec![F],
        "the dragged region is selected"
    );
    let moved = h.region_pos(F);
    // Snapped to the grid, and moved about (100, 60) from where it began.
    let grid = crate::canvas::view::GRID;
    assert_eq!(moved.x % grid, 0.0);
    assert_eq!(moved.y % grid, 0.0);
    assert!(
        (moved.x - start.x - 100.0).abs() <= grid,
        "x: {moved:?} from {start:?}"
    );
    assert!(
        (moved.y - start.y - 60.0).abs() <= grid,
        "y: {moved:?} from {start:?}"
    );
}

#[test]
fn a_whole_drag_is_one_undo_step_and_redo_puts_it_back() {
    let mut h = Harness::new();
    h.settle();
    let start = h.region_pos(F);
    h.drag(F);
    let moved = h.region_pos(F);
    assert_ne!(moved, start);

    h.command(CommandId::EditUndo);
    assert_eq!(h.region_pos(F), start, "one undo reverses the whole drag");
    assert!(!h.sheet().can_undo(), "the drag was a single step");
    h.command(CommandId::EditRedo);
    assert_eq!(h.region_pos(F), moved);
}

#[test]
fn moving_a_region_does_not_recalculate() {
    let mut h = Harness::new();
    h.settle();
    let results_before: Vec<_> = {
        let mut v: Vec<_> = h
            .app
            .state
            .output
            .values
            .iter()
            .map(|(k, v)| (k.0, v.clone()))
            .collect();
        v.sort_by_key(|(k, _)| *k);
        v
    };
    let eval_before = h.sheet().doc.eval_revision();
    let requests_before = h.app.state.calc.revision;
    h.drag(F);
    assert_eq!(
        h.app.state.calc.revision, requests_before,
        "dragging must not ask the worker to recalculate"
    );
    assert_eq!(
        h.sheet().doc.eval_revision(),
        eval_before,
        "a move is not an evaluation change"
    );
    assert!(
        h.app.state.status_text().contains("up to date"),
        "no recalculation was started"
    );
    let mut results_after: Vec<_> = h
        .app
        .state
        .output
        .values
        .iter()
        .map(|(k, v)| (k.0, v.clone()))
        .collect();
    results_after.sort_by_key(|(k, _)| *k);
    assert_eq!(results_before, results_after);
}

#[test]
fn ctrl_z_and_ctrl_y_work_from_the_keyboard() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    h.command(CommandId::InsertPlot); // a plot: no text field takes the keyboard
    assert_eq!(h.sheet().doc.regions().len(), before + 1);

    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(
        h.sheet().doc.regions().len(),
        before,
        "Ctrl+Z removed the plot"
    );
    h.key(Key::Y, Modifiers::COMMAND);
    assert_eq!(
        h.sheet().doc.regions().len(),
        before + 1,
        "Ctrl+Y brought it back"
    );
    h.key(Key::Z, Modifiers::COMMAND);
    h.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(
        h.sheet().doc.regions().len(),
        before + 1,
        "Ctrl+Shift+Z also redoes"
    );
}

#[test]
fn delete_removes_the_selection_dependents_error_and_undo_restores_everything() {
    let mut h = Harness::new();
    h.settle();
    let order_before: Vec<RegionId> = h.sheet().doc.regions().iter().map(|r| r.id).collect();
    h.click(F);
    assert_eq!(h.sheet().canvas.selection, vec![F]);
    h.command(CommandId::RegionDelete);
    h.settle();
    assert!(h.sheet().doc.region(F).is_none());
    assert!(h.sheet().canvas.selection.is_empty());
    assert!(
        h.app.state.output.values.values().any(Result::is_err),
        "a region that used F should now be an error"
    );

    h.command(CommandId::EditUndo);
    h.settle();
    let order_after: Vec<RegionId> = h.sheet().doc.regions().iter().map(|r| r.id).collect();
    assert_eq!(
        order_after, order_before,
        "back in the same z-order position"
    );
    assert!(
        h.app.state.output.values.values().all(Result::is_ok),
        "undoing the delete recalculated and everything evaluates again"
    );
}

#[test]
fn typing_into_a_region_is_one_undo_step_and_recalculates_live() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::InsertMath); // opens the new region for editing with focus
    h.frame();
    for ch in ["q", " ", ":", "=", " ", "7"] {
        h.frame_with(vec![Event::Text(ch.to_owned())]);
    }
    let id = *h
        .sheet()
        .canvas
        .selection
        .first()
        .expect("the new region is selected");
    let source = |h: &Harness| match &h.sheet().doc.region(id).expect("region").kind {
        RegionKind::Math(m) => m.source().to_owned(),
        _ => panic!("math region"),
    };
    assert_eq!(source(&h), "q := 7");
    h.settle();
    assert!(
        h.app.state.output.values[&id].is_ok(),
        "live recalculation evaluated what was typed"
    );

    // Finish editing (Enter), then undo: the whole burst of typing goes at once.
    h.key(Key::Enter, Modifiers::NONE);
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(source(&h), "", "one undo reverts all the typing");
}

#[test]
fn switching_sheets_never_shows_another_sheets_results() {
    // Region IDs restart at 1 on every sheet, so applying a stale result from
    // the previous sheet would show wrong values on the new one.
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::FileNew);
    h.settle();
    assert!(
        h.app.state.output.values.is_empty(),
        "{:?}",
        h.app.state.output.values
    );
}

#[test]
fn each_sheet_keeps_its_own_undo_history() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::InsertPlot);
    assert!(h.sheet().can_undo());
    h.command(CommandId::FileNew);
    assert!(!h.sheet().can_undo(), "a new sheet starts with no history");
    h.command(CommandId::SelectSheet(0));
    assert!(
        h.sheet().can_undo(),
        "the first sheet still has its history"
    );
}

/// The Windows main thread has a 1 MB stack, and a debug build uses a lot of it.
/// Paint a frame containing the worst-case shapes at the symbol limit on exactly
/// that much stack. Overflowing aborts the test binary, so finishing is the proof.
#[test]
fn worst_case_regions_paint_within_the_main_thread_stack() {
    let limit = opencalc_core::parser::MAX_SYMBOLS;
    let half = (limit - 1) / 2;
    let shapes = [
        format!("{}1", "-".repeat(limit - 1)),
        format!("{}1{}", "(".repeat(half), ")".repeat(half)),
        format!("1{}", "+1".repeat(half)),
        format!("2{}", "^2".repeat(half)),
        format!("1{}", "/1".repeat(half)),
        format!("{}x{}", "sqrt(".repeat(half / 2), ")".repeat(half / 2)),
    ];
    let handle = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            let mut h = Harness::new();
            let mut sheet = OpenSheet::new("worst case");
            for (i, src) in shapes.iter().enumerate() {
                sheet.add(
                    RegionKind::math(src),
                    Point::new(20.0, 20.0 + 80.0 * i as f32),
                );
            }
            h.app.state.sheets.push(sheet);
            h.command(CommandId::SelectSheet(1));
            h.settle();
            // Every region either evaluated or reported an error; none was lost.
            assert_eq!(h.app.state.output.values.len(), 6);
        })
        .expect("spawn");
    handle.join().expect("the frame must not overflow or panic");
}

#[test]
fn conversions_place_regions_where_the_document_says() {
    // A document position survives the trip through the view into screen space
    // and back, so what the user clicks is what the document holds.
    let mut h = Harness::new();
    h.settle();
    let sheet = h.sheet();
    let r = sheet.doc.region(F).expect("F");
    let screen = sheet
        .view
        .doc_to_screen(pos(r.pos), h.app.state.viewport.min);
    let back = sheet.view.screen_to_doc(screen, h.app.state.viewport.min);
    assert!((back - pos(r.pos)).length() < 1e-3);
}

/// SPEC 5.6: pan and zoom at 60 fps with 200 visible regions. This measures the
/// work the UI does every frame (typesetting, painting into shapes, input) with
/// all 200 regions on screen. It excludes tessellation and the GPU, which egui
/// does outside `ctx.run`, so it is a lower bound on a real frame.
///
/// Ignored by default because timing means nothing in a debug build. Run:
///   cargo test --release -p opencalc-ui perf -- --ignored --nocapture
#[test]
#[ignore = "performance: run in release with --ignored --nocapture"]
fn perf_frame_time_with_200_visible_regions() {
    let mut h = Harness::new();
    let mut sheet = OpenSheet::new("perf");
    for i in 0..200 {
        // 8 columns by 25 rows: wide enough to be realistic, compact enough that
        // "fit" (minimum zoom 25%) can show all 200 in the open canvas.
        let (col, row) = (i % 8, i / 8);
        let at = Point::new(col as f32 * 330.0, row as f32 * 60.0);
        sheet.add(RegionKind::math(&format!("v{i} := {} m * 2", i + 1)), at);
    }
    h.app.state.sheets.push(sheet);
    h.command(CommandId::SelectSheet(1));
    h.settle();
    // Fit twice: text measured at 100% differs a little from text measured at the
    // fitted zoom (font rounding at small sizes), so the second fit uses the
    // sizes the sheet really has when drawn small.
    for _ in 0..2 {
        h.command(CommandId::ViewZoomFit);
        h.settle();
    }

    // All 200 must really be on screen, or the number below means less.
    let sheet = h.sheet();
    let viewport = h.app.state.viewport;
    let on_screen = sheet
        .doc
        .regions()
        .iter()
        .filter(|r| {
            let rect = sheet
                .view
                .rect_to_screen(rect_of(r, &sheet.canvas.sizes), viewport.min);
            viewport.contains_rect(rect)
        })
        .count();
    assert_eq!(
        on_screen, 200,
        "zoom {}: not every region is visible",
        sheet.view.zoom
    );

    let mut times: Vec<Duration> = (0..80)
        .map(|_| {
            let start = Instant::now();
            h.frame();
            start.elapsed()
        })
        .collect();
    times.sort();
    let (min, median, max) = (times[0], times[times.len() / 2], times[times.len() - 1]);
    println!(
        "\nframe with 200 visible regions at {:.0}% zoom: median {:.2} ms, best {:.2} ms, worst {:.2} ms (budget 16.7 ms)\n",
        h.sheet().view.zoom * 100.0,
        median.as_secs_f64() * 1e3,
        min.as_secs_f64() * 1e3,
        max.as_secs_f64() * 1e3
    );
}
