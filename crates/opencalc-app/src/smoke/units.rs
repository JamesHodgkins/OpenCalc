//! The Units window and defined units through the real frame loop.

#![cfg(test)]

use opencalc_core::document::{Point, RegionKind};
use opencalc_core::edit::Edit;
use opencalc_core::unit_define::define_unit;

use super::Harness;
use crate::app::dispatch::apply_canvas;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::panels::units::{form_id, Form};

fn value_of(h: &Harness, source: &str) -> String {
    let sheet = &h.app.state.sheets[0];
    let id = sheet
        .doc
        .regions()
        .iter()
        .find(|r| matches!(&r.kind, RegionKind::Math(m) if m.source() == source))
        .map(|r| r.id)
        .expect("region");
    match h.app.state.output.values.get(&id) {
        Some(Ok(v)) => crate::format::format_value(v, sheet.doc.unit_system()),
        Some(Err(e)) => format!("error: {e}"),
        None => "no value".to_owned(),
    }
}

#[test]
fn a_defined_unit_works_in_the_worksheet_and_undo_takes_it_away() {
    let mut h = Harness::new();
    h.settle();
    h.app.state.sheets[0].add(RegionKind::math("L := 2 hand"), Point::new(40.0, 700.0));
    h.app.state.sheets[0].seal();
    h.app.recalculate();
    h.settle();
    assert!(
        value_of(&h, "L := 2 hand").starts_with("error"),
        "no such unit yet"
    );

    // What the window does when its Add button is pressed.
    let hand = define_unit("hand", "4 in", &[], None).unwrap();
    let effects = apply_canvas(
        &mut h.app.state,
        0,
        vec![
            CanvasCmd::Seal,
            CanvasCmd::Edit(Edit::SetCustomUnits(vec![hand])),
            CanvasCmd::Seal,
        ],
    );
    assert!(!effects.is_empty(), "defining a unit recalculates");
    h.app.recalculate();
    h.settle();
    assert_eq!(value_of(&h, "L := 2 hand"), "203.2 mm");

    h.command(CommandId::EditUndo);
    h.settle();
    assert!(value_of(&h, "L := 2 hand").starts_with("error"));
    assert!(h.app.state.sheets[0].doc.custom_units().is_empty());
}

#[test]
fn the_window_opens_from_either_button_shows_the_list_and_closes() {
    let mut h = Harness::new();
    h.settle();
    let hand = define_unit("hand", "4 in", &[], None).unwrap();
    h.app.state.sheets[0].edit(Edit::SetCustomUnits(vec![hand]));
    for command in [CommandId::UnitsManage, CommandId::UnitsDefine] {
        h.command(command);
        assert!(h.app.state.units_open);
        for _ in 0..3 {
            h.frame();
        }
        h.command(CommandId::UnitsClose);
        assert!(!h.app.state.units_open);
    }
}

#[test]
fn the_form_survives_frames_with_good_bad_and_editing_input() {
    let mut h = Harness::new();
    h.settle();
    let hand = define_unit("hand", "4 in", &[], None).unwrap();
    h.app.state.sheets[0].edit(Edit::SetCustomUnits(vec![hand]));
    h.command(CommandId::UnitsManage);
    for (symbol, definition, editing) in [
        ("span", "9 in", None),
        ("span", "9 wibble", None),
        ("m", "9 in", None),
        ("hand", "5 in", Some(0)),
        ("", "", None),
    ] {
        h.ctx.data_mut(|d| {
            d.insert_temp(
                form_id(),
                Form {
                    symbol: symbol.into(),
                    definition: definition.into(),
                    editing,
                    started: true,
                },
            )
        });
        h.frame();
        h.frame();
    }
}

#[test]
fn the_window_closes_itself_on_the_home_page() {
    let mut h = Harness::new();
    h.command(CommandId::UnitsManage);
    h.command(CommandId::TabHome);
    h.frame();
    h.frame();
    assert!(!h.app.state.units_open);
}
