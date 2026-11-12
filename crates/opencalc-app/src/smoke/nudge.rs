//! Arrow keys move the selection; Select all and Escape.

#![cfg(test)]

use eframe::egui::{Key, Modifiers};
use opencalc_core::document::RegionId;

use super::{Harness, F};
use crate::commands::CommandId;

/// A region selected by a click, with nothing being edited.
fn selected() -> (Harness, RegionId) {
    let mut h = Harness::new();
    h.settle();
    h.click(F);
    h.frame();
    assert_eq!(h.sheet().canvas.selection, vec![F]);
    assert!(h.sheet().canvas.editing.is_none());
    (h, F)
}

fn on_grid(h: &Harness, v: f32, y: bool) -> bool {
    let sheet = h.sheet();
    let o = sheet.snapper().origin_at(if y { v } else { 0.0 });
    let k = (v - if y { o.y } else { o.x }) / sheet.canvas.grid.step();
    (k - k.round()).abs() < 1e-3
}

#[test]
fn an_arrow_moves_the_selected_region_one_grid_step_and_keeps_it_selected() {
    let (mut h, id) = selected();
    let start = h.region_pos(id);
    let step = h.sheet().canvas.grid.step();
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    let moved = h.region_pos(id);
    assert!(
        moved.x > start.x && moved.x - start.x <= step + 1e-3,
        "{start:?} {moved:?}"
    );
    assert_eq!(moved.y, start.y);
    assert!(on_grid(&h, moved.x, false), "lands on the grid: {moved:?}");
    assert_eq!(h.sheet().canvas.selection, vec![id], "still selected");
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    let down = h.region_pos(id);
    assert!(down.y > moved.y && on_grid(&h, down.y, true));
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    assert!(h.region_pos(id).x < down.x && h.region_pos(id).y < down.y);
}

#[test]
fn the_crosshair_does_not_move_while_something_is_selected() {
    let (mut h, _) = selected();
    let at = h.sheet().canvas.insert_at;
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(h.sheet().canvas.insert_at, at);
}

#[test]
fn shift_moves_five_steps_and_alt_one_unit() {
    let (mut h, id) = selected();
    let start = h.region_pos(id);
    let step = h.sheet().canvas.grid.step();
    h.held = Modifiers::SHIFT;
    h.press_key(Key::ArrowRight, Modifiers::SHIFT);
    let big = h.region_pos(id).x - start.x;
    assert!((big - 5.0 * step).abs() <= step, "about five steps: {big}");
    assert!(big > 3.0 * step);
    h.held = Modifiers::ALT;
    let before = h.region_pos(id);
    h.press_key(Key::ArrowDown, Modifiers::ALT);
    assert!(
        (h.region_pos(id).y - before.y - 1.0).abs() < 1e-3,
        "one unit, off the grid"
    );
    h.held = Modifiers::NONE;
}

#[test]
fn holding_an_arrow_is_one_undo_step() {
    let (mut h, id) = selected();
    let start = h.region_pos(id);
    for _ in 0..4 {
        h.press_key(Key::ArrowRight, Modifiers::NONE);
    }
    assert!(h.region_pos(id).x > start.x + 2.0 * h.sheet().canvas.grid.step());
    h.frame();
    h.command(CommandId::EditUndo);
    assert_eq!(h.region_pos(id), start, "one undo puts it back");
}

#[test]
fn a_locked_region_is_not_nudged() {
    let (mut h, id) = selected();
    h.app.state.sheets[0].canvas.selection = vec![id];
    h.command(CommandId::RegionLock);
    let start = h.region_pos(id);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(h.region_pos(id), start);
}

#[test]
fn escape_lets_go_of_the_selection_and_then_the_arrows_move_the_crosshair() {
    let (mut h, _) = selected();
    h.press_key(Key::Escape, Modifiers::NONE);
    assert!(h.sheet().canvas.selection.is_empty());
    let at = h.sheet().canvas.insert_at;
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert_ne!(h.sheet().canvas.insert_at, at);
}

#[test]
fn select_all_selects_every_region_and_the_arrows_move_them_together() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::EditSelectAll);
    let all: Vec<RegionId> = h.sheet().doc.regions().iter().map(|r| r.id).collect();
    assert!(all.len() > 1);
    assert_eq!(h.sheet().canvas.selection, all);
    let before: Vec<_> = all.iter().map(|id| h.region_pos(*id)).collect();
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    let after: Vec<_> = all.iter().map(|id| h.region_pos(*id)).collect();
    let d = after[0].y - before[0].y;
    assert!(d > 0.0);
    for (a, b) in after.iter().zip(&before) {
        assert!(
            (a.y - b.y - d).abs() < 1e-3 && a.x == b.x,
            "the group keeps its shape"
        );
    }
}

#[test]
fn ctrl_d_copies_the_selection_one_step_down_and_selects_the_copy() {
    let (mut h, id) = selected();
    h.app.state.sheets[0].edit(opencalc_core::edit::Edit::SetLabel {
        id,
        label: "load".into(),
    });
    let before = h.sheet().doc.regions().len();
    let at = h.region_pos(id);
    let step = h.sheet().canvas.grid.step();
    h.press_key(Key::D, Modifiers::COMMAND);
    assert_eq!(h.sheet().doc.regions().len(), before + 1);
    let [copy] = h.sheet().canvas.selection.as_slice() else {
        panic!("the copy is selected");
    };
    assert_ne!(*copy, id);
    let copy = h.sheet().doc.region(*copy).expect("copy").clone();
    assert_eq!((copy.pos.x, copy.pos.y), (at.x + step, at.y + step));
    assert!(copy.label.is_empty(), "a label names one region");
    let original = h.sheet().doc.region(id).expect("original");
    assert_eq!(copy.kind, original.kind);
    assert_eq!(original.label, "load");
    // One undo takes the copy away again.
    h.command(CommandId::EditUndo);
    assert_eq!(h.sheet().doc.regions().len(), before);
}

#[test]
fn duplicate_needs_a_selection() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    h.press_key(Key::D, Modifiers::COMMAND);
    assert_eq!(h.sheet().doc.regions().len(), before);
}

#[test]
fn enter_or_f2_starts_editing_the_selected_region_and_escape_leaves_it_selected() {
    for key in [Key::Enter, Key::F2] {
        let (mut h, id) = selected();
        h.press_key(key, Modifiers::NONE);
        assert_eq!(h.sheet().canvas.editing, Some(id), "{key:?}");
        h.press_key(Key::Escape, Modifiers::NONE);
        assert!(h.sheet().canvas.editing.is_none());
        assert_eq!(
            h.sheet().canvas.selection,
            vec![id],
            "{key:?}: still selected"
        );
    }
}

#[test]
fn enter_does_not_edit_a_locked_region_or_a_group() {
    let (mut h, id) = selected();
    h.command(CommandId::RegionLock);
    h.press_key(Key::Enter, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_none());
    h.command(CommandId::EditSelectAll);
    h.press_key(Key::Enter, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_none());
    let _ = id;
}

fn hover(h: &mut Harness, at: eframe::egui::Pos2) {
    h.frame_with(vec![eframe::egui::Event::PointerMoved(at)]);
    h.frame_with(vec![eframe::egui::Event::PointerMoved(at)]);
}

#[test]
fn the_cursor_shows_what_a_press_will_do() {
    use eframe::egui::CursorIcon;
    let mut h = Harness::new();
    h.settle();
    // Over a region that can be dragged: the move cursor.
    let at = h.screen_centre(F);
    hover(&mut h, at);
    assert_eq!(h.cursor, CursorIcon::Move);
    // Over blank page: the plain arrow.
    let blank = h.sheet().view.doc_to_screen(
        eframe::egui::Pos2::new(650.0, 300.0),
        h.app.state.viewport.min,
    );
    hover(&mut h, blank);
    assert_eq!(h.cursor, CursorIcon::Default);
    // A locked region cannot be dragged, so it does not offer to be.
    h.app.state.sheets[0].canvas.selection = vec![F];
    h.command(CommandId::RegionLock);
    hover(&mut h, at);
    assert_ne!(h.cursor, CursorIcon::Move);
}

#[test]
fn the_cursor_closes_to_a_grab_while_a_region_is_dragged() {
    use eframe::egui::{CursorIcon, Event, PointerButton};
    let mut h = Harness::new();
    h.settle();
    let from = h.screen_centre(F);
    hover(&mut h, from);
    h.frame_with(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    let mut at = from;
    for _ in 0..4 {
        at += eframe::egui::vec2(12.0, 8.0);
        h.frame_with(vec![Event::PointerMoved(at)]);
    }
    assert_eq!(h.cursor, CursorIcon::Grabbing);
    h.frame_with(vec![Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    h.frame();
    assert_ne!(h.cursor, CursorIcon::Grabbing, "let go");
}
