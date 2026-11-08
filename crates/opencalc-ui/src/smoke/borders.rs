//! Region borders: the command, undo, and the line on the page.

#![cfg(test)]

use opencalc_core::document::{Point, RegionKind, Worksheet};
use opencalc_core::edit::Edit;
use opencalc_core::eval::evaluate_with;

use super::Harness;
use crate::commands::CommandId;
use crate::export::{render_pages, Source};
use crate::theme::Theme;

fn border_of(h: &Harness, n: usize) -> bool {
    h.sheet().doc.regions()[n].border
}

#[test]
fn the_border_command_toggles_every_selected_region_as_one_undo_step() {
    let mut h = Harness::new();
    h.settle();
    let ids: Vec<_> = h
        .sheet()
        .doc
        .regions()
        .iter()
        .take(2)
        .map(|r| r.id)
        .collect();
    h.app.state.sheets[0].canvas.selection = ids;
    h.frame();
    h.command(CommandId::FormatBorder);
    assert!(border_of(&h, 0) && border_of(&h, 1));
    assert!(h
        .app
        .state
        .active_toggles()
        .contains(&CommandId::FormatBorder));
    h.command(CommandId::FormatBorder);
    assert!(!border_of(&h, 0) && !border_of(&h, 1));
    h.command(CommandId::EditUndo);
    assert!(
        border_of(&h, 0) && border_of(&h, 1),
        "one undo restores both"
    );
}

#[test]
fn a_locked_region_keeps_its_border_setting() {
    let mut h = Harness::new();
    h.settle();
    let id = h.sheet().doc.regions()[0].id;
    h.app.state.sheets[0].edit(Edit::SetLocked { id, locked: true });
    h.app.state.sheets[0].canvas.selection = vec![id];
    h.frame();
    h.command(CommandId::FormatBorder);
    assert!(!border_of(&h, 0));
}

#[test]
fn the_border_is_drawn_in_the_export_round_the_region() {
    let mut doc = Worksheet::new("t");
    let r = doc.new_region(
        RegionKind::Area {
            title: String::new(),
        },
        Point::new(100.0, 100.0),
    );
    let id = r.id;
    doc.apply(Edit::InsertRegions {
        regions: vec![(0, r)],
    })
    .unwrap();
    let out = |doc: &Worksheet| {
        let output = evaluate_with(&doc.math_regions(), &doc.units().unwrap());
        let theme = Theme::paper();
        let source = Source {
            doc,
            output: &output,
            theme: &theme,
        };
        let mut pages = Vec::new();
        render_pages(&source, 1.0, |_, i, _| {
            pages.push(i);
            Ok(())
        })
        .unwrap();
        pages.remove(0)
    };
    let dark = |im: &crate::export::raster::Image| {
        let mut n = 0;
        for y in 90..320 {
            for x in 90..440 {
                let [r, g, b] = im.pixel(x, y);
                if u32::from(r) + u32::from(g) + u32::from(b) < 150 {
                    n += 1;
                }
            }
        }
        n
    };
    let plain = dark(&out(&doc));
    doc.apply(Edit::SetBorder { id, border: true }).unwrap();
    let boxed = dark(&out(&doc));
    assert!(
        boxed > plain + 400,
        "a box outline of a 320 by 200 region: {plain} -> {boxed}"
    );
}
