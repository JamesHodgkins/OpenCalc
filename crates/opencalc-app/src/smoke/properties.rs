//! The Properties side panel: the settings of the selected plot, table, picture or
//! text sit beside the page, not over it.

#![cfg(test)]

use opencalc_core::document::{Point, RegionId, RegionKind};

use super::{Harness, F};
use crate::panels::properties::{wanted, Kind};

fn add(h: &mut Harness, kind: RegionKind) -> RegionId {
    h.app.state.sheets[0].add(kind, Point::new(40.0, 700.0))
}

#[test]
fn each_selection_gets_its_own_settings_and_a_math_region_gets_none() {
    let mut h = Harness::new();
    h.settle();
    h.app.state.sheets[0].canvas.selection = vec![F];
    h.frame();
    assert_eq!(wanted(&h.app.state), None, "a math region has no panel");
    for (kind, expect) in [
        (RegionKind::plot(), Kind::Plot),
        (RegionKind::table(), Kind::Table),
        (RegionKind::picture(), Kind::Picture),
        (
            RegionKind::Text {
                text: "note".into(),
            },
            Kind::Text,
        ),
    ] {
        let id = add(&mut h, kind);
        h.app.state.sheets[0].canvas.selection = vec![id];
        h.frame();
        h.frame();
        assert_eq!(wanted(&h.app.state), Some(expect));
    }
}

#[test]
fn the_panel_takes_room_from_the_page_instead_of_covering_it() {
    let mut h = Harness::new();
    h.settle();
    h.frame();
    let wide = h.app.state.viewport.width();
    let id = add(&mut h, RegionKind::plot());
    h.app.state.sheets[0].canvas.selection = vec![id];
    h.frame();
    h.frame();
    let narrow = h.app.state.viewport.width();
    assert!(
        narrow < wide - 200.0,
        "the page is narrower beside the panel: {narrow} against {wide}"
    );
    // Letting go of the selection gives the room back.
    h.app.state.sheets[0].canvas.selection.clear();
    h.frame();
    h.frame();
    assert!((h.app.state.viewport.width() - wide).abs() < 1.0);
}
