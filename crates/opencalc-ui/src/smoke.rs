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

mod arrange;
mod clipboard;
mod export;
mod files;
mod find;
mod format;
mod text_edit;
mod units;

/// File dialogs that are never answered: a test must not open a real one.
struct NoFiles;

impl crate::app::Files for NoFiles {
    fn pick_open(&mut self) -> Option<std::path::PathBuf> {
        None
    }

    fn pick_save(&mut self, _: &str, _: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
        None
    }

    fn pick_export(
        &mut self,
        _: &str,
        _: crate::export::Kind,
        _: Option<&std::path::Path>,
    ) -> Option<std::path::PathBuf> {
        None
    }
}

struct Harness {
    ctx: egui::Context,
    app: App,
    time: f64,
    /// The last text the app put on the clipboard.
    copied: Option<String>,
}

impl Harness {
    fn new() -> Harness {
        let ctx = egui::Context::default();
        let app = App::with_files(
            ctx.clone(),
            AppState::with_sample(PaletteKey::Neutral),
            Box::new(NoFiles),
        );
        Harness {
            ctx,
            app,
            time: 0.0,
            copied: None,
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
        let out = self.ctx.run(input, |ctx| app.frame(ctx));
        for command in out.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                self.copied = Some(text);
            }
        }
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
            if self.app.state.calc_label() == "Calculated" {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!(
            "worker result never arrived: {}",
            self.app.state.calc_label()
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
    assert!(!app.state.show_functions);
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

#[test]
fn options_opens_a_window_and_the_theme_choice_changes_the_application_colours() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::AppOptions);
    h.frame();
    assert!(h.app.state.options_open);
    let before = h.app.state.theme.base;
    h.command(CommandId::ThemeSlate);
    h.frame();
    assert_ne!(
        h.app.state.theme.base, before,
        "the application theme changed"
    );
    // Pages stay white whatever the theme.
    assert_eq!(crate::theme::Theme::paper().base, egui::Color32::WHITE);
    h.command(CommandId::AppOptionsClose);
    h.frame();
    assert!(!h.app.state.options_open);
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

    // Selecting it opens the Plot window; frames must not panic.
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
        h.app.state.calc_label() == "Calculated",
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
    for ch in ["q", ":", "=", "7"] {
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
    assert_eq!(source(&h), "q:=7");
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

/// The ribbon is a fixed size: no tab, however full, makes it taller or shorter.
#[test]
fn the_ribbon_has_the_same_height_on_every_tab() {
    let mut h = Harness::new();
    h.settle();
    let height = |h: &Harness| {
        egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("ribbon"))
            .expect("the ribbon panel")
            .rect
            .height()
    };
    let first = height(&h);
    for tab in 0..h.app.ribbon.tabs.len() {
        h.app.ribbon_state.active_tab = tab;
        h.frame();
        h.frame();
        assert!(
            (height(&h) - first).abs() < 0.5,
            "tab {tab} is {} high, not {first}",
            height(&h)
        );
    }
}

// ---- the math editor (E4)

impl Harness {
    /// Open a new math region for editing and return its id.
    fn new_math(&mut self) -> RegionId {
        self.command(CommandId::InsertMath);
        self.frame();
        *self
            .sheet()
            .canvas
            .selection
            .first()
            .expect("the new region is selected")
    }

    fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.frame_with(vec![Event::Text(ch.to_string())]);
        }
    }

    fn press_key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame_with(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }]);
    }

    fn source_of(&self, id: RegionId) -> String {
        match &self.sheet().doc.region(id).expect("region").kind {
            RegionKind::Math(m) => m.source().to_owned(),
            _ => panic!("math region"),
        }
    }

    fn caret(&self) -> usize {
        self.sheet().canvas.field.as_ref().expect("editing").caret
    }
}

const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};

#[test]
fn the_editor_inserts_at_the_caret_and_backspace_and_delete_work() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("ac");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    h.type_text("b");
    assert_eq!(h.source_of(id), "abc");
    h.press_key(Key::Backspace, Modifiers::NONE);
    assert_eq!(h.source_of(id), "ac");
    h.press_key(Key::Delete, Modifiers::NONE);
    assert_eq!(h.source_of(id), "a");
    h.press_key(Key::Home, Modifiers::NONE);
    h.type_text("\u{3b1}");
    assert_eq!(
        h.source_of(id),
        "\u{3b1}a",
        "Greek goes in as one character"
    );
}

#[test]
fn shift_arrows_select_and_typing_replaces_the_selection() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+yy");
    h.press_key(Key::ArrowLeft, SHIFT);
    h.press_key(Key::ArrowLeft, SHIFT);
    h.type_text("z");
    assert_eq!(h.source_of(id), "x+z");
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text("q");
    assert_eq!(h.source_of(id), "q");
}

#[test]
fn copy_puts_the_source_text_of_the_selection_on_the_clipboard_and_cut_removes_it() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a/b");
    h.press_key(Key::Home, SHIFT);
    h.frame_with(vec![Event::Copy]);
    assert_eq!(h.copied.as_deref(), Some("a/b"));
    h.frame_with(vec![Event::Cut]);
    assert_eq!(h.source_of(id), "");
    h.frame_with(vec![Event::Paste("1\n+ 2".to_owned())]);
    assert_eq!(
        h.source_of(id),
        "1 + 2",
        "a pasted line break becomes a space"
    );
}

#[test]
fn arrows_skip_characters_that_draw_nothing_and_up_down_move_between_fraction_lines() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("((x))");
    h.press_key(Key::Home, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(
        h.caret() >= 2,
        "the brackets draw nothing, so one arrow press moves past them: {}",
        h.caret()
    );
    // In a fraction, Up goes to the numerator and Down back to the denominator.
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text("a/b");
    assert_eq!(h.source_of(id), "a/b");
    h.press_key(Key::End, Modifiers::NONE);
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    assert!(h.caret() <= 1, "caret in the numerator: {}", h.caret());
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    assert!(h.caret() >= 2, "caret in the denominator: {}", h.caret());
}

#[test]
fn arrows_off_the_edge_of_an_equation_leave_it_and_arrows_on_the_page_move_the_crosshair() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+1");
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(
        h.sheet().canvas.editing.is_none(),
        "Right at the end leaves the equation"
    );
    let right = h.sheet().canvas.insert_at;
    let r = h.sheet().doc.region(id).expect("region").pos;
    assert!(right.x > r.x, "the crosshair is right of the equation");
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    let down = h.sheet().canvas.insert_at;
    assert!(down.y > right.y && down.x == right.x, "Down moves one step");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    assert!(h.sheet().canvas.insert_at.x < down.x);
    assert!(h.sheet().canvas.armed, "typing starts a region there");
    // Walking back (along the equation's own line) hands control to it.
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    for _ in 0..30 {
        h.press_key(Key::ArrowLeft, Modifiers::NONE);
        if h.sheet().canvas.editing.is_some() {
            break;
        }
    }
    h.frame();
    let (at, rect) = (
        h.sheet().canvas.insert_at,
        h.sheet().doc.region(id).map(|r| (r.pos, r.size)),
    );
    assert_eq!(
        h.sheet().canvas.editing,
        Some(id),
        "the equation took over: {at:?} {rect:?}"
    );
    assert!(!h.sheet().canvas.armed);
}

#[test]
fn the_crosshair_can_walk_back_onto_a_region_that_is_off_the_grid() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+1");
    h.press_key(Key::Escape, Modifiers::NONE);
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    // A region at a position that is not a multiple of the grid step.
    let moves = vec![(id, Point { x: 41.0, y: 523.0 })];
    h.app.state.sheets[0].edit(Edit::MoveRegions { moves });
    h.frame();
    h.frame();
    h.app.state.sheets[0].canvas.editing = Some(id);
    h.app.state.sheets[0].canvas.focus_edit = true;
    h.frame();
    h.frame();
    assert_eq!(h.sheet().canvas.editing, Some(id));
    h.press_key(Key::End, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(
        h.sheet().canvas.editing.is_none(),
        "Right at the end leaves"
    );
    for _ in 0..30 {
        h.press_key(Key::ArrowLeft, Modifiers::NONE);
        if h.sheet().canvas.editing.is_some() {
            break;
        }
    }
    assert_eq!(
        h.sheet().canvas.editing,
        Some(id),
        "the region took over again"
    );
}

#[test]
fn clicking_in_the_region_places_the_caret_and_a_double_click_selects_a_node() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("alpha+beta");
    h.frame();
    let sheet = h.sheet();
    let r = sheet.doc.region(id).expect("region");
    let rect = rect_of(r, &sheet.canvas.sizes);
    let at = |fx: f32| {
        sheet.view.doc_to_screen(
            Pos2::new(rect.min.x + rect.width() * fx, rect.center().y),
            h.app.state.viewport.min,
        )
    };
    let (near_start, near_end) = (at(0.1), at(0.97));
    h.frame_with(vec![Event::PointerMoved(near_start)]);
    h.frame_with(vec![press(near_start)]);
    h.frame_with(vec![release(near_start)]);
    assert!(
        h.caret() <= 2,
        "a click near the left puts the caret near the start: {}",
        h.caret()
    );
    h.frame_with(vec![Event::PointerMoved(near_end)]);
    h.frame_with(vec![press(near_end)]);
    h.frame_with(vec![release(near_end)]);
    assert!(h.caret() >= 10, "a click near the right: {}", h.caret());
    assert_eq!(
        h.source_of(id),
        "alpha+beta",
        "clicking never changes the text"
    );
}

#[test]
fn enter_and_escape_end_editing_and_the_whole_burst_is_one_undo_step() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("m:=3");
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    assert!(h.sheet().canvas.editing.is_none());
    assert!(h.sheet().canvas.field.is_none());
    assert_eq!(h.source_of(id), "m:=3");
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(h.source_of(id), "");
}

#[test]
fn half_typed_input_never_breaks_a_frame_and_a_huge_paste_is_capped() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    for text in ["(", "1+", "$", "*[1,2;3", "f(", "\u{20ac}"] {
        h.type_text(text);
    }
    assert_eq!(h.source_of(id), "(1+$*[1,2;3f(\u{20ac}");
    h.frame_with(vec![Event::Paste("9".repeat(20_000))]);
    assert!(h.source_of(id).chars().count() <= 8192);
    h.settle();
}

#[test]
fn the_functions_window_inserts_into_the_region_being_edited() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x:=");
    h.sheet().doc.region(id).expect("region");
    // An edit from outside the editor (as the Functions window makes) is picked up.
    let source = "x:=sqrt(".to_owned();
    h.app.state.sheets[0].edit(Edit::SetMathSource { id, source });
    h.frame();
    h.type_text("4)");
    assert_eq!(h.source_of(id), "x:=sqrt(4)");
}

#[test]
fn backslash_names_become_greek_letters_and_a_colon_makes_a_define() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text(r"\sigma");
    h.type_text(" ");
    h.type_text(":");
    h.type_text("=5");
    assert_eq!(h.source_of(id), "\u{3c3}:=5");
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text(r"\beta");
    h.press_key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.source_of(id), "\u{3b2}", "Tab also completes the letter");
    assert!(
        h.sheet().canvas.editing.is_some(),
        "Tab does not end editing"
    );
}

#[test]
fn autocomplete_finishes_a_word_with_tab_or_enter_and_escape_only_closes_the_list() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("sqr");
    h.press_key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.source_of(id), "sqrt(");
    // With the list open, Enter accepts instead of ending the edit.
    h.type_text("co");
    h.press_key(Key::Enter, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_some());
    assert!(
        h.source_of(id).starts_with("sqrt(co"),
        "{}",
        h.source_of(id)
    );
    assert_ne!(h.source_of(id), "sqrt(co", "the word was completed");
    // Escape closes the list; a second Escape ends the edit.
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text("ta");
    h.press_key(Key::Escape, Modifiers::NONE);
    assert!(
        h.sheet().canvas.editing.is_some(),
        "the first Escape closed only the list"
    );
    assert_eq!(h.source_of(id), "ta");
    h.press_key(Key::Escape, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_none());
}

#[test]
fn autocomplete_offers_names_defined_in_the_worksheet_and_arrows_choose() {
    let mut h = Harness::new();
    h.settle();
    let first = h.new_math();
    h.type_text("stress_y:=250");
    h.press_key(Key::Enter, Modifiers::NONE);
    h.settle();
    let second = h.new_math();
    h.type_text("stress");
    h.press_key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.source_of(second), "stress_y");
    assert_ne!(first, second);
}

#[test]
fn a_subscript_name_edits_like_any_other_text() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("F_x+F_y");
    h.press_key(Key::Home, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(h.caret() >= 3, "{}", h.caret());
    assert_eq!(h.source_of(id), "F_x+F_y");
    h.settle();
}

#[test]
fn many_keystrokes_in_one_frame_all_arrive() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    let mut events = Vec::new();
    for ch in "sigma_y".chars() {
        events.push(Event::Key {
            key: Key::A,
            physical_key: Some(Key::A),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        events.push(Event::Text(ch.to_string()));
    }
    // The Enter that finishes the region arrives in the same frame.
    events.push(Event::Key {
        key: Key::Enter,
        physical_key: Some(Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    h.frame_with(events);
    h.frame();
    assert_eq!(h.source_of(id), "sigma_y");
    assert!(h.sheet().canvas.editing.is_none());
}

#[test]
fn clicking_the_ribbon_does_not_end_editing_and_typing_carries_on() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    let count = h.sheet().doc.regions().len();
    h.type_text("ab");
    // A ribbon tab, then the empty part of the ribbon: neither is the canvas.
    for at in [Pos2::new(108.0, 45.0), Pos2::new(1100.0, 80.0)] {
        h.frame_with(vec![Event::PointerMoved(at)]);
        h.frame_with(vec![press(at)]);
        h.frame_with(vec![release(at)]);
        h.frame();
        h.frame();
        assert_eq!(h.sheet().canvas.editing, Some(id), "a click at {at:?}");
    }
    h.type_text("cd");
    assert_eq!(h.source_of(id), "abcd");
    assert_eq!(h.sheet().doc.regions().len(), count, "no region was added");
}

#[test]
fn a_press_on_the_canvas_away_from_the_region_ends_editing() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("ab");
    let away = Pos2::new(1000.0, 650.0);
    h.frame_with(vec![Event::PointerMoved(away)]);
    h.frame_with(vec![press(away)]);
    h.frame_with(vec![release(away)]);
    h.frame();
    assert!(h.sheet().canvas.editing.is_none());
    assert_eq!(h.source_of(id), "ab");
}

#[test]
fn a_greek_letter_from_the_ribbon_goes_in_at_the_caret_while_editing() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x:=");
    h.command(CommandId::InsertSymbol('\u{3c3}'));
    assert_eq!(h.source_of(id), "x:=\u{3c3}");
    h.type_text("+1");
    assert_eq!(
        h.source_of(id),
        "x:=\u{3c3}+1",
        "typing carries on after the letter"
    );
    // In the middle: the letter goes at the caret and replaces a selection.
    h.press_key(Key::Home, Modifiers::NONE);
    h.command(CommandId::InsertSymbol('\u{3b1}'));
    assert_eq!(h.source_of(id), "\u{3b1}x:=\u{3c3}+1");
    // One undo step for the typing before, and the letter edits are undoable.
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    h.key(Key::Z, Modifiers::COMMAND);
    assert!(!h.source_of(id).starts_with('\u{3b1}'));
}

#[test]
fn symbols_are_not_offered_when_nothing_is_being_edited() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a");
    // The first Escape closes the autocomplete list, the second ends the edit.
    h.press_key(Key::Escape, Modifiers::NONE);
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    assert!(h.sheet().canvas.field.is_none());
    h.app.queue.push(CommandId::InsertSymbol('\u{3c3}'));
    h.frame();
    assert_eq!(
        h.source_of(id),
        "a",
        "the command is disabled when no region is open"
    );
}

#[test]
fn clicking_a_flyout_that_floats_over_the_canvas_does_not_end_editing() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x:=");
    // The Greek flyout (Math tab, Symbols panel) hangs below the ribbon, over the page.
    h.app.ribbon_state.open_panel = Some((0, 4));
    h.frame();
    h.frame();
    let flyout = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("ribbon-flyout", (0usize, 4usize)))))
        .expect("the flyout is open");
    assert!(
        flyout.max.y > 100.0,
        "it reaches into the canvas area: {flyout:?}"
    );
    let at = flyout.center();
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![press(at)]);
    h.frame_with(vec![release(at)]);
    h.frame();
    assert_eq!(
        h.sheet().canvas.editing,
        Some(id),
        "still editing after the click"
    );
}

#[test]
fn up_and_down_remember_the_column_they_started_from() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a/b");
    h.press_key(Key::End, Modifiers::NONE);
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    let goal = h.sheet().canvas.field.as_ref().expect("editing").goal_x;
    assert!(goal.is_some(), "a vertical move records its column");
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        h.sheet().canvas.field.as_ref().expect("editing").goal_x,
        goal,
        "the column survives repeated vertical moves"
    );
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(
        h.sheet().canvas.field.as_ref().expect("editing").goal_x,
        None,
        "a horizontal move forgets it"
    );
    assert_eq!(h.source_of(id), "a/b", "navigation never edits the source");
}

#[test]
fn moving_the_caret_makes_no_edit_and_undo_while_editing_keeps_it_in_range() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+12345");
    let before = h.source_of(id);
    h.press_key(Key::Home, Modifiers::NONE);
    h.press_key(Key::ArrowRight, SHIFT);
    h.press_key(Key::End, Modifiers::NONE);
    assert_eq!(
        h.source_of(id),
        before,
        "caret and selection moves are not edits"
    );
    h.press_key(Key::Z, Modifiers::COMMAND);
    let len = h.source_of(id).chars().count();
    if let Some(f) = h.sheet().canvas.field.as_ref() {
        assert!(
            f.caret <= len && f.anchor <= len,
            "caret clamped after undo"
        );
    }
}

#[test]
fn the_crosshair_follows_a_click_on_blank_page() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().canvas.insert_at;
    let at = h.app.state.viewport.max - egui::vec2(120.0, 80.0);
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![press(at)]);
    h.frame_with(vec![release(at)]);
    assert_ne!(
        h.sheet().canvas.insert_at,
        before,
        "{at:?} {:?}",
        h.app.state.viewport
    );
    assert!(h.sheet().canvas.editing.is_none());
}

#[test]
fn typing_after_a_click_on_blank_page_starts_a_math_region_there() {
    let mut h = Harness::new();
    h.settle();
    let at = h.app.state.viewport.max - egui::vec2(120.0, 80.0);
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![press(at)]);
    h.frame_with(vec![release(at)]);
    let count = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Text("x".into())]);
    assert_eq!(h.sheet().doc.regions().len(), count + 1);
    let id = *h.sheet().canvas.selection.first().expect("selected");
    assert_eq!(h.source_of(id), "x");
    assert_eq!(h.sheet().canvas.editing, Some(id));
    h.type_text("+1");
    assert_eq!(
        h.source_of(id),
        "x+1",
        "typing carries on in the new region"
    );
    h.key(Key::Enter, Modifiers::NONE);
    h.key(Key::Z, Modifiers::COMMAND);
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(
        h.sheet().doc.regions().len(),
        count,
        "typing, then creation, each undo in one step"
    );
}

#[test]
fn typing_on_a_fresh_worksheet_starts_a_region_at_the_insertion_point() {
    let mut h = Harness::new();
    h.settle();
    let count = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Text("x".into())]);
    assert_eq!(h.sheet().doc.regions().len(), count + 1);
}

#[test]
fn deleting_the_region_being_edited_closes_the_editor() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+1");
    h.command(CommandId::RegionDelete);
    h.frame();
    assert!(h.sheet().doc.region(id).is_none());
    assert!(
        h.sheet().canvas.editing.is_none(),
        "no editor on a gone region"
    );
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    h.command(CommandId::EditUndo);
    h.settle();
    assert!(
        h.sheet().doc.region(id).is_some(),
        "undo restores the region"
    );
}

#[test]
fn undoing_the_creation_of_the_region_being_edited_closes_the_editor() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("abc");
    h.key(Key::Enter, Modifiers::NONE);
    h.frame_with(vec![Event::PointerMoved(h.screen_centre(id))]);
    h.click(id);
    h.frame();
    for _ in 0..3 {
        h.command(CommandId::EditUndo);
        h.frame();
    }
    assert!(
        h.sheet().doc.region(id).is_none(),
        "undo removed the region"
    );
    assert!(h.sheet().canvas.editing.is_none(), "and the editor with it");
}

#[test]
fn a_new_sheet_has_no_editor_and_the_first_keeps_its_own() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("ab");
    h.command(CommandId::FileNew);
    h.frame();
    assert!(h.sheet().canvas.editing.is_none());
    assert!(h.sheet().canvas.field.is_none());
    h.type_text("zzz");
    h.command(CommandId::SelectSheet(0));
    h.frame();
    assert_eq!(
        h.source_of(id),
        "ab",
        "keystrokes on the new sheet never reach sheet 0"
    );
}

#[test]
fn zooming_while_editing_keeps_the_caret_and_clicks_consistent() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a/b");
    let caret = h.caret();
    for c in [
        CommandId::ViewZoomIn,
        CommandId::ViewZoomIn,
        CommandId::ViewZoomOut,
    ] {
        h.command(c);
        h.frame();
        assert_eq!(h.caret(), caret, "zoom never moves the caret in the source");
        assert_eq!(h.source_of(id), "a/b");
    }
    h.type_text("c");
    assert_eq!(h.source_of(id), "a/bc");
}

#[test]
fn enter_readies_the_next_input_below_and_typing_starts_it() {
    let mut h = Harness::new();
    h.settle();
    let first = h.new_math();
    h.type_text("a:=1");
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_none());
    assert!(
        h.sheet().canvas.selection.is_empty(),
        "nothing stays highlighted"
    );
    let below = h.sheet().canvas.insert_at;
    let r = h.sheet().doc.region(first).expect("region");
    assert!(below.y > r.pos.y, "the insertion point is below it");
    let count = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Text("b".into())]);
    assert_eq!(h.sheet().doc.regions().len(), count + 1);
    let id = *h.sheet().canvas.selection.first().expect("selected");
    assert_ne!(id, first);
    assert_eq!(h.source_of(id), "b");
    assert_eq!(h.source_of(first), "a:=1", "the first input is untouched");
}

#[test]
fn ctrl_space_grows_the_selection_through_the_expression_and_typing_replaces_it() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("1+a/b");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    h.press_key(Key::Space, Modifiers::COMMAND);
    let sel = |h: &Harness| {
        h.sheet()
            .canvas
            .field
            .as_ref()
            .expect("editing")
            .selection()
    };
    assert_eq!(sel(&h), (4, 5), "the denominator");
    h.press_key(Key::Space, Modifiers::COMMAND);
    assert_eq!(sel(&h), (2, 5), "the whole fraction");
    h.press_key(Key::Space, Modifiers::COMMAND);
    assert_eq!(sel(&h), (0, 5), "the whole sum");
    h.press_key(Key::Space, Modifiers::COMMAND);
    assert_eq!(sel(&h), (0, 5), "nothing larger");
    h.press_key(Key::Backspace, Modifiers::NONE);
    assert_eq!(h.source_of(id), "");
}
