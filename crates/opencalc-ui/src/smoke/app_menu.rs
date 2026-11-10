//! The File menu behind the app button, driven through real frames.

use super::*;

/// The middle of the app button, at the left of the title row.
const BUTTON: Pos2 = Pos2::new(36.0, 16.0);

fn click_at(h: &mut Harness, p: Pos2) {
    h.frame_with(vec![Event::PointerMoved(p)]);
    h.frame_with(vec![press(p)]);
    h.frame_with(vec![release(p)]);
    h.frame();
}

fn is_open(h: &Harness) -> bool {
    h.app.state.app_menu_open
}

#[test]
fn the_app_button_opens_the_menu_and_closes_it_again() {
    let mut h = Harness::new();
    h.frame();
    click_at(&mut h, BUTTON);
    assert!(is_open(&h));
    click_at(&mut h, BUTTON);
    assert!(!is_open(&h));
}

#[test]
fn alt_f_opens_it_and_escape_or_a_click_outside_closes_it() {
    let mut h = Harness::new();
    h.frame();
    h.key(Key::F, Modifiers::ALT);
    assert!(is_open(&h));
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!is_open(&h));

    h.key(Key::F, Modifiers::ALT);
    assert!(is_open(&h));
    click_at(&mut h, Pos2::new(900.0, 600.0));
    assert!(!is_open(&h));
}

#[test]
fn running_any_command_closes_it() {
    let mut h = Harness::new();
    h.command(CommandId::AppMenu);
    assert!(is_open(&h));
    h.command(CommandId::ViewZoomIn);
    assert!(!is_open(&h));
}

#[test]
fn the_arrow_keys_and_enter_run_a_row() {
    let mut h = Harness::new();
    h.frame();
    let sheets = h.app.state.sheets.len();
    h.key(Key::F, Modifiers::ALT);
    // The first row is New worksheet.
    h.key(Key::ArrowDown, Modifiers::NONE);
    h.key(Key::Enter, Modifiers::NONE);
    h.frame();
    assert_eq!(h.app.state.sheets.len(), sheets + 1);
    assert!(!is_open(&h), "running a row closes the menu");
}

#[test]
fn right_enters_the_pane_of_choices_under_new() {
    let mut h = Harness::new();
    h.frame();
    let sheets = h.app.state.sheets.len();
    h.key(Key::F, Modifiers::ALT);
    h.key(Key::ArrowDown, Modifiers::NONE); // New
    h.key(Key::ArrowRight, Modifiers::NONE); // New worksheet
    h.key(Key::ArrowDown, Modifiers::NONE); // New from template
    h.key(Key::ArrowDown, Modifiers::NONE); // Open example
    h.key(Key::Enter, Modifiers::NONE);
    h.frame();
    assert_eq!(h.app.state.sheets.len(), sheets + 1);
    let title = h.app.state.sheets.last().unwrap().doc.title().to_owned();
    assert_eq!(title, OpenSheet::sample().doc.title(), "the example opened");
}

#[test]
fn a_number_opens_that_recent_worksheet() {
    let mut h = Harness::new();
    let gone = std::env::temp_dir().join("opencalc-app-menu-no-such-file.ocx");
    h.app.state.recent = vec![gone.clone()];
    h.frame();
    h.key(Key::F, Modifiers::ALT);
    // Two frames with the recent list showing paint it.
    h.frame();
    h.key(Key::Num1, Modifiers::NONE);
    h.frame();
    assert!(!is_open(&h));
    // The file is not there, so opening it says so.
    assert!(h.app.state.dialog.is_some(), "OpenRecent(0) ran");
}

#[test]
fn it_paints_on_the_home_page_with_the_file_commands_dimmed() {
    let mut h = Harness::new();
    h.command(CommandId::TabHome);
    h.key(Key::F, Modifiers::ALT);
    // Save is dimmed, so Down from New skips it and the menu does not panic
    // going round the column.
    for _ in 0..12 {
        h.key(Key::ArrowDown, Modifiers::NONE);
    }
    h.key(Key::ArrowRight, Modifiers::NONE);
    h.key(Key::ArrowLeft, Modifiers::NONE);
    assert!(is_open(&h));
}

#[test]
fn a_pointer_resting_on_a_row_does_not_undo_what_the_keys_chose() {
    let mut h = Harness::new();
    h.frame();
    let sheets = h.app.state.sheets.len();
    h.key(Key::F, Modifiers::ALT);
    // Rest the pointer on Open (the second row), whose pane is the recent list.
    let open_row = Pos2::new(100.0, 32.0 + 1.0 + 6.0 + 34.0 * 1.5);
    // (The first position egui hears of is not a move, so come from beside it.)
    h.frame_with(vec![Event::PointerMoved(open_row - egui::vec2(10.0, 0.0))]);
    h.frame_with(vec![Event::PointerMoved(open_row)]);
    h.key(Key::ArrowUp, Modifiers::NONE); // New, and its pane
    for _ in 0..8 {
        h.frame(); // well past the time a resting pointer takes to open a pane
    }
    h.key(Key::ArrowRight, Modifiers::NONE); // New worksheet
    h.key(Key::ArrowUp, Modifiers::NONE); // Open example
    h.key(Key::Enter, Modifiers::NONE);
    h.frame();
    assert_eq!(h.app.state.sheets.len(), sheets + 1);
    let title = h.app.state.sheets.last().unwrap().doc.title().to_owned();
    assert_eq!(title, OpenSheet::sample().doc.title());
}
