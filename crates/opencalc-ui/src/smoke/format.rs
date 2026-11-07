//! The Format commands through the real frame loop: how results are shown.

#![cfg(test)]

use opencalc_core::document::{Notation, Point, RegionId, RegionKind};
use opencalc_core::persist::{load_bytes, save_bytes};

use super::Harness;
use crate::commands::CommandId;
use crate::format::format_value_with;

/// A worksheet with `x := 1234.5678` and the evaluation `x =`.
fn setup() -> (Harness, RegionId, RegionId) {
    let mut h = Harness::new();
    h.settle();
    let sheet = &mut h.app.state.sheets[0];
    sheet.add(RegionKind::math("x := 1234.5678"), Point::new(40.0, 700.0));
    let shown = sheet.add(RegionKind::math("x ="), Point::new(40.0, 760.0));
    let other = sheet.add(RegionKind::math("y := 0.5"), Point::new(40.0, 820.0));
    h.app.recalculate();
    h.settle();
    (h, shown, other)
}

fn select(h: &mut Harness, ids: &[RegionId]) {
    h.app.state.sheets[0].canvas.selection = ids.to_vec();
    h.frame();
}

/// The text a region shows for its result.
fn result(h: &Harness, id: RegionId) -> String {
    let sheet = h.sheet();
    let RegionKind::Math(m) = &sheet.doc.region(id).unwrap().kind else {
        panic!("not math")
    };
    match h.app.state.output.values.get(&id) {
        Some(Ok(v)) => format_value_with(v, sheet.doc.unit_system(), m.format()),
        other => panic!("no value: {other:?}"),
    }
}

fn notation(h: &Harness, id: RegionId) -> Notation {
    match &h.sheet().doc.region(id).unwrap().kind {
        RegionKind::Math(m) => m.format().notation(),
        _ => panic!("not math"),
    }
}

#[test]
fn more_and_fewer_decimals_change_how_the_result_reads_and_undo_one_step_each() {
    let (mut h, shown, _) = setup();
    assert_eq!(result(&h, shown), "1234.568");
    select(&mut h, &[shown]);
    h.command(CommandId::FormatDecimalsMore);
    assert_eq!(result(&h, shown), "1234.5678");
    h.command(CommandId::FormatDecimalsMore);
    assert_eq!(result(&h, shown), "1234.56780");
    h.command(CommandId::FormatDecimalsLess);
    h.command(CommandId::FormatDecimalsLess);
    h.command(CommandId::FormatDecimalsLess);
    assert_eq!(result(&h, shown), "1234.57");
    h.command(CommandId::EditUndo);
    assert_eq!(result(&h, shown), "1234.568", "the last press only");
    // Back to the beginning: the default again.
    for _ in 0..4 {
        h.command(CommandId::EditUndo);
    }
    assert_eq!(result(&h, shown), "1234.568");
}

#[test]
fn scientific_and_engineering_switch_on_and_off_and_exclude_each_other() {
    let (mut h, shown, _) = setup();
    select(&mut h, &[shown]);
    h.command(CommandId::FormatScientific);
    assert_eq!(result(&h, shown), "1.235e3");
    assert!(h
        .app
        .state
        .active_toggles()
        .contains(&CommandId::FormatScientific));
    h.command(CommandId::FormatEngineering);
    assert_eq!(result(&h, shown), "1.235e3", "1234 is 1.234e3 either way");
    assert_eq!(notation(&h, shown), Notation::Engineering);
    let on = h.app.state.active_toggles();
    assert!(
        on.contains(&CommandId::FormatEngineering) && !on.contains(&CommandId::FormatScientific)
    );
    // Pressing the one that is on puts the default back.
    h.command(CommandId::FormatEngineering);
    assert_eq!(notation(&h, shown), Notation::General);
    assert_eq!(result(&h, shown), "1234.568");
}

#[test]
fn engineering_notation_uses_exponents_in_threes() {
    let (mut h, _, other) = setup();
    select(&mut h, &[other]);
    h.command(CommandId::FormatEngineering);
    h.command(CommandId::FormatDecimalsMore);
    // y := 0.5 shows its result only if evaluated, so look at the format itself.
    let RegionKind::Math(m) = &h.sheet().doc.region(other).unwrap().kind else {
        panic!()
    };
    assert_eq!(m.format().notation(), Notation::Engineering);
    assert_eq!(m.format().decimals(), Some(4));
    assert_eq!(
        crate::number_format::number_in(15_000.0, m.format()),
        "15.0000e3"
    );
}

#[test]
fn only_math_regions_in_the_selection_are_formatted_and_nothing_is_recalculated() {
    let (mut h, shown, other) = setup();
    let text = h.app.state.sheets[0].add(
        RegionKind::Text {
            text: "note".into(),
        },
        Point::new(40.0, 900.0),
    );
    select(&mut h, &[shown, other, text]);
    let eval = h.sheet().doc.eval_revision();
    h.command(CommandId::FormatScientific);
    assert_eq!(notation(&h, shown), Notation::Scientific);
    assert_eq!(notation(&h, other), Notation::Scientific);
    assert_eq!(h.sheet().doc.eval_revision(), eval, "display only");
    // One undo reverses both regions.
    h.command(CommandId::EditUndo);
    assert_eq!(notation(&h, shown), Notation::General);
    assert_eq!(notation(&h, other), Notation::General);
}

#[test]
fn the_format_commands_need_a_math_region_selected() {
    let (mut h, shown, _) = setup();
    let text = h.app.state.sheets[0].add(
        RegionKind::Text { text: "n".into() },
        Point::new(40.0, 900.0),
    );
    let formats = [
        CommandId::FormatDecimalsMore,
        CommandId::FormatDecimalsLess,
        CommandId::FormatScientific,
        CommandId::FormatEngineering,
    ];
    select(&mut h, &[]);
    assert!(formats.iter().all(|c| !h.app.registry().allows(*c)));
    select(&mut h, &[text]);
    assert!(formats.iter().all(|c| !h.app.registry().allows(*c)));
    select(&mut h, &[shown]);
    assert!(formats.iter().all(|c| h.app.registry().allows(*c)));
    // Used with nothing to act on, a command changes nothing.
    select(&mut h, &[text]);
    let revision = h.sheet().doc.revision();
    h.command(CommandId::FormatScientific);
    assert_eq!(h.sheet().doc.revision(), revision);
}

#[test]
fn the_format_is_saved_with_the_worksheet_and_survives_copy_and_paste() {
    let (mut h, shown, other) = setup();
    select(&mut h, &[shown]);
    h.command(CommandId::FormatScientific);
    h.command(CommandId::FormatDecimalsMore);

    let loaded = load_bytes(&save_bytes(&h.sheet().doc).unwrap()).unwrap();
    assert!(loaded.repairs.is_empty());
    let RegionKind::Math(m) = &loaded.worksheet.region(shown).unwrap().kind else {
        panic!()
    };
    assert_eq!(m.format().notation(), Notation::Scientific);
    assert_eq!(m.format().decimals(), Some(4));
    let RegionKind::Math(plain) = &loaded.worksheet.region(other).unwrap().kind else {
        panic!()
    };
    assert!(plain.format().is_default());

    // Copy and paste keeps it.
    h.command(CommandId::ClipboardCopy);
    h.command(CommandId::ClipboardPaste);
    let pasted = *h.sheet().canvas.selection.first().unwrap();
    assert_ne!(pasted, shown);
    assert_eq!(notation(&h, pasted), Notation::Scientific);
}

/// A worksheet with `L := 2 m`, `L =` and `F := 5 kN`, `F =`.
fn setup_units() -> (Harness, RegionId, RegionId) {
    let mut h = Harness::new();
    h.settle();
    let sheet = &mut h.app.state.sheets[0];
    sheet.add(RegionKind::math("L := 2 m"), Point::new(40.0, 700.0));
    let length = sheet.add(RegionKind::math("L ="), Point::new(40.0, 760.0));
    sheet.add(RegionKind::math("F := 5 kN"), Point::new(40.0, 820.0));
    let force = sheet.add(RegionKind::math("F ="), Point::new(40.0, 880.0));
    h.app.recalculate();
    h.settle();
    (h, length, force)
}

#[test]
fn display_in_shows_a_result_in_the_chosen_unit_and_reset_puts_it_back() {
    let (mut h, length, force) = setup_units();
    assert_eq!(result(&h, length), "2 m");
    select(&mut h, &[length]);
    h.command(CommandId::UnitsDisplayMm);
    assert_eq!(result(&h, length), "2000 mm");
    h.command(CommandId::UnitsDisplayIn);
    assert_eq!(result(&h, length), "78.74 in");
    assert!(h
        .app
        .state
        .active_toggles()
        .contains(&CommandId::UnitsDisplayIn));
    h.command(CommandId::UnitsDisplayReset);
    assert_eq!(result(&h, length), "2 m");

    select(&mut h, &[force]);
    h.command(CommandId::UnitsDisplayN);
    assert_eq!(result(&h, force), "5000 N");
    h.command(CommandId::UnitsDisplayKn);
    assert_eq!(result(&h, force), "5 kN");
    // Undo goes back one choice at a time.
    h.command(CommandId::EditUndo);
    assert_eq!(result(&h, force), "5000 N");
}

#[test]
fn a_unit_of_another_kind_is_ignored_and_the_choice_combines_with_the_number_format() {
    let (mut h, length, force) = setup_units();
    select(&mut h, &[force]);
    h.command(CommandId::UnitsDisplayMm);
    assert_eq!(result(&h, force), "5 kN", "metres do not apply to a force");
    // Length in millimetres to two decimals, in scientific notation.
    select(&mut h, &[length]);
    h.command(CommandId::UnitsDisplayMm);
    h.command(CommandId::FormatDecimalsLess);
    h.command(CommandId::FormatScientific);
    assert_eq!(result(&h, length), "2.00e3 mm");
    // A format change keeps the unit, and the other way round.
    h.command(CommandId::FormatScientific);
    assert_eq!(result(&h, length), "2000.00 mm");
}

#[test]
fn display_in_is_saved_and_the_buttons_need_a_math_region() {
    let (mut h, length, _) = setup_units();
    let formats = [
        CommandId::UnitsDisplayM,
        CommandId::UnitsDisplayMm,
        CommandId::UnitsDisplayIn,
        CommandId::UnitsDisplayN,
        CommandId::UnitsDisplayKn,
        CommandId::UnitsDisplayReset,
    ];
    select(&mut h, &[]);
    assert!(formats.iter().all(|c| !h.app.registry().allows(*c)));
    select(&mut h, &[length]);
    assert!(formats.iter().all(|c| h.app.registry().allows(*c)));
    h.command(CommandId::UnitsDisplayMm);
    let bytes = save_bytes(&h.sheet().doc).unwrap();
    let loaded = load_bytes(&bytes).unwrap();
    let RegionKind::Math(m) = &loaded.worksheet.region(length).unwrap().kind else {
        panic!()
    };
    assert_eq!(
        m.format().unit(),
        Some(opencalc_core::document::DisplayUnit::Millimetre)
    );
}
