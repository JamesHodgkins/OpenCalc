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
