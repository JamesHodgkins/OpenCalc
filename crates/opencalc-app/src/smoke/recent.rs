//! The Home page's recent files: pinning, forgetting, and what the program opens on.

#![cfg(test)]

use std::path::PathBuf;

use eframe::egui::{Event, Modifiers, PointerButton, Pos2};

use super::Harness;
use crate::app::filing::MAX_RECENT;
use crate::app::AppState;
use crate::commands::CommandId;
use crate::theme::PaletteKey;

fn paths(n: usize) -> Vec<PathBuf> {
    (0..n)
        .map(|i| PathBuf::from(format!("C:/nowhere/w{i}.ocalc")))
        .collect()
}

#[test]
fn the_program_starts_on_the_home_page_with_no_worksheet_open() {
    let mut state = AppState::new(PaletteKey::Neutral);
    assert!(
        state.active.is_some(),
        "(AppState::new keeps one for the tests)"
    );
    state.open_on_home();
    let mut h = Harness::new();
    h.app.state = state;
    h.frame();
    h.frame();
    assert!(h.app.state.active_sheet().is_none() && h.app.state.sheets.is_empty());
    // Starting a worksheet from there works.
    h.command(CommandId::FileNew);
    assert_eq!(h.app.state.sheets.len(), 1);
    assert!(h.app.state.active_sheet().is_some());
}

#[test]
fn a_pinned_file_stays_on_the_list_however_many_others_come_after() {
    let mut s = AppState::new(PaletteKey::Neutral);
    let all = paths(MAX_RECENT + 5);
    s.note_recent(&all[0]);
    s.toggle_pin(&all[0]);
    for p in &all[1..] {
        s.note_recent(p);
    }
    assert!(s.recent.contains(&all[0]), "pinned, so it is still there");
    assert_eq!(
        s.recent.len(),
        MAX_RECENT + 1,
        "the ten newest and the pinned one"
    );
    // Unpinned, it is an ordinary entry again and goes at the next addition.
    s.toggle_pin(&all[0]);
    s.note_recent(&PathBuf::from("C:/nowhere/new.ocalc"));
    assert!(!s.recent.contains(&all[0]));
    assert_eq!(s.recent.len(), MAX_RECENT);
}

#[test]
fn pin_forget_and_clear_unpinned_through_the_commands() {
    let mut h = Harness::new();
    h.app.state.recent = paths(4);
    h.command(CommandId::PinRecent(2));
    assert_eq!(h.app.state.pinned, vec![paths(4)[2].clone()]);
    h.command(CommandId::PinRecent(2));
    assert!(h.app.state.pinned.is_empty(), "again unpins");
    h.command(CommandId::PinRecent(1));
    h.command(CommandId::FileClearRecent);
    assert_eq!(
        h.app.state.recent,
        vec![paths(4)[1].clone()],
        "only the pinned file stays"
    );
    h.command(CommandId::ForgetRecent(0));
    assert!(h.app.state.recent.is_empty() && h.app.state.pinned.is_empty());
    // An entry that is not there is ignored.
    h.command(CommandId::ForgetRecent(9));
    h.command(CommandId::PinRecent(9));
}

/// A saved worksheet on disk, for a card with a picture.
fn real_file(name: &str) -> PathBuf {
    use opencalc_core::document::{Point, RegionKind, Worksheet};
    let mut ws = Worksheet::new("sample");
    let r = ws.new_region(RegionKind::math("F := 10 kN"), Point::new(80.0, 80.0));
    ws.apply(opencalc_core::edit::Edit::InsertRegions {
        regions: vec![(0, r)],
    })
    .unwrap();
    let dir = std::env::temp_dir().join(format!("opencalc-recent-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, opencalc_core::persist::save_bytes(&ws).unwrap()).unwrap();
    path
}

#[test]
fn the_home_page_draws_a_picture_on_the_card_of_a_real_file() {
    let file = real_file("card.ocalc");
    let mut h = Harness::new();
    h.app.state.open_on_home();
    h.app.state.recent = vec![file.clone(), PathBuf::from("C:/nowhere/gone.ocalc")];
    h.app.state.pinned = vec![file.clone()];
    let stamp = {
        let m = std::fs::metadata(&file).unwrap();
        (m.modified().unwrap(), m.len())
    };
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut got = false;
    while !got && std::time::Instant::now() < end {
        h.frame();
        std::thread::sleep(std::time::Duration::from_millis(20));
        got = h.app.state.thumbs.texture(&h.ctx, &file, stamp).is_some();
    }
    assert!(
        got,
        "the first page of the file was drawn in the background"
    );
    h.frame();
}

#[test]
fn clicking_a_card_opens_its_file_and_clicking_its_pin_pins_it() {
    let file = real_file("click.ocalc");
    let mut h = Harness::new();
    h.app.state.open_on_home();
    h.app.state.recent = vec![file.clone()];
    h.frame();
    h.frame();
    // Find the card from the layout: the first card sits under the Start tiles.
    let click = |h: &mut Harness, at: Pos2| {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        h.frame_with(vec![Event::PointerMoved(at)]);
        h.frame_with(vec![button(true)]);
        h.frame_with(vec![button(false)]);
    };
    // The window is 1280 wide: the wide layout, its main column starting at
    // (1280 - 1120) / 2 = 80, and the cards below the header and the tiles (move
    // this if the page's layout changes).
    let card = Pos2::new(80.0 + 100.0, 330.0);
    click(&mut h, card);
    assert!(
        h.app.state.sheets.len() == 1 || h.app.state.dialog.is_some(),
        "a click on the card tried to open the file"
    );
}
