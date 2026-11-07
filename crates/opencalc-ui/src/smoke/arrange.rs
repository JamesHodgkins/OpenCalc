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
