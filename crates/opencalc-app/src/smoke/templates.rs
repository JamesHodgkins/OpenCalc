//! Templates: saving one, starting a new worksheet from one, and keeping the
//! template safe from being overwritten by the new worksheet.

#![cfg(test)]

use opencalc_core::document::RegionKind;
use opencalc_core::persist;

use super::files::{setup, Rig};
use crate::app::filing::Dialog;
use crate::commands::CommandId;
use crate::theme::PaletteKey;

fn rig(name: &str) -> Rig {
    let mut t = setup(name, crate::app::AppState::new(PaletteKey::Neutral));
    t.type_into_active("F := 10 kN");
    t
}

fn sources(t: &Rig, sheet: usize) -> Vec<String> {
    t.state().sheets[sheet]
        .doc
        .regions()
        .iter()
        .filter_map(|r| match &r.kind {
            RegionKind::Math(m) => Some(m.source().to_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_template_is_saved_with_its_kind_and_the_worksheet_is_untouched() {
    let mut t = rig("tpl_save");
    let revision = t.state().sheets[0].doc.revision();
    let file = t.dir.join("beam");
    t.answer_template_save(Some(file));
    t.run(CommandId::FileSaveAsTemplate);
    let bytes = std::fs::read(t.dir.join("beam.ocalct")).unwrap();
    let loaded = persist::load_bytes(&bytes).unwrap();
    assert!(loaded.is_template);
    assert_eq!(loaded.worksheet.regions().len(), 1);
    assert_eq!(t.state().sheets[0].doc.revision(), revision);
    assert!(t.state().sheets[0].path.is_none(), "it is still untitled");
    // an ordinary save is not a template
    let plain = persist::save_bytes(&loaded.worksheet).unwrap();
    assert!(!persist::load_bytes(&plain).unwrap().is_template);
}

#[test]
fn a_new_worksheet_from_a_template_is_an_untitled_copy() {
    let mut t = rig("tpl_new");
    t.answer_template_save(Some(t.dir.join("beam.ocalct")));
    t.run(CommandId::FileSaveAsTemplate);
    t.run(CommandId::DialogOk);
    t.answer_template_open(Some(t.dir.join("beam.ocalct")));
    t.run(CommandId::FileNewFromTemplate);
    assert_eq!(t.state().sheets.len(), 2);
    let i = t.state().active.unwrap();
    assert_eq!(i, 1);
    assert_eq!(sources(&t, 1), ["F := 10 kN"]);
    assert!(
        t.state().sheets[1].path.is_none(),
        "not tied to the template file"
    );
    assert!(t.state().sheets[1].dirty(), "closing asks about saving");
    assert!(t.state().sheets[1].title().starts_with("Untitled"));
}

#[test]
fn an_ordinary_worksheet_can_also_be_used_as_a_template() {
    let mut t = rig("tpl_plain");
    let file = t.dir.join("sheet.ocalc");
    std::fs::write(
        &file,
        persist::save_bytes(&t.state().sheets[0].doc).unwrap(),
    )
    .unwrap();
    t.answer_template_open(Some(file));
    t.run(CommandId::FileNewFromTemplate);
    assert_eq!(t.state().sheets.len(), 2);
}

#[test]
fn a_bad_template_says_why_and_opens_nothing() {
    let mut t = rig("tpl_bad");
    let file = t.dir.join("junk.ocalct");
    std::fs::write(&file, b"not a zip").unwrap();
    t.answer_template_open(Some(file));
    t.run(CommandId::FileNewFromTemplate);
    assert_eq!(t.state().sheets.len(), 1);
    match &t.state().dialog {
        Some(Dialog::Message { title, .. }) => assert!(title.contains("template")),
        other => panic!("expected a message, found {other:?}"),
    }
    t.run(CommandId::DialogOk);
    t.answer_template_open(None);
    t.run(CommandId::FileNewFromTemplate);
    assert!(t.state().dialog.is_none());
}

#[test]
fn a_template_opened_for_editing_is_saved_back_as_a_template() {
    let mut t = rig("tpl_edit");
    t.answer_template_save(Some(t.dir.join("beam.ocalct")));
    t.run(CommandId::FileSaveAsTemplate);
    t.run(CommandId::DialogOk);
    t.h.app.open_path(&t.dir.join("beam.ocalct"));
    t.h.frame();
    let i = t.state().active.unwrap();
    assert_eq!(
        t.state().sheets[i].path.as_deref(),
        Some(t.dir.join("beam.ocalct").as_path())
    );
    t.type_into_active("G := 2");
    t.run(CommandId::FileSave);
    let back = persist::load_bytes(&std::fs::read(t.dir.join("beam.ocalct")).unwrap()).unwrap();
    assert!(back.is_template, "still a template after Save");
    assert_eq!(back.worksheet.regions().len(), 2);
}
