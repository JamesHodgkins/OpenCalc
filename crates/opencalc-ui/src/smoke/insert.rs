//! Insert Matrix and Insert Range through the real frame loop.

#![cfg(test)]

use eframe::egui::{Key, Modifiers};
use opencalc_core::document::RegionKind;

use super::Harness;
use crate::commands::CommandId;

#[test]
fn a_range_goes_into_a_new_region_with_its_first_number_selected() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    h.command(CommandId::InsertRange);
    assert_eq!(h.sheet().doc.regions().len(), before + 1);
    let id = *h.sheet().canvas.selection.first().unwrap();
    assert_eq!(h.source_of(id), "1..10");
    assert_eq!(h.sheet().canvas.editing, Some(id));
    // Typing replaces the selected first number.
    h.type_text("5");
    assert_eq!(h.source_of(id), "5..10");
    h.press_key(Key::Enter, Modifiers::NONE);
    h.settle();
    let value = h.app.state.output.values.get(&id).unwrap();
    assert!(value.is_ok(), "a range evaluates: {value:?}");
}

#[test]
fn a_matrix_goes_in_at_the_caret_when_a_region_is_being_edited() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("A:=");
    h.command(CommandId::InsertMatrix);
    assert!(h.app.state.matrix_open, "the size window opens");
    h.command(CommandId::InsertMatrixSize(2, 3));
    assert!(!h.app.state.matrix_open);
    assert_eq!(h.source_of(id), "A:=[0, 0, 0; 0, 0, 0]");
    assert_eq!(h.sheet().canvas.editing, Some(id), "still editing");
    // The first cell is selected: type its value, and the rest is untouched.
    h.type_text("7");
    assert_eq!(h.source_of(id), "A:=[7, 0, 0; 0, 0, 0]");
}

#[test]
fn a_matrix_with_nothing_being_edited_makes_a_new_region_and_undo_removes_it() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    h.command(CommandId::InsertMatrixSize(2, 2));
    assert_eq!(h.sheet().doc.regions().len(), before + 1);
    let id = *h.sheet().canvas.selection.first().unwrap();
    assert_eq!(h.source_of(id), "[0, 0; 0, 0]");
    h.type_text("1");
    assert_eq!(h.source_of(id), "[1, 0; 0, 0]");
    assert!(matches!(
        h.sheet().doc.region(id).unwrap().kind,
        RegionKind::Math(_)
    ));
    h.press_key(Key::Escape, Modifiers::NONE);
    h.command(CommandId::EditUndo);
    h.command(CommandId::EditUndo);
    assert_eq!(h.sheet().doc.regions().len(), before);
}

#[test]
fn the_insert_commands_need_a_worksheet_and_the_window_follows_the_page() {
    let mut h = Harness::new();
    h.settle();
    for c in [CommandId::InsertMatrix, CommandId::InsertRange] {
        assert!(h.app.registry().allows(c), "{c:?}");
    }
    h.command(CommandId::InsertMatrix);
    h.command(CommandId::MatrixClose);
    assert!(!h.app.state.matrix_open);
    h.command(CommandId::InsertMatrix);
    h.command(CommandId::TabHome);
    h.frame();
    assert!(!h.app.state.matrix_open, "closes itself on the Home page");
    for c in [CommandId::InsertMatrix, CommandId::InsertRange] {
        assert!(!h.app.registry().allows(c), "{c:?}");
    }
}
