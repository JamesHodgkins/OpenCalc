//! The Options window and the settings it changes, driven through real frames.

use super::*;
use crate::panels::options::{self, Section};
use crate::settings::{GridSpacing, Settings};
use opencalc_core::document::{Background, PageSetup, PaperSize};
use opencalc_core::registry::UnitSystem;

fn change(h: &mut Harness, edit: impl FnOnce(&mut Settings)) {
    let mut next = h.app.state.settings;
    edit(&mut next);
    let ctx = h.ctx.clone();
    h.app.change_settings(&ctx, next);
    h.frame();
}

#[test]
fn every_section_of_the_options_window_draws() {
    let mut h = Harness::new();
    h.command(CommandId::AppOptions);
    assert!(h.app.state.options_open);
    for section in Section::ALL {
        options::select(&h.ctx, section);
        h.frame();
        h.frame();
        assert!(h.app.state.options_open, "{section:?}");
    }
    h.command(CommandId::AppOptionsClose);
    assert!(!h.app.state.options_open);
}

#[test]
fn the_interface_size_scales_the_whole_window() {
    let mut h = Harness::new();
    h.frame();
    change(&mut h, |s| s.scale = 125);
    assert_eq!(h.ctx.zoom_factor(), 1.25);
    change(&mut h, |s| s.scale = 100);
    assert_eq!(h.ctx.zoom_factor(), 1.0);
}

#[test]
fn a_new_grid_applies_to_open_worksheets_and_dragging_snaps_to_it() {
    let mut h = Harness::new();
    h.settle();
    change(&mut h, |s| s.grid = GridSpacing::Mm5);
    assert_eq!(h.sheet().canvas.grid, GridSpacing::Mm5);
    let step = GridSpacing::Mm5.step();
    // The insertion point moved onto the new grid too.
    let at = h.sheet().canvas.insert_at;
    for v in [at.x, at.y] {
        assert!(((v / step) - (v / step).round()).abs() < 1e-3, "{at:?}");
    }

    h.drag(F);
    let moved = h.region_pos(F);
    for v in [moved.x, moved.y] {
        let steps = v / step;
        assert!((steps - steps.round()).abs() < 1e-3, "{moved:?} on {step}");
    }
}

#[test]
fn a_new_worksheet_starts_with_the_default_units_and_page_and_is_unmodified() {
    let mut h = Harness::new();
    h.frame();
    let page = PageSetup {
        paper: PaperSize::Letter,
        landscape: true,
        margin_mm: 15.0,
        background: Background::Grid,
        ..PageSetup::default()
    };
    change(&mut h, |s| {
        s.units = UnitSystem::Uscs;
        s.page = page;
        s.grid = GridSpacing::Quarter;
    });
    // The open worksheet keeps its own.
    assert_eq!(h.sheet().doc.unit_system(), UnitSystem::Si);
    assert_eq!(h.sheet().doc.page(), PageSetup::default());

    h.command(CommandId::FileNew);
    let new = h.sheet();
    assert_eq!(new.doc.unit_system(), UnitSystem::Uscs);
    assert_eq!(new.doc.page(), page);
    assert_eq!(new.canvas.grid, GridSpacing::Quarter);
    assert!(!new.dirty(), "the defaults are not a change");
    assert!(!new.can_undo(), "nor something to undo");
    // The first input sits inside the margin, on the grid.
    let at = new.canvas.insert_at;
    let margin = page.margin();
    assert!(at.x >= margin && at.y >= margin, "{at:?}");
    assert_eq!(at.x % GridSpacing::Quarter.step(), 0.0);
}

#[test]
fn restore_defaults_puts_back_every_setting_and_the_theme() {
    let mut h = Harness::new();
    h.frame();
    h.command(CommandId::ThemeSlate);
    h.command(CommandId::ViewGrid);
    change(&mut h, |s| {
        s.scale = 150;
        s.grid = GridSpacing::Mm2;
        s.recovery = 0;
    });
    h.command(CommandId::AppOptionsDefaults);
    assert_eq!(h.app.state.settings, Settings::default());
    assert_eq!(h.app.state.palette, PaletteKey::Neutral);
    assert!(h.app.state.show_grid && h.app.state.snap);
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    assert_eq!(h.sheet().canvas.grid, GridSpacing::default());
}

#[test]
fn clear_recent_list_empties_the_file_menu_list() {
    let mut h = Harness::new();
    h.app.state.recent = vec!["a.ocalc".into(), "b.ocalc".into()];
    h.command(CommandId::FileClearRecent);
    assert!(h.app.state.recent.is_empty());
}
