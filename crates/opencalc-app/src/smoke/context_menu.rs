//! Right click: it acts on what is under the pointer before the menu opens.

#![cfg(test)]

use eframe::egui::{Event, Modifiers, PointerButton, Pos2};

use super::{Harness, F};

fn right_click(h: &mut Harness, at: Pos2) {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![button(true)]);
    h.frame_with(vec![button(false)]);
}

#[test]
fn right_clicking_a_region_selects_it() {
    let mut h = Harness::new();
    h.settle();
    assert!(h.sheet().canvas.selection.is_empty());
    let at = h.screen_centre(F);
    right_click(&mut h, at);
    assert_eq!(h.sheet().canvas.selection, vec![F]);
}

#[test]
fn right_clicking_inside_the_selection_keeps_all_of_it() {
    let mut h = Harness::new();
    h.settle();
    let others: Vec<_> = h
        .sheet()
        .doc
        .regions()
        .iter()
        .map(|r| r.id)
        .filter(|id| *id != F)
        .take(2)
        .collect();
    let mut all = vec![F];
    all.extend(others);
    h.app.state.sheets[0].canvas.selection = all.clone();
    let at = h.screen_centre(F);
    right_click(&mut h, at);
    assert_eq!(h.sheet().canvas.selection, all);
}

#[test]
fn right_clicking_blank_page_clears_the_selection_and_moves_the_insertion_point() {
    let mut h = Harness::new();
    h.settle();
    h.app.state.sheets[0].canvas.selection = vec![F];
    // Blank page, and on screen.
    let doc = Pos2::new(650.0, 300.0);
    let sheet = h.sheet();
    assert!(crate::canvas::hit::hit_test(sheet.doc.regions(), &sheet.canvas.sizes, doc).is_none());
    let at = h.sheet().view.doc_to_screen(doc, h.app.state.viewport.min);
    right_click(&mut h, at);
    assert!(h.sheet().canvas.selection.is_empty());
    let expected = h.sheet().snapper().snap(doc);
    let got = h.sheet().canvas.insert_at;
    assert!(
        (got - expected).length() < 1e-2,
        "the insertion point is where the click was: {got:?} vs {expected:?}"
    );
}
