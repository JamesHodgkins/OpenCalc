//! Separate regions, through the command.

#![cfg(test)]

use opencalc_core::document::{Point, RegionKind};

use super::Harness;
use crate::canvas::adapt::rect_of;
use crate::commands::CommandId;

fn overlaps(h: &Harness) -> usize {
    let sheet = &h.app.state.sheets[0];
    let rects: Vec<_> = sheet
        .doc
        .regions()
        .iter()
        .map(|r| rect_of(r, &sheet.canvas.sizes))
        .collect();
    let mut n = 0;
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            if a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y {
                n += 1;
            }
        }
    }
    n
}

fn positions(h: &Harness) -> Vec<Point> {
    h.app.state.sheets[0]
        .doc
        .regions()
        .iter()
        .map(|r| r.pos)
        .collect()
}

#[test]
fn separating_clears_every_overlap_in_one_undoable_step() {
    let mut h = Harness::new();
    h.settle();
    assert_eq!(overlaps(&h), 0, "the example starts tidy");
    let before = positions(&h);
    // A pile of three on top of one another, below the example.
    for (i, src) in ["p := 1 m", "q := 2 m", "r := 3 m"].iter().enumerate() {
        h.app.state.sheets[0].add(
            RegionKind::math(src),
            Point::new(60.0, 600.0 + i as f32 * 4.0),
        );
    }
    h.frame();
    h.frame();
    assert!(overlaps(&h) >= 2, "the pile overlaps: {}", overlaps(&h));
    let piled = positions(&h);

    h.command(CommandId::ArrangeSeparate);
    h.frame();
    assert_eq!(overlaps(&h), 0);
    let tidy = positions(&h);
    assert_eq!(
        &tidy[..before.len()],
        &before[..],
        "what was clear did not move"
    );
    assert!(
        tidy[before.len()..].iter().all(|p| p.x == 60.0),
        "only moved down"
    );

    h.command(CommandId::EditUndo);
    assert_eq!(positions(&h), piled, "one undo puts them all back");
}

#[test]
fn separating_a_tidy_worksheet_changes_nothing_and_records_nothing() {
    let mut h = Harness::new();
    h.settle();
    let revision = h.app.state.sheets[0].doc.revision();
    h.command(CommandId::ArrangeSeparate);
    assert_eq!(h.app.state.sheets[0].doc.revision(), revision);
    assert!(!h.app.state.sheets[0].can_undo());
    assert!(h.app.state.dialog.is_none());
}

fn three(h: &mut Harness) -> [opencalc_core::document::RegionId; 3] {
    let s = &mut h.app.state.sheets[0];
    let a = s.add(RegionKind::math("a := 1"), Point::new(60.0, 700.0));
    let b = s.add(RegionKind::math("b := 2"), Point::new(300.0, 660.0));
    let c = s.add(RegionKind::math("c := 3"), Point::new(180.0, 760.0));
    [a, b, c]
}

fn select(h: &mut Harness, ids: &[opencalc_core::document::RegionId]) {
    h.app.state.sheets[0].canvas.selection = ids.to_vec();
    h.frame();
}

#[test]
fn align_horizontally_lines_the_selection_up_in_a_row_as_one_undo_step() {
    let mut h = Harness::new();
    h.settle();
    let [a, b, c] = three(&mut h);
    select(&mut h, &[a, b, c]);
    h.command(CommandId::ArrangeAlignH);
    for id in [a, b, c] {
        assert_eq!(h.region_pos(id).y, 660.0, "the highest top");
    }
    assert_eq!(
        (h.region_pos(a).x, h.region_pos(b).x, h.region_pos(c).x),
        (60.0, 300.0, 180.0),
        "x is untouched"
    );
    h.command(CommandId::EditUndo);
    assert_eq!(
        (h.region_pos(a).y, h.region_pos(b).y, h.region_pos(c).y),
        (700.0, 660.0, 760.0)
    );
}

#[test]
fn align_vertically_lines_the_selection_up_in_a_column() {
    let mut h = Harness::new();
    h.settle();
    let [a, b, c] = three(&mut h);
    select(&mut h, &[a, b, c]);
    h.command(CommandId::ArrangeAlignV);
    for id in [a, b, c] {
        assert_eq!(h.region_pos(id).x, 60.0, "the leftmost edge");
    }
    assert_eq!(h.region_pos(c).y, 760.0, "y is untouched");
}

#[test]
fn aligning_needs_two_regions_and_leaves_the_unselected_alone() {
    let mut h = Harness::new();
    h.settle();
    let [a, b, c] = three(&mut h);
    select(&mut h, &[a]);
    assert!(!h.app.registry().allows(CommandId::ArrangeAlignH));
    assert!(!h.app.registry().allows(CommandId::ArrangeAlignV));
    select(&mut h, &[a, c]);
    assert!(h.app.registry().allows(CommandId::ArrangeAlignH));
    h.command(CommandId::ArrangeAlignH);
    assert_eq!(h.region_pos(c).y, 700.0);
    assert_eq!(h.region_pos(b).y, 660.0, "not selected, not moved");
}

#[test]
fn a_locked_region_cannot_be_deleted_moved_edited_or_restyled_and_unlocking_frees_it() {
    let mut h = Harness::new();
    h.settle();
    let [a, b, _] = three(&mut h);
    select(&mut h, &[a]);
    h.command(CommandId::RegionLock);
    assert!(h.sheet().doc.region(a).unwrap().locked);
    assert!(h
        .app
        .state
        .active_toggles()
        .contains(&CommandId::RegionLock));

    // Delete, with another region selected too: only the unlocked one goes.
    select(&mut h, &[a, b]);
    h.command(CommandId::RegionDelete);
    assert!(h.sheet().doc.region(a).is_some(), "locked stays");
    assert!(h.sheet().doc.region(b).is_none());
    h.command(CommandId::EditUndo);
    assert!(h.sheet().doc.region(b).is_some());

    // Moved by an edit: refused. Aligned with another: it stays put.
    let at = h.region_pos(a);
    assert!(
        !h.app.state.sheets[0].edit(opencalc_core::edit::Edit::MoveRegions {
            moves: vec![(a, Point::new(1.0, 1.0))],
        })
    );
    select(&mut h, &[a, b]);
    h.command(CommandId::ArrangeAlignV);
    assert_eq!(h.region_pos(a), at);
    assert_eq!(h.region_pos(b).x, at.x.min(h.region_pos(b).x));

    // Opening it for editing is not possible.
    h.app.state.sheets[0].canvas.editing = Some(a);
    h.frame();
    assert_eq!(h.sheet().canvas.editing, None);
    assert_eq!(h.source_of(a), "a := 1");

    // Unlocking (the same button) frees it, and undo puts the lock back.
    select(&mut h, &[a]);
    h.command(CommandId::RegionLock);
    assert!(!h.sheet().doc.region(a).unwrap().locked);
    h.command(CommandId::RegionDelete);
    assert!(h.sheet().doc.region(a).is_none());
    h.command(CommandId::EditUndo);
    h.command(CommandId::EditUndo);
    assert!(h.sheet().doc.region(a).unwrap().locked, "undo re-locks it");
}

#[test]
fn locking_is_saved_and_a_copy_is_not_locked_and_find_replace_skips_it() {
    use opencalc_core::persist::{load_bytes, save_bytes};
    let mut h = Harness::new();
    h.settle();
    let [a, b, _] = three(&mut h);
    select(&mut h, &[a]);
    h.command(CommandId::RegionLock);
    let loaded = load_bytes(&save_bytes(&h.sheet().doc).unwrap()).unwrap();
    assert!(loaded.worksheet.region(a).unwrap().locked);
    assert!(!loaded.worksheet.region(b).unwrap().locked);

    h.command(CommandId::ClipboardCopy);
    h.command(CommandId::ClipboardPaste);
    let copy = *h.sheet().canvas.selection.first().unwrap();
    assert_ne!(copy, a);
    assert!(!h.sheet().doc.region(copy).unwrap().locked);

    // Replace everywhere changes the unlocked regions and leaves the locked one.
    let doc = &h.sheet().doc;
    let matches = opencalc_core::find::find_all(doc, "1", Default::default());
    let edit = opencalc_core::find::replace_edit(doc, &matches, "9").unwrap();
    let mut w = doc.clone();
    w.apply(edit).unwrap();
    assert_eq!(
        match &w.region(a).unwrap().kind {
            RegionKind::Math(m) => m.source().to_owned(),
            _ => unreachable!(),
        },
        "a := 1"
    );
}

#[test]
fn lock_needs_a_selection() {
    let mut h = Harness::new();
    h.settle();
    select(&mut h, &[]);
    assert!(!h.app.registry().allows(CommandId::RegionLock));
    let [a, _, _] = three(&mut h);
    select(&mut h, &[a]);
    assert!(h.app.registry().allows(CommandId::RegionLock));
}
