//! Table regions in the running window: inserting, the editing window, results
//! and drawing.

#![cfg(test)]

use opencalc_core::document::{Point, RegionId, RegionKind, TableSpec, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::eval::evaluate_with;

use super::Harness;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;
use crate::export::raster::Image;
use crate::export::{render_pages, Source};
use crate::panels::table_window::selected_table;
use crate::theme::Theme;

fn table_id(h: &Harness) -> RegionId {
    h.sheet()
        .doc
        .regions()
        .iter()
        .find(|r| matches!(r.kind, RegionKind::Table(_)))
        .expect("a table")
        .id
}

fn edit(h: &mut Harness, id: RegionId, spec: TableSpec) {
    let i = h.app.state.active.expect("a sheet");
    let cmds = vec![
        CanvasCmd::Edit(Edit::SetTable {
            id,
            spec: Box::new(spec),
        }),
        CanvasCmd::Seal,
    ];
    crate::app::dispatch::apply_canvas(&mut h.app.state, i, cmds);
    h.app.recalculate();
    h.settle();
}

fn loads() -> TableSpec {
    TableSpec::default()
        .with_name("loads")
        .with_header(0, "x")
        .with_unit(0, "m")
        .with_unit(1, "kN")
        .with_cell(0, 0, "1")
        .with_cell(0, 1, "10")
        .with_cell(1, 0, "2")
        .with_cell(1, 1, "20")
        .with_cell(2, 0, "3")
        .with_cell(2, 1, "30")
}

#[test]
fn inserting_a_table_selects_it_and_opens_the_table_window() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::InsertTable);
    let id = table_id(&h);
    assert_eq!(selected_table(&h.app.state).map(|(i, _)| i), Some(id));
    h.frame();
    // Not a table: no window.
    let first = h.sheet().doc.regions()[0].id;
    h.app.state.sheets[0].canvas.selection = vec![first];
    assert!(selected_table(&h.app.state).is_none());
    h.frame();
}

#[test]
fn a_filled_table_gives_a_matrix_that_a_formula_can_use() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::InsertTable);
    let id = table_id(&h);
    edit(&mut h, id, loads());
    assert!(h.app.state.output.values[&id].is_ok());
    // undo is one step
    h.command(CommandId::EditUndo);
    h.settle();
    assert!(!h.app.state.output.values.contains_key(&id));
}

fn pixels_for(table: TableSpec) -> Image {
    let mut doc = Worksheet::new("t");
    let r = doc.new_region(
        RegionKind::Table(Box::new(table.clone())),
        Point::new(40.0, 40.0),
    );
    doc.apply(Edit::InsertRegions {
        regions: vec![(0, r)],
    })
    .unwrap();
    let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
    let theme = Theme::paper();
    let source = Source {
        doc: &doc,
        output: &output,
        theme: &theme,
    };
    let mut pages = Vec::new();
    render_pages(&source, 1.0, |_, image, _| {
        pages.push(image);
        Ok(())
    })
    .unwrap();
    pages.remove(0)
}

fn ink(image: &Image) -> usize {
    let mut n = 0;
    for y in 0..image.height {
        for x in 0..image.width {
            let [r, g, b] = image.pixel(x, y);
            if u32::from(r) + u32::from(g) + u32::from(b) < 600 {
                n += 1;
            }
        }
    }
    n
}

fn red(image: &Image) -> usize {
    let mut n = 0;
    for y in 0..image.height {
        for x in 0..image.width {
            let [r, g, b] = image.pixel(x, y);
            if r > 150 && g < 100 && b < 100 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn a_table_is_drawn_and_a_bad_cell_shows_an_error() {
    let good = pixels_for(loads());
    let blank = pixels_for(TableSpec::default());
    assert!(ink(&good) > ink(&blank) + 200, "numbers are drawn");
    assert_eq!(red(&good), 0);
    let bad = pixels_for(loads().with_cell(1, 1, "oops"));
    assert!(red(&bad) > 20, "the error is drawn in red");
}
