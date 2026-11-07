//! Cut, copy and paste through the real frame loop, with a clipboard that lives in
//! memory.

#![cfg(test)]

use eframe::egui::{Event, Key, Modifiers};
use opencalc_core::document::{RegionId, RegionKind};

use super::Harness;
use crate::commands::CommandId;

impl Harness {
    fn clip_text(&mut self) -> Option<String> {
        self.app.clipboard.get()
    }

    fn select(&mut self, ids: &[RegionId]) {
        self.app.state.sheets[0].canvas.selection = ids.to_vec();
        self.frame();
    }

    /// The regions as (source or text, x, y), in layout order.
    fn contents(&self) -> Vec<(String, f32, f32)> {
        self.sheet()
            .doc
            .regions()
            .iter()
            .map(|r| {
                let text = match &r.kind {
                    RegionKind::Math(m) => m.source().to_owned(),
                    RegionKind::Text { text } => text.clone(),
                    other => other.label().to_owned(),
                };
                (text, r.pos.x, r.pos.y)
            })
            .collect()
    }

    fn ids(&self) -> Vec<RegionId> {
        self.sheet().doc.regions().iter().map(|r| r.id).collect()
    }
}

fn find(h: &Harness, source: &str) -> RegionId {
    h.sheet()
        .doc
        .regions()
        .iter()
        .find(|r| matches!(&r.kind, RegionKind::Math(m) if m.source() == source))
        .map(|r| r.id)
        .expect(source)
}

#[test]
fn copy_then_paste_makes_new_regions_at_the_insertion_point_and_undo_removes_them() {
    let mut h = Harness::new();
    h.settle();
    let f = find(&h, "F := 10 kN");
    let before = h.contents();
    h.select(&[f]);
    h.command(CommandId::ClipboardCopy);
    assert_eq!(
        h.clip_text().as_deref(),
        Some("F := 10 kN"),
        "plain text for other programs"
    );

    let at = h.sheet().canvas.insert_at;
    h.command(CommandId::ClipboardPaste);
    let after = h.contents();
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(
        after.last().unwrap(),
        &("F := 10 kN".to_owned(), at.x, at.y)
    );
    assert_eq!(
        &after[..before.len()],
        &before[..],
        "the original did not move"
    );
    let new = *h.ids().last().unwrap();
    assert_ne!(new, f, "a new region, with its own number");
    assert_eq!(h.sheet().canvas.selection, [new]);
    assert!(
        h.sheet().canvas.insert_at.y > at.y,
        "the next paste goes below"
    );

    // Pasting again makes another, not on top of the first.
    h.command(CommandId::ClipboardPaste);
    let last_two: Vec<_> = h.contents().into_iter().rev().take(2).collect();
    assert_ne!(
        (last_two[0].1, last_two[0].2),
        (last_two[1].1, last_two[1].2)
    );

    h.command(CommandId::EditUndo);
    h.command(CommandId::EditUndo);
    assert_eq!(h.contents(), before);
}

#[test]
fn several_regions_keep_their_layout_relative_to_each_other() {
    let mut h = Harness::new();
    h.settle();
    let (a, b) = (find(&h, "F := 10 kN"), find(&h, "b := 100 mm"));
    let (pa, pb) = {
        let s = h.sheet();
        (s.doc.region(a).unwrap().pos, s.doc.region(b).unwrap().pos)
    };
    h.select(&[a, b]);
    h.command(CommandId::ClipboardCopy);
    assert_eq!(
        h.clip_text().as_deref(),
        Some("F := 10 kN\nb := 100 mm"),
        "top to bottom"
    );
    let at = h.sheet().canvas.insert_at;
    h.command(CommandId::ClipboardPaste);
    let n = h.sheet().doc.regions().len();
    let regions = h.sheet().doc.regions();
    let (na, nb) = (&regions[n - 2], &regions[n - 1]);
    assert_eq!(nb.pos.x - na.pos.x, pb.x - pa.x);
    assert_eq!(nb.pos.y - na.pos.y, pb.y - pa.y);
    assert_eq!(
        (na.pos.x.min(nb.pos.x), na.pos.y.min(nb.pos.y)),
        (at.x, at.y)
    );
    assert_eq!(h.sheet().canvas.selection.len(), 2);
}

#[test]
fn cut_removes_the_regions_and_paste_brings_them_back_as_they_were() {
    let mut h = Harness::new();
    h.settle();
    let b = find(&h, "b := 100 mm");
    let before = h.sheet().doc.regions().len();
    h.select(&[b]);
    h.command(CommandId::ClipboardCut);
    h.frame();
    assert_eq!(h.sheet().doc.regions().len(), before - 1);
    assert!(h.sheet().doc.region(b).is_none());
    assert_eq!(h.clip_text().as_deref(), Some("b := 100 mm"));
    h.command(CommandId::ClipboardPaste);
    assert_eq!(h.sheet().doc.regions().len(), before);
    assert!(h.contents().iter().any(|(t, _, _)| t == "b := 100 mm"));
    // And the pasted math is calculated.
    h.settle();
    let id = find(&h, "b := 100 mm");
    assert!(h
        .app
        .state
        .output
        .values
        .get(&id)
        .is_some_and(|v| v.is_ok()));
}

#[test]
fn text_copied_elsewhere_pastes_as_a_text_region_and_blank_text_does_nothing() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    h.app.clipboard.set("Design check\r\nrev B");
    h.command(CommandId::ClipboardPaste);
    let regions = h.contents();
    assert_eq!(regions.len(), before + 1);
    assert_eq!(regions.last().unwrap().0, "Design check\nrev B");
    assert!(matches!(
        h.sheet().doc.regions().last().unwrap().kind,
        RegionKind::Text { .. }
    ));
    h.app.clipboard.set("  \n ");
    h.command(CommandId::ClipboardPaste);
    assert_eq!(h.sheet().doc.regions().len(), before + 1);
}

#[test]
fn what_this_app_copied_is_forgotten_once_something_else_is_copied() {
    let mut h = Harness::new();
    h.settle();
    let f = find(&h, "F := 10 kN");
    h.select(&[f]);
    h.command(CommandId::ClipboardCopy);
    h.app.clipboard.set("from another program");
    h.command(CommandId::ClipboardPaste);
    let last = h.contents().pop().unwrap();
    assert_eq!(last.0, "from another program");
    assert!(matches!(
        h.sheet().doc.regions().last().unwrap().kind,
        RegionKind::Text { .. }
    ));
}

#[test]
fn inside_a_math_region_copy_cut_and_paste_work_on_the_text() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().doc.regions().len();
    let id = h.new_math();
    h.type_text("abc+1");
    h.press_key(Key::A, Modifiers::COMMAND); // select all
    h.command(CommandId::ClipboardCopy);
    assert_eq!(h.clip_text().as_deref(), Some("abc+1"));
    assert_eq!(h.source_of(id), "abc+1", "copy changes nothing");

    h.command(CommandId::ClipboardCut);
    assert_eq!(h.source_of(id), "", "the selected text is gone");
    h.command(CommandId::ClipboardPaste);
    assert_eq!(h.source_of(id), "abc+1", "and comes back at the caret");
    assert_eq!(
        h.sheet().doc.regions().len(),
        before + 1,
        "no extra region was made"
    );
}

#[test]
fn keyboard_events_reach_regions_when_nothing_is_edited_and_the_editor_otherwise() {
    let mut h = Harness::new();
    h.settle();
    let f = find(&h, "F := 10 kN");
    h.select(&[f]);
    h.frame_with(vec![Event::Copy]);
    h.frame();
    assert_eq!(h.clip_text().as_deref(), Some("F := 10 kN"));
    let before = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Paste("the clipboard is read directly".into())]);
    h.frame();
    assert_eq!(
        h.sheet().doc.regions().len(),
        before + 1,
        "pasted once, not twice"
    );

    // While a math region is open the editor takes the same events.
    let id = h.new_math();
    h.type_text("x");
    let regions = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Paste("yy".into())]);
    assert_eq!(h.source_of(id), "xyy");
    assert_eq!(h.sheet().doc.regions().len(), regions);
}

#[test]
fn copy_and_cut_need_a_selection_and_paste_needs_a_worksheet() {
    let mut h = Harness::new();
    h.settle();
    h.select(&[]);
    let r = h.app.registry();
    assert!(!r.allows(CommandId::ClipboardCopy) && !r.allows(CommandId::ClipboardCut));
    assert!(r.allows(CommandId::ClipboardPaste));
    let f = find(&h, "F := 10 kN");
    h.select(&[f]);
    let r = h.app.registry();
    assert!(r.allows(CommandId::ClipboardCopy) && r.allows(CommandId::ClipboardCut));
    h.command(CommandId::TabHome);
    assert!(!h.app.registry().allows(CommandId::ClipboardPaste));
}
