//! Region labels: the Tag command, the Label window, and what labels are used for.

#![cfg(test)]

use opencalc_core::document::{Point, RegionId, RegionKind};
use opencalc_core::edit::Edit;

use super::Harness;
use crate::commands::CommandId;

fn two() -> (Harness, RegionId, RegionId) {
    let mut h = Harness::new();
    h.settle();
    let s = &mut h.app.state.sheets[0];
    let a = s.add(RegionKind::math("a := 1"), Point::new(60.0, 700.0));
    let b = s.add(RegionKind::math("zq = 5 m"), Point::new(60.0, 760.0));
    h.app.recalculate();
    h.settle();
    (h, a, b)
}

fn label(h: &Harness, id: RegionId) -> String {
    h.sheet().doc.region(id).unwrap().label.clone()
}

/// What the Label window does as text is typed into it.
fn set(h: &mut Harness, id: RegionId, text: &str) {
    h.app.state.sheets[0].edit(Edit::SetLabel {
        id,
        label: text.into(),
    });
    h.frame();
}

#[test]
fn tag_opens_the_window_for_the_selected_region_and_closing_it_keeps_the_label() {
    let (mut h, a, _) = two();
    h.app.state.sheets[0].canvas.selection = vec![a];
    h.frame();
    assert!(h.app.registry().allows(CommandId::RegionTag));
    h.command(CommandId::RegionTag);
    assert_eq!(h.app.state.label_for, Some(a));
    h.frame();
    for text in ["B", "Be", "Beam check "] {
        set(&mut h, a, text);
    }
    assert_eq!(label(&h, a), "Beam check", "tidied");
    h.command(CommandId::LabelClose);
    assert_eq!(h.app.state.label_for, None);
    assert_eq!(label(&h, a), "Beam check");
    // Typing was one undo step.
    h.command(CommandId::EditUndo);
    assert_eq!(label(&h, a), "");
}

#[test]
fn the_tag_command_needs_a_selection_and_the_window_follows_the_region() {
    let (mut h, a, _) = two();
    h.app.state.sheets[0].canvas.selection.clear();
    h.frame();
    assert!(!h.app.registry().allows(CommandId::RegionTag));
    h.app.state.sheets[0].canvas.selection = vec![a];
    h.command(CommandId::RegionTag);
    h.app.state.sheets[0].canvas.selection.clear();
    h.command(CommandId::TabHome);
    h.frame();
    h.frame();
    assert_eq!(
        h.app.state.label_for, None,
        "closes when the worksheet goes"
    );
}

#[test]
fn a_locked_region_can_still_be_labelled_and_find_and_copy_see_the_label() {
    let (mut h, a, _) = two();
    h.app.state.sheets[0].canvas.selection = vec![a];
    h.command(CommandId::RegionLock);
    set(&mut h, a, "Beam check");
    assert_eq!(label(&h, a), "Beam check");

    // Find lands on the labelled region.
    let doc = &h.sheet().doc;
    let found = opencalc_core::find::find_all(doc, "beam", Default::default());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].region, a);

    // A copy keeps the label (and, being new, not the lock).
    h.command(CommandId::ClipboardCopy);
    h.command(CommandId::ClipboardPaste);
    let copy = *h.sheet().canvas.selection.first().unwrap();
    assert_eq!(label(&h, copy), "Beam check");
    assert!(!h.sheet().doc.region(copy).unwrap().locked);
}

#[test]
fn a_finding_carries_the_label_and_the_label_is_saved() {
    use opencalc_core::persist::{load_bytes, save_bytes};
    let (mut h, _, b) = two();
    set(&mut h, b, "Column load");
    h.command(CommandId::CalcCheck);
    let finding = &h.app.state.check.findings[0];
    assert_eq!((finding.region, finding.label.as_str()), (b, "Column load"));

    let loaded = load_bytes(&save_bytes(&h.sheet().doc).unwrap()).unwrap();
    assert_eq!(loaded.worksheet.region(b).unwrap().label, "Column load");
}

#[test]
fn the_shortcuts_window_opens_draws_and_closes() {
    let mut h = Harness::new();
    h.settle();
    assert!(h.app.registry().allows(CommandId::HelpShortcuts));
    h.command(CommandId::HelpShortcuts);
    assert!(h.app.state.shortcuts_open);
    h.frame();
    h.frame();
    h.command(CommandId::HelpShortcutsClose);
    assert!(!h.app.state.shortcuts_open);
    // It also opens from the Home page, where there is no worksheet.
    h.command(CommandId::TabHome);
    h.command(CommandId::HelpShortcuts);
    h.frame();
    assert!(h.app.state.shortcuts_open);
}

#[test]
fn the_options_window_offers_to_register_the_file_type_and_reports_a_failure() {
    use crate::app::association::MemoryFileType;
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::AppOptions);
    assert_eq!(
        h.app.state.file_type,
        (true, false),
        "read when Options opens"
    );
    h.frame();
    h.command(CommandId::FileTypeRegister);
    assert_eq!(h.app.state.file_type, (true, true));
    h.command(CommandId::FileTypeRemove);
    assert_eq!(h.app.state.file_type, (true, false));

    // A refusal from the system is said, and the state still tells the truth.
    h.app.file_type = Box::new(MemoryFileType {
        on: false,
        fail: true,
    });
    h.command(CommandId::FileTypeRegister);
    match &h.app.state.dialog {
        Some(crate::app::filing::Dialog::Message { text, .. }) => {
            assert!(text.contains("access denied"), "{text}");
        }
        other => panic!("expected a message, found {other:?}"),
    }
    assert_eq!(h.app.state.file_type, (true, false));
}

#[test]
fn the_labels_window_lists_the_labelled_regions_and_goes_to_the_one_chosen() {
    let (mut h, a, b) = two();
    set(&mut h, b, "Column load");
    set(&mut h, a, "Beam check");
    h.command(CommandId::ViewLabels);
    assert!(h.app.state.labels_open);
    h.frame();
    h.frame();
    // `a` is the higher of the two, so it is first.
    h.command(CommandId::GoToLabel(1));
    assert_eq!(h.sheet().canvas.selection, [b]);
    h.command(CommandId::GoToLabel(0));
    assert_eq!(h.sheet().canvas.selection, [a]);
    // A number past the end does nothing.
    h.command(CommandId::GoToLabel(40));
    assert_eq!(h.sheet().canvas.selection, [a]);
    h.command(CommandId::LabelsClose);
    assert!(!h.app.state.labels_open);
    // It closes itself when the worksheet goes, and needs one to open.
    h.command(CommandId::ViewLabels);
    h.command(CommandId::TabHome);
    h.frame();
    h.frame();
    assert!(!h.app.state.labels_open);
    assert!(!h.app.registry().allows(CommandId::ViewLabels));
}
