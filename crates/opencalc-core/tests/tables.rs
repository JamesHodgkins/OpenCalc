//! Table regions: they define a name as a matrix, take part in recalculation,
//! undo, saving, and loading old files.

use opencalc_core::document::{Point, RegionId, RegionKind, TableSpec, Worksheet};
use opencalc_core::edit::{Edit, History};
use opencalc_core::eval::{evaluate_all, Output};
use opencalc_core::persist::{load_bytes, save_bytes};
use opencalc_core::registry::UnitRegistry;

fn add(w: &mut Worksheet, kind: RegionKind) -> RegionId {
    let r = w.new_region(kind, Point::default());
    let id = r.id;
    let at = w.regions().len();
    w.apply(Edit::InsertRegions {
        regions: vec![(at, r)],
    })
    .unwrap();
    id
}

fn run(w: &Worksheet) -> Output {
    evaluate_all(
        &w.math_regions(),
        &w.plot_inputs(),
        &UnitRegistry::builtin(),
    )
}

fn forces() -> TableSpec {
    TableSpec::default()
        .with_name("F")
        .with_unit(0, "m")
        .with_unit(1, "kN")
        .with_cell(0, 0, "1")
        .with_cell(0, 1, "10")
        .with_cell(1, 0, "2")
        .with_cell(1, 1, "20")
        .with_cell(2, 0, "3")
        .with_cell(2, 1, "30")
}

fn sheet() -> (Worksheet, RegionId, RegionId) {
    let mut w = Worksheet::new("t");
    let table = add(&mut w, RegionKind::table());
    w.apply(Edit::SetTable {
        id: table,
        spec: Box::new(forces()),
    })
    .unwrap();
    // reads the whole second column; the unit comes from the table
    let m = add(&mut w, RegionKind::math("total := sum(col(F, 1)) / 1 kN"));
    (w, table, m)
}

#[test]
fn a_table_defines_a_matrix_that_other_regions_use_with_its_units() {
    let (w, table, m) = sheet();
    let out = run(&w);
    assert!(out.values[&table].is_ok(), "{:?}", out.values[&table]);
    let total = out.values[&m].as_ref().unwrap();
    assert!(format!("{total:?}").contains("60"), "{total:?}");
}

#[test]
fn a_bad_cell_is_an_error_on_the_table_and_dependents_fail() {
    let (mut w, table, m) = sheet();
    w.apply(Edit::SetTable {
        id: table,
        spec: Box::new(forces().with_cell(1, 1, "oops")),
    })
    .unwrap();
    let out = run(&w);
    let e = out.values[&table].as_ref().unwrap_err().to_string();
    assert!(e.contains("row 2, column 2"), "{e}");
    assert!(out.values[&m].is_err());
}

#[test]
fn editing_a_table_recalculates_and_undoes_as_one_step() {
    let (mut w, table, m) = sheet();
    let mut h = History::default();
    let before = w.eval_revision();
    h.apply(
        &mut w,
        Edit::SetTable {
            id: table,
            spec: Box::new(forces().with_cell(0, 1, "100")),
        },
    )
    .unwrap();
    assert!(w.eval_revision() > before);
    let after = format!("{:?}", run(&w).values[&m]);
    assert!(after.contains("150"), "{after}");
    assert!(h.undo(&mut w).unwrap());
    assert!(format!("{:?}", run(&w).values[&m]).contains("60"));
}

#[test]
fn a_blank_table_defines_nothing_and_does_not_disturb_results() {
    let mut w = Worksheet::new("t");
    let table = add(&mut w, RegionKind::table());
    let out = run(&w);
    assert!(!out.values.contains_key(&table));
    let e = w.eval_revision();
    w.apply(Edit::DeleteRegions { ids: vec![table] }).unwrap();
    assert_eq!(w.eval_revision(), e);
}

#[test]
fn a_table_survives_saving() {
    let (w, table, _) = sheet();
    let back = load_bytes(&save_bytes(&w).unwrap()).unwrap().worksheet;
    assert_eq!(back, w);
    assert!(run(&back).values[&table].is_ok());
}

#[test]
fn a_border_is_an_undoable_display_edit_that_is_saved() {
    let (mut w, table, _) = sheet();
    let e = w.eval_revision();
    let mut h = History::default();
    h.apply(
        &mut w,
        Edit::SetBorder {
            id: table,
            border: true,
        },
    )
    .unwrap();
    assert_eq!(w.eval_revision(), e, "a border changes no result");
    let back = load_bytes(&save_bytes(&w).unwrap()).unwrap().worksheet;
    assert!(back.region(table).unwrap().border);
    assert!(h.undo(&mut w).unwrap());
    assert!(!w.region(table).unwrap().border);
    // a locked region refuses
    w.apply(Edit::SetLocked {
        id: table,
        locked: true,
    })
    .unwrap();
    assert!(w
        .apply(Edit::SetBorder {
            id: table,
            border: true
        })
        .is_err());
}
