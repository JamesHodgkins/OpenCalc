//! Find and Replace through the real frame loop.

#![cfg(test)]

use eframe::egui::{Key, Modifiers, Pos2, Rect, Vec2};
use opencalc_core::document::RegionKind;

use super::Harness;
use crate::app::finding::FindCmd;
use crate::commands::CommandId;

fn sources(h: &Harness) -> Vec<String> {
    h.app.state.sheets[0]
        .doc
        .regions()
        .iter()
        .map(|r| match &r.kind {
            RegionKind::Math(m) => m.source().to_owned(),
            RegionKind::Text { text } => text.clone(),
            _ => String::new(),
        })
        .collect()
}

fn search(h: &mut Harness, query: &str) {
    h.command(CommandId::EditFind);
    h.app.state.apply_find(FindCmd::Query(query.to_owned()));
    h.frame();
}

fn selected(h: &Harness) -> Vec<u64> {
    h.app.state.sheets[0]
        .canvas
        .selection
        .iter()
        .map(|id| id.0)
        .collect()
}

#[test]
fn searching_finds_matches_selects_the_first_and_highlights_the_regions() {
    let mut h = Harness::new();
    h.settle();
    search(&mut h, "b");
    let find = &h.app.state.find;
    assert!(find.open && !find.matches.is_empty());
    assert_eq!(find.current, 0);
    assert_eq!(selected(&h), [find.matches[0].region.0]);
    let found = h.app.state.found();
    assert!(found.regions.len() >= 2 && found.current.is_some());
    assert!(crate::panels::find::count_text(&h.app.state).starts_with("1 of "));
    // Painting with highlights on does not fall over, with or without the replace row.
    h.frame();
    h.command(CommandId::EditReplace);
    assert!(h.app.state.find.replace);
    h.frame();
    h.frame();
}

#[test]
fn a_query_with_no_match_says_so_and_next_does_nothing() {
    let mut h = Harness::new();
    h.settle();
    search(&mut h, "qqqq");
    assert!(h.app.state.find.matches.is_empty());
    assert_eq!(crate::panels::find::count_text(&h.app.state), "No matches");
    let before = selected(&h);
    h.command(CommandId::FindNext);
    assert_eq!(selected(&h), before);
    assert!(h.app.state.found().regions.is_empty());
}

#[test]
fn next_and_previous_walk_the_matches_and_wrap() {
    let mut h = Harness::new();
    h.settle();
    search(&mut h, "b");
    let n = h.app.state.find.matches.len();
    assert!(n >= 2);
    let first = selected(&h);
    h.command(CommandId::FindNext);
    assert_eq!(h.app.state.find.current, 1);
    h.command(CommandId::FindPrev);
    assert_eq!(h.app.state.find.current, 0);
    assert_eq!(selected(&h), first);
    h.command(CommandId::FindPrev);
    assert_eq!(h.app.state.find.current, n - 1, "wraps backwards");
    h.command(CommandId::FindNext);
    assert_eq!(h.app.state.find.current, 0, "and forwards");
}

#[test]
fn the_current_match_is_brought_into_view() {
    let mut h = Harness::new();
    h.settle();
    search(&mut h, "w :=");
    let id = h.app.state.find.matches[0].region;
    // Scroll far away, then step to the match again.
    h.app.state.sheets[0].view.offset = Vec2::new(-5000.0, -5000.0);
    h.command(CommandId::FindNext);
    let sheet = &h.app.state.sheets[0];
    let region = sheet.doc.region(id).unwrap();
    let rect = crate::canvas::adapt::rect_of(region, &sheet.canvas.sizes);
    let on = Rect::from_min_size(Pos2::ZERO, h.app.state.viewport.size());
    let at = (rect.center().to_vec2() * sheet.view.zoom + sheet.view.offset).to_pos2();
    assert!(on.contains(at), "{at:?} not in {on:?}");
}

#[test]
fn options_change_what_matches() {
    let mut h = Harness::new();
    h.settle();
    search(&mut h, "f");
    let ignoring_case = h.app.state.find.matches.len();
    h.app.state.apply_find(FindCmd::MatchCase(true));
    h.frame();
    let exact = h.app.state.find.matches.len();
    assert!(exact < ignoring_case, "{exact} < {ignoring_case}");
    h.app.state.apply_find(FindCmd::MatchCase(false));
    h.app.state.apply_find(FindCmd::WholeWord(true));
    h.frame();
    assert!(h.app.state.find.matches.len() <= ignoring_case);
}

#[test]
fn replace_one_changes_only_the_current_match_and_is_one_undo_step() {
    let mut h = Harness::new();
    h.settle();
    let before = sources(&h);
    search(&mut h, "b");
    let total = h.app.state.find.matches.len();
    h.command(CommandId::EditReplace);
    h.app.state.apply_find(FindCmd::Replacement("width".into()));
    h.command(CommandId::FindReplaceOne);
    assert_eq!(h.app.state.find.matches.len(), total - 1);
    let after = sources(&h);
    assert_eq!(
        before.iter().zip(&after).filter(|(a, b)| a != b).count(),
        1,
        "exactly one region changed"
    );
    h.command(CommandId::EditUndo);
    assert_eq!(sources(&h), before);
}

#[test]
fn replace_all_is_one_undo_step_and_recalculates() {
    let mut h = Harness::new();
    h.settle();
    let before = sources(&h);
    search(&mut h, "100 mm");
    assert_eq!(h.app.state.find.matches.len(), 1);
    h.command(CommandId::EditReplace);
    h.app
        .state
        .apply_find(FindCmd::Replacement("200 mm".into()));
    h.command(CommandId::FindReplaceAll);
    h.settle();
    assert!(sources(&h).contains(&"b := 200 mm".to_owned()));
    assert!(h.app.state.find.matches.is_empty());

    // Several regions at once.
    search(&mut h, "b");
    let total = h.app.state.find.matches.len();
    assert!(total >= 2);
    h.app.state.apply_find(FindCmd::Replacement("q".into()));
    h.command(CommandId::FindReplaceAll);
    assert!(h.app.state.find.matches.is_empty());
    h.command(CommandId::EditUndo);
    assert!(
        sources(&h).contains(&"A := b^2".to_owned()),
        "one undo puts every region back"
    );
    h.command(CommandId::EditUndo);
    assert_eq!(sources(&h), before);
}

#[test]
fn closing_the_window_clears_the_highlights_and_find_next_reopens_it() {
    let mut h = Harness::new();
    h.settle();
    search(&mut h, "b");
    h.command(CommandId::FindClose);
    assert!(!h.app.state.find.open && h.app.state.found().regions.is_empty());
    h.command(CommandId::FindNext);
    assert!(h.app.state.find.open);
    h.command(CommandId::TabHome);
    h.frame();
    h.frame();
    assert!(!h.app.state.find.open, "no worksheet, nothing to find in");
}

#[test]
fn the_keyboard_opens_find_and_replace_and_steps_through_matches() {
    let mut h = Harness::new();
    h.settle();
    h.key(Key::F, Modifiers::COMMAND);
    h.frame();
    assert!(h.app.state.find.open && !h.app.state.find.replace);
    h.app.state.apply_find(FindCmd::Query("b".into()));
    h.frame();
    h.key(Key::F3, Modifiers::NONE);
    h.frame();
    assert_eq!(h.app.state.find.current, 1);
    h.key(Key::F3, Modifiers::SHIFT);
    h.frame();
    assert_eq!(h.app.state.find.current, 0);
    h.key(Key::H, Modifiers::COMMAND);
    h.frame();
    assert!(h.app.state.find.replace);
}

#[test]
fn editing_the_worksheet_refreshes_the_matches() {
    let mut h = Harness::new();
    h.settle();
    search(&mut h, "zzz");
    assert!(h.app.state.find.matches.is_empty());
    let i = h.app.state.active.unwrap();
    h.app.state.sheets[i].insert(RegionKind::math("zzz := 1"));
    h.frame();
    assert_eq!(h.app.state.find.matches.len(), 1);
}
