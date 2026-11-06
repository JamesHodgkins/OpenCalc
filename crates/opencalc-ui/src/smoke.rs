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
use opencalc_core::document::{Point, RegionId, RegionKind};

use crate::app::App;
use crate::canvas::adapt::{pos, rect_of};
use crate::commands::HANDLED;
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
        let app = App::new(ctx.clone(), PaletteKey::Neutral);
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
            if self.app.status_text().contains("up to date") {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("worker result never arrived: {}", self.app.status_text());
    }

    fn command(&mut self, command: &'static str) {
        self.app.queue.push(command);
        self.frame();
        self.frame();
    }

    fn sheet(&self) -> &OpenSheet {
        &self.app.sheets[self.app.active.expect("a worksheet is open")]
    }

    fn region_pos(&self, id: RegionId) -> Point {
        self.sheet().doc.region(id).expect("region exists").pos
    }

    /// Screen position of the middle of a region, from the canvas view.
    fn screen_centre(&self, id: RegionId) -> Pos2 {
        let sheet = self.sheet();
        let r = sheet.doc.region(id).expect("region exists");
        let centre = rect_of(r, &sheet.canvas.sizes).center();
        sheet.view.doc_to_screen(centre, self.app.viewport.min)
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
fn the_sample_sheet_renders_and_every_region_evaluates() {
    let mut h = Harness::new();
    h.settle();
    let sheet = h.sheet();
    let math = sheet.doc.math_regions().len();
    assert_eq!(math, 6);
    assert_eq!(h.app.output.values.len(), math);
    assert!(h.app.output.values.values().all(Result::is_ok));
    assert!(h.app.output.warnings.is_empty());
}

#[test]
fn every_command_runs_without_panicking() {
    let extras = ["file.example", "tab.home", "app.quit", "no.such.command"];
    for command in HANDLED.iter().chain(extras.iter()) {
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
        ("insert.math", "Math"),
        ("insert.text", "Text"),
        ("insert.area", "Area"),
        ("insert.plot", "Plot"),
        ("insert.table", "Table"),
        ("insert.picture", "Picture"),
    ];
    for (n, (command, label)) in commands.iter().enumerate() {
        h.command(command);
        let sheet = h.sheet();
        assert_eq!(sheet.doc.regions().len(), before + n + 1, "{command}");
        let newest = sheet.doc.regions().last().expect("a region");
        assert_eq!(newest.kind.label(), *label);
        assert_eq!(sheet.canvas.selection, vec![newest.id], "{command}");
    }
    h.settle();
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
    // Snapped to the 20-unit grid, and moved about (100, 60) from where it began.
    assert_eq!(moved.x % 20.0, 0.0);
    assert_eq!(moved.y % 20.0, 0.0);
    assert!(
        (moved.x - start.x - 100.0).abs() <= 20.0,
        "x: {moved:?} from {start:?}"
    );
    assert!(
        (moved.y - start.y - 60.0).abs() <= 20.0,
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

    h.command("edit.undo");
    assert_eq!(h.region_pos(F), start, "one undo reverses the whole drag");
    assert!(!h.sheet().can_undo(), "the drag was a single step");
    h.command("edit.redo");
    assert_eq!(h.region_pos(F), moved);
}

#[test]
fn moving_a_region_does_not_recalculate() {
    let mut h = Harness::new();
    h.settle();
    let results_before: Vec<_> = {
        let mut v: Vec<_> = h
            .app
            .output
            .values
            .iter()
            .map(|(k, v)| (k.0, v.clone()))
            .collect();
        v.sort_by_key(|(k, _)| *k);
        v
    };
    let eval_before = h.sheet().doc.eval_revision();
    let requests_before = h.app.revision;
    h.drag(F);
    assert_eq!(
        h.app.revision, requests_before,
        "dragging must not ask the worker to recalculate"
    );
    assert_eq!(
        h.sheet().doc.eval_revision(),
        eval_before,
        "a move is not an evaluation change"
    );
    assert!(
        h.app.status_text().contains("up to date"),
        "no recalculation was started"
    );
    let mut results_after: Vec<_> = h
        .app
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
    h.command("insert.plot"); // a plot: no text field takes the keyboard
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
    h.command("region.delete");
    h.settle();
    assert!(h.sheet().doc.region(F).is_none());
    assert!(h.sheet().canvas.selection.is_empty());
    assert!(
        h.app.output.values.values().any(Result::is_err),
        "a region that used F should now be an error"
    );

    h.command("edit.undo");
    h.settle();
    let order_after: Vec<RegionId> = h.sheet().doc.regions().iter().map(|r| r.id).collect();
    assert_eq!(
        order_after, order_before,
        "back in the same z-order position"
    );
    assert!(
        h.app.output.values.values().all(Result::is_ok),
        "undoing the delete recalculated and everything evaluates again"
    );
}

#[test]
fn typing_into_a_region_is_one_undo_step_and_recalculates_live() {
    let mut h = Harness::new();
    h.settle();
    h.command("insert.math"); // opens the new region for editing with focus
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
        h.app.output.values[&id].is_ok(),
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
    h.command("file.new");
    h.settle();
    assert!(h.app.output.values.is_empty(), "{:?}", h.app.output.values);
}

#[test]
fn each_sheet_keeps_its_own_undo_history() {
    let mut h = Harness::new();
    h.settle();
    h.command("insert.plot");
    assert!(h.sheet().can_undo());
    h.command("file.new");
    assert!(!h.sheet().can_undo(), "a new sheet starts with no history");
    h.app.select_sheet(0);
    h.frame();
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
            h.app.sheets.push(sheet);
            h.app.select_sheet(1);
            h.settle();
            // Every region either evaluated or reported an error; none was lost.
            assert_eq!(h.app.output.values.len(), 6);
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
    let screen = sheet.view.doc_to_screen(pos(r.pos), h.app.viewport.min);
    let back = sheet.view.screen_to_doc(screen, h.app.viewport.min);
    assert!((back - pos(r.pos)).length() < 1e-3);
}
