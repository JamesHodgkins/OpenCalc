//! Editing a text region: it behaves like a math region at its edges.

#![cfg(test)]

use eframe::egui::{Event, Key, Modifiers};
use opencalc_core::document::{RegionId, RegionKind};

use super::Harness;
use crate::commands::CommandId;

impl Harness {
    fn new_text(&mut self) -> RegionId {
        self.command(CommandId::InsertText);
        self.frame();
        self.frame();
        *self.sheet().canvas.selection.first().expect("selected")
    }

    fn text_of(&self, id: RegionId) -> String {
        match &self.sheet().doc.region(id).unwrap().kind {
            RegionKind::Text { text } => text.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn editing(&self) -> bool {
        self.sheet().canvas.editing.is_some()
    }
}

#[test]
fn typing_goes_into_the_text_region() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_text();
    h.type_text("hello");
    assert_eq!(h.text_of(id), "hello");
    assert!(h.editing());
}

#[test]
fn an_arrow_off_the_end_of_the_text_leaves_for_the_crosshair() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_text();
    h.type_text("hello");
    let region = h.sheet().doc.region(id).unwrap().clone();
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    h.frame();
    assert!(!h.editing(), "the editor closed");
    let right = region.pos.x + region.size.w;
    assert!(
        h.sheet().canvas.insert_at.x >= right - 1.0,
        "the crosshair is beyond the right edge: {:?} vs {right}",
        h.sheet().canvas.insert_at
    );
    assert!(h.sheet().canvas.armed);
    assert_eq!(h.text_of(id), "hello", "nothing was typed");
}

#[test]
fn an_arrow_off_the_start_leaves_to_the_left_and_inside_the_text_it_just_moves() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_text();
    h.type_text("ab");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    assert!(h.editing(), "from the end, Left moves the caret back one");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    assert!(h.editing(), "now at the start");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    assert!(!h.editing(), "and one more leaves");
    let region = h.sheet().doc.region(id).unwrap();
    assert!(
        h.sheet().canvas.insert_at.x <= region.pos.x,
        "{:?}",
        h.sheet().canvas.insert_at
    );
}

#[test]
fn up_and_down_leave_only_from_the_first_and_last_lines() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_text();
    h.type_text("a");
    h.press_key(Key::Enter, Modifiers::NONE);
    h.type_text("b");
    assert_eq!(h.text_of(id), "a\nb");
    // The caret is on the last line: Up moves to the first, and stays in the text.
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    assert!(h.editing());
    // Now on the first line: Up leaves, above the region.
    let top = h.sheet().doc.region(id).unwrap().pos.y;
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    assert!(!h.editing());
    assert!(h.sheet().canvas.insert_at.y <= top);
}

#[test]
fn a_selection_is_not_an_edge_and_modifiers_are_left_to_the_text_box() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_text();
    h.type_text("abc");
    h.press_key(Key::A, Modifiers::COMMAND);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(
        h.editing(),
        "with text selected, Right just collapses the selection"
    );
    // Shift+Right at the end extends nothing and does not leave.
    h.press_key(Key::ArrowRight, Modifiers::SHIFT);
    assert!(h.editing());
    assert_eq!(h.text_of(id), "abc");
}

#[test]
fn ctrl_enter_ends_the_text_and_readies_the_next_input_below() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_text();
    h.type_text("note");
    let region = h.sheet().doc.region(id).unwrap().clone();
    h.press_key(Key::Enter, Modifiers::COMMAND);
    assert!(!h.editing());
    assert_eq!(h.text_of(id), "note", "no new line was added");
    assert!(h.sheet().canvas.insert_at.y > region.pos.y);
    assert!(h.sheet().canvas.armed);
    h.type_text("x");
    assert!(
        h.sheet()
            .doc
            .math_regions()
            .iter()
            .any(|(_, e)| format!("{e:?}").contains("x")),
        "typing now starts a math region"
    );
}

#[test]
fn a_text_region_cannot_grow_past_what_a_file_can_hold() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_text();
    let huge = "x".repeat(opencalc_core::persist::MAX_TEXT_CHARS + 5_000);
    h.frame_with(vec![Event::Text(huge)]);
    assert!(h.text_of(id).chars().count() <= opencalc_core::persist::MAX_TEXT_CHARS);
    // So the worksheet can still be saved.
    assert!(opencalc_core::persist::save_bytes(&h.sheet().doc).is_ok());
}
