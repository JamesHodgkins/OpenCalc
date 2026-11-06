//! Headless end-to-end tests: build the whole app, run real frames with
//! synthetic input, and check what the user would see. No window needed.
//!
//! These guard the integration the unit tests cannot: that every command can
//! run, every region type paints, pointer input reaches the canvas, and the
//! worker's results make it back into the document view.

use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use opencalc_core::eval::RegionId;

use crate::app::App;
use crate::commands::HANDLED;
use crate::sheet::{RegionKind, Sheet};
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

    fn sheet(&self) -> &Sheet {
        &self.app.sheets[self.app.active.expect("a worksheet is open")]
    }

    /// Screen position of the middle of a region, from the canvas view.
    fn screen_centre(&self, id: RegionId) -> Pos2 {
        let sheet = self.sheet();
        let r = sheet.region(id).expect("region exists");
        sheet
            .view
            .doc_to_screen(r.rect().center(), self.app.viewport.min)
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

#[test]
fn the_sample_sheet_renders_and_every_region_evaluates() {
    let mut h = Harness::new();
    h.settle();
    let sheet = h.sheet();
    let math = sheet.snapshot().len();
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
    let before = h.sheet().regions.len();
    let commands = [
        ("insert.math", "Math"),
        ("insert.text", "Text"),
        ("insert.area", "Area"),
        ("insert.plot", "Plot"),
        ("insert.table", "Table"),
        ("insert.picture", "Picture"),
    ];
    for (n, (command, label)) in commands.iter().enumerate() {
        h.app.queue.push(command);
        h.frame();
        h.frame();
        let sheet = h.sheet();
        assert_eq!(sheet.regions.len(), before + n + 1, "{command}");
        let newest = sheet.regions.last().expect("a region");
        assert_eq!(newest.kind.label(), *label);
        assert_eq!(sheet.state.selection, vec![newest.id], "{command}");
    }
    h.settle();
}

#[test]
fn clicking_selects_and_dragging_moves_a_region() {
    let mut h = Harness::new();
    h.settle();
    // The "F := 10 kN" region is the second one created (after the title text).
    let id = RegionId(3);
    let start_pos = h.sheet().region(id).expect("F region").pos;

    let from = h.screen_centre(id);
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![press(from)]);
    // Move in steps so egui recognises a drag, ending 100 px right, 60 px down.
    let mut at = from;
    for _ in 0..10 {
        at += egui::vec2(10.0, 6.0);
        h.frame_with(vec![Event::PointerMoved(at)]);
    }
    h.frame_with(vec![release(at)]);
    h.frame();

    let sheet = h.sheet();
    assert_eq!(
        sheet.state.selection,
        vec![id],
        "the dragged region is selected"
    );
    let moved = sheet.region(id).expect("still there").pos;
    // Snapped to the 20-unit grid, and moved about (100, 60) from where it began.
    assert_eq!(moved.x % 20.0, 0.0);
    assert_eq!(moved.y % 20.0, 0.0);
    assert!(
        (moved.x - start_pos.x - 100.0).abs() <= 20.0,
        "x: {moved:?} from {start_pos:?}"
    );
    assert!(
        (moved.y - start_pos.y - 60.0).abs() <= 20.0,
        "y: {moved:?} from {start_pos:?}"
    );
}

#[test]
fn delete_removes_the_selection_and_dependents_report_an_error() {
    let mut h = Harness::new();
    h.settle();
    // Select the "F := 10 kN" region, then delete it. σ := F / A must now fail.
    let id = RegionId(3);
    let p = h.screen_centre(id);
    h.frame_with(vec![Event::PointerMoved(p)]);
    h.frame_with(vec![press(p)]);
    h.frame_with(vec![release(p)]);
    assert_eq!(h.sheet().state.selection, vec![id]);
    h.app.queue.push("region.delete");
    h.settle();
    assert!(h.sheet().region(id).is_none());
    assert!(h.sheet().state.selection.is_empty());
    assert!(
        h.app.output.values.values().any(Result::is_err),
        "a region that used F should now be an error"
    );
}

#[test]
fn switching_sheets_never_shows_another_sheets_results() {
    // Region IDs restart at 1 on every sheet, so applying a stale result from
    // the previous sheet would show wrong values on the new one.
    let mut h = Harness::new();
    h.settle();
    h.app.queue.push("file.new");
    h.frame();
    h.frame();
    // A fresh worksheet has no math regions: nothing may be displayed for it,
    // even though the sample sheet's results were just computed.
    h.settle();
    assert!(h.app.output.values.is_empty(), "{:?}", h.app.output.values);
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
            let mut sheet = Sheet::new("worst case");
            for (i, src) in shapes.iter().enumerate() {
                sheet.add(
                    RegionKind::math(src),
                    Pos2::new(20.0, 20.0 + 80.0 * i as f32),
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
