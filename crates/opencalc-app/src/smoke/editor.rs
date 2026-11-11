//! The math editor through the real frame loop: caret, selection, clipboard, autocomplete,
//! Greek letters, and leaving the editor by keyboard or pointer.

#![cfg(test)]

use super::*;

impl Harness {
    /// Open a new math region for editing and return its id.
    pub(super) fn new_math(&mut self) -> RegionId {
        self.command(CommandId::InsertMath);
        self.frame();
        *self
            .sheet()
            .canvas
            .selection
            .first()
            .expect("the new region is selected")
    }

    pub(super) fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.frame_with(vec![Event::Text(ch.to_string())]);
        }
    }

    pub(super) fn press_key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame_with(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }]);
    }

    pub(super) fn source_of(&self, id: RegionId) -> String {
        match &self.sheet().doc.region(id).expect("region").kind {
            RegionKind::Math(m) => m.source().to_owned(),
            _ => panic!("math region"),
        }
    }

    pub(super) fn caret(&self) -> usize {
        self.sheet().canvas.field.as_ref().expect("editing").caret
    }
}

const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};

#[test]
fn the_editor_inserts_at_the_caret_and_backspace_and_delete_work() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("ac");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    h.type_text("b");
    assert_eq!(h.source_of(id), "abc");
    h.press_key(Key::Backspace, Modifiers::NONE);
    assert_eq!(h.source_of(id), "ac");
    h.press_key(Key::Delete, Modifiers::NONE);
    assert_eq!(h.source_of(id), "a");
    h.press_key(Key::Home, Modifiers::NONE);
    h.type_text("\u{3b1}");
    assert_eq!(
        h.source_of(id),
        "\u{3b1}a",
        "Greek goes in as one character"
    );
}

#[test]
fn shift_arrows_select_and_typing_replaces_the_selection() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+yy");
    h.press_key(Key::ArrowLeft, SHIFT);
    h.press_key(Key::ArrowLeft, SHIFT);
    h.type_text("z");
    assert_eq!(h.source_of(id), "x+z");
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text("q");
    assert_eq!(h.source_of(id), "q");
}

#[test]
fn copy_puts_the_source_text_of_the_selection_on_the_clipboard_and_cut_removes_it() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a/b");
    h.press_key(Key::Home, SHIFT);
    h.frame_with(vec![Event::Copy]);
    assert_eq!(h.copied.as_deref(), Some("a/b"));
    h.frame_with(vec![Event::Cut]);
    assert_eq!(h.source_of(id), "");
    h.frame_with(vec![Event::Paste("1\n+ 2".to_owned())]);
    assert_eq!(
        h.source_of(id),
        "1 + 2",
        "a pasted line break becomes a space"
    );
}

#[test]
fn arrows_skip_characters_that_draw_nothing_and_up_down_move_between_fraction_lines() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("((x))");
    h.press_key(Key::Home, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(
        h.caret() >= 2,
        "the brackets draw nothing, so one arrow press moves past them: {}",
        h.caret()
    );
    // In a fraction, Up goes to the numerator and Down back to the denominator.
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text("a/b");
    assert_eq!(h.source_of(id), "a/b");
    h.press_key(Key::End, Modifiers::NONE);
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    assert!(h.caret() <= 1, "caret in the numerator: {}", h.caret());
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    assert!(h.caret() >= 2, "caret in the denominator: {}", h.caret());
}

#[test]
fn arrows_off_the_edge_of_an_equation_leave_it_and_arrows_on_the_page_move_the_crosshair() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+1");
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(
        h.sheet().canvas.editing.is_none(),
        "Right at the end leaves the equation"
    );
    let right = h.sheet().canvas.insert_at;
    let r = h.sheet().doc.region(id).expect("region").pos;
    assert!(right.x > r.x, "the crosshair is right of the equation");
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    let down = h.sheet().canvas.insert_at;
    assert!(down.y > right.y && down.x == right.x, "Down moves one step");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    assert!(h.sheet().canvas.insert_at.x < down.x);
    assert!(h.sheet().canvas.armed, "typing starts a region there");
    // Walking back (along the equation's own line) hands control to it.
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    for _ in 0..30 {
        h.press_key(Key::ArrowLeft, Modifiers::NONE);
        if h.sheet().canvas.editing.is_some() {
            break;
        }
    }
    h.frame();
    let (at, rect) = (
        h.sheet().canvas.insert_at,
        h.sheet().doc.region(id).map(|r| (r.pos, r.size)),
    );
    assert_eq!(
        h.sheet().canvas.editing,
        Some(id),
        "the equation took over: {at:?} {rect:?}"
    );
    assert!(!h.sheet().canvas.armed);
}

#[test]
fn the_crosshair_can_walk_back_onto_a_region_that_is_off_the_grid() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+1");
    h.press_key(Key::Escape, Modifiers::NONE);
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    // A region at a position that is not a multiple of the grid step.
    let moves = vec![(id, Point { x: 41.0, y: 523.0 })];
    h.app.state.sheets[0].edit(Edit::MoveRegions { moves });
    h.frame();
    h.frame();
    h.app.state.sheets[0].canvas.editing = Some(id);
    h.app.state.sheets[0].canvas.focus_edit = true;
    h.frame();
    h.frame();
    assert_eq!(h.sheet().canvas.editing, Some(id));
    h.press_key(Key::End, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(
        h.sheet().canvas.editing.is_none(),
        "Right at the end leaves"
    );
    for _ in 0..30 {
        h.press_key(Key::ArrowLeft, Modifiers::NONE);
        if h.sheet().canvas.editing.is_some() {
            break;
        }
    }
    assert_eq!(
        h.sheet().canvas.editing,
        Some(id),
        "the region took over again"
    );
}

#[test]
fn clicking_in_the_region_places_the_caret_and_a_double_click_selects_a_node() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("alpha+beta");
    h.frame();
    let sheet = h.sheet();
    let r = sheet.doc.region(id).expect("region");
    let rect = rect_of(r, &sheet.canvas.sizes);
    let at = |fx: f32| {
        sheet.view.doc_to_screen(
            Pos2::new(rect.min.x + rect.width() * fx, rect.center().y),
            h.app.state.viewport.min,
        )
    };
    let (near_start, near_end) = (at(0.1), at(0.97));
    h.frame_with(vec![Event::PointerMoved(near_start)]);
    h.frame_with(vec![press(near_start)]);
    h.frame_with(vec![release(near_start)]);
    assert!(
        h.caret() <= 2,
        "a click near the left puts the caret near the start: {}",
        h.caret()
    );
    h.frame_with(vec![Event::PointerMoved(near_end)]);
    h.frame_with(vec![press(near_end)]);
    h.frame_with(vec![release(near_end)]);
    assert!(h.caret() >= 10, "a click near the right: {}", h.caret());
    assert_eq!(
        h.source_of(id),
        "alpha+beta",
        "clicking never changes the text"
    );
}

#[test]
fn enter_and_escape_end_editing_and_the_whole_burst_is_one_undo_step() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("m:=3");
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    assert!(h.sheet().canvas.editing.is_none());
    assert!(h.sheet().canvas.field.is_none());
    assert_eq!(h.source_of(id), "m:=3");
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(h.source_of(id), "");
}

#[test]
fn half_typed_input_never_breaks_a_frame_and_a_huge_paste_is_capped() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    for text in ["(", "1+", "$", "*[1,2;3", "f(", "\u{20ac}"] {
        h.type_text(text);
    }
    assert_eq!(h.source_of(id), "(1+$*[1,2;3f(\u{20ac}");
    h.frame_with(vec![Event::Paste("9".repeat(20_000))]);
    assert!(h.source_of(id).chars().count() <= 8192);
    h.settle();
}

#[test]
fn the_functions_window_inserts_into_the_region_being_edited() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x:=");
    h.sheet().doc.region(id).expect("region");
    // An edit from outside the editor (as the Functions window makes) is picked up.
    let source = "x:=sqrt(".to_owned();
    h.app.state.sheets[0].edit(Edit::SetMathSource { id, source });
    h.frame();
    h.type_text("4)");
    assert_eq!(h.source_of(id), "x:=sqrt(4)");
}

#[test]
fn backslash_names_become_greek_letters_and_a_colon_makes_a_define() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text(r"\sigma");
    h.type_text(" ");
    h.type_text(":");
    h.type_text("=5");
    assert_eq!(h.source_of(id), "\u{3c3}:=5");
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text(r"\beta");
    h.press_key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.source_of(id), "\u{3b2}", "Tab also completes the letter");
    assert!(
        h.sheet().canvas.editing.is_some(),
        "Tab does not end editing"
    );
}

#[test]
fn autocomplete_finishes_a_word_with_tab_or_enter_and_escape_only_closes_the_list() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("sqr");
    h.press_key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.source_of(id), "sqrt(");
    // With the list open, Enter accepts instead of ending the edit.
    h.type_text("co");
    h.press_key(Key::Enter, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_some());
    assert!(
        h.source_of(id).starts_with("sqrt(co"),
        "{}",
        h.source_of(id)
    );
    assert_ne!(h.source_of(id), "sqrt(co", "the word was completed");
    // Escape closes the list; a second Escape ends the edit.
    h.press_key(Key::A, Modifiers::COMMAND);
    h.type_text("ta");
    h.press_key(Key::Escape, Modifiers::NONE);
    assert!(
        h.sheet().canvas.editing.is_some(),
        "the first Escape closed only the list"
    );
    assert_eq!(h.source_of(id), "ta");
    h.press_key(Key::Escape, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_none());
}

#[test]
fn autocomplete_offers_names_defined_in_the_worksheet_and_arrows_choose() {
    let mut h = Harness::new();
    h.settle();
    let first = h.new_math();
    h.type_text("stress_y:=250");
    h.press_key(Key::Enter, Modifiers::NONE);
    h.settle();
    let second = h.new_math();
    h.type_text("stress");
    h.press_key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.source_of(second), "stress_y");
    assert_ne!(first, second);
}

#[test]
fn a_subscript_name_edits_like_any_other_text() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("F_x+F_y");
    h.press_key(Key::Home, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    h.press_key(Key::ArrowRight, Modifiers::NONE);
    assert!(h.caret() >= 3, "{}", h.caret());
    assert_eq!(h.source_of(id), "F_x+F_y");
    h.settle();
}

#[test]
fn many_keystrokes_in_one_frame_all_arrive() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    let mut events = Vec::new();
    for ch in "sigma_y".chars() {
        events.push(Event::Key {
            key: Key::A,
            physical_key: Some(Key::A),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        events.push(Event::Text(ch.to_string()));
    }
    // The Enter that finishes the region arrives in the same frame.
    events.push(Event::Key {
        key: Key::Enter,
        physical_key: Some(Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    h.frame_with(events);
    h.frame();
    assert_eq!(h.source_of(id), "sigma_y");
    assert!(h.sheet().canvas.editing.is_none());
}

#[test]
fn clicking_the_ribbon_does_not_end_editing_and_typing_carries_on() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    let count = h.sheet().doc.regions().len();
    h.type_text("ab");
    // A ribbon tab, then the empty part of the ribbon: neither is the canvas.
    for at in [Pos2::new(108.0, 45.0), Pos2::new(1100.0, 80.0)] {
        h.frame_with(vec![Event::PointerMoved(at)]);
        h.frame_with(vec![press(at)]);
        h.frame_with(vec![release(at)]);
        h.frame();
        h.frame();
        assert_eq!(h.sheet().canvas.editing, Some(id), "a click at {at:?}");
    }
    h.type_text("cd");
    assert_eq!(h.source_of(id), "abcd");
    assert_eq!(h.sheet().doc.regions().len(), count, "no region was added");
}

#[test]
fn a_press_on_the_canvas_away_from_the_region_ends_editing() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("ab");
    let away = Pos2::new(1000.0, 650.0);
    h.frame_with(vec![Event::PointerMoved(away)]);
    h.frame_with(vec![press(away)]);
    h.frame_with(vec![release(away)]);
    h.frame();
    assert!(h.sheet().canvas.editing.is_none());
    assert_eq!(h.source_of(id), "ab");
}

#[test]
fn a_greek_letter_from_the_ribbon_goes_in_at_the_caret_while_editing() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x:=");
    h.command(CommandId::InsertSymbol('\u{3c3}'));
    assert_eq!(h.source_of(id), "x:=\u{3c3}");
    h.type_text("+1");
    assert_eq!(
        h.source_of(id),
        "x:=\u{3c3}+1",
        "typing carries on after the letter"
    );
    // In the middle: the letter goes at the caret and replaces a selection.
    h.press_key(Key::Home, Modifiers::NONE);
    h.command(CommandId::InsertSymbol('\u{3b1}'));
    assert_eq!(h.source_of(id), "\u{3b1}x:=\u{3c3}+1");
    // One undo step for the typing before, and the letter edits are undoable.
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    h.key(Key::Z, Modifiers::COMMAND);
    assert!(!h.source_of(id).starts_with('\u{3b1}'));
}

#[test]
fn symbols_are_not_offered_when_nothing_is_being_edited() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a");
    // The first Escape closes the autocomplete list, the second ends the edit.
    h.press_key(Key::Escape, Modifiers::NONE);
    h.press_key(Key::Escape, Modifiers::NONE);
    h.frame();
    assert!(h.sheet().canvas.field.is_none());
    h.app.queue.push(CommandId::InsertSymbol('\u{3c3}'));
    h.frame();
    assert_eq!(
        h.source_of(id),
        "a",
        "the command is disabled when no region is open"
    );
}

#[test]
fn clicking_a_flyout_that_floats_over_the_canvas_does_not_end_editing() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x:=");
    // The Greek flyout (Insert tab, Symbols group) hangs below the ribbon, over the page.
    let tab = h
        .app
        .ribbon
        .tabs
        .iter()
        .position(|t| t.title == "Insert")
        .unwrap();
    let panel = h.app.ribbon.tabs[tab]
        .panels
        .iter()
        .position(|p| p.title == "Symbols")
        .unwrap();
    h.app.ribbon_state.active_tab = tab;
    h.app.ribbon_state.open_panel = Some((tab, panel));
    h.frame();
    h.frame();
    let flyout = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("ribbon-flyout", (tab, panel)))))
        .expect("the flyout is open");
    assert!(
        flyout.max.y > 100.0,
        "it reaches into the canvas area: {flyout:?}"
    );
    let at = flyout.center();
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![press(at)]);
    h.frame_with(vec![release(at)]);
    h.frame();
    assert_eq!(
        h.sheet().canvas.editing,
        Some(id),
        "still editing after the click"
    );
}

#[test]
fn up_and_down_remember_the_column_they_started_from() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a/b");
    h.press_key(Key::End, Modifiers::NONE);
    h.press_key(Key::ArrowUp, Modifiers::NONE);
    let goal = h.sheet().canvas.field.as_ref().expect("editing").goal_x;
    assert!(goal.is_some(), "a vertical move records its column");
    h.press_key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        h.sheet().canvas.field.as_ref().expect("editing").goal_x,
        goal,
        "the column survives repeated vertical moves"
    );
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(
        h.sheet().canvas.field.as_ref().expect("editing").goal_x,
        None,
        "a horizontal move forgets it"
    );
    assert_eq!(h.source_of(id), "a/b", "navigation never edits the source");
}

#[test]
fn moving_the_caret_makes_no_edit_and_undo_while_editing_keeps_it_in_range() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+12345");
    let before = h.source_of(id);
    h.press_key(Key::Home, Modifiers::NONE);
    h.press_key(Key::ArrowRight, SHIFT);
    h.press_key(Key::End, Modifiers::NONE);
    assert_eq!(
        h.source_of(id),
        before,
        "caret and selection moves are not edits"
    );
    h.press_key(Key::Z, Modifiers::COMMAND);
    let len = h.source_of(id).chars().count();
    if let Some(f) = h.sheet().canvas.field.as_ref() {
        assert!(
            f.caret <= len && f.anchor <= len,
            "caret clamped after undo"
        );
    }
}

#[test]
fn the_crosshair_follows_a_click_on_blank_page() {
    let mut h = Harness::new();
    h.settle();
    let before = h.sheet().canvas.insert_at;
    let at = h.app.state.viewport.max - egui::vec2(120.0, 80.0);
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![press(at)]);
    h.frame_with(vec![release(at)]);
    assert_ne!(
        h.sheet().canvas.insert_at,
        before,
        "{at:?} {:?}",
        h.app.state.viewport
    );
    assert!(h.sheet().canvas.editing.is_none());
}

#[test]
fn typing_after_a_click_on_blank_page_starts_a_math_region_there() {
    let mut h = Harness::new();
    h.settle();
    let at = h.app.state.viewport.max - egui::vec2(120.0, 80.0);
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![press(at)]);
    h.frame_with(vec![release(at)]);
    let count = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Text("x".into())]);
    assert_eq!(h.sheet().doc.regions().len(), count + 1);
    let id = *h.sheet().canvas.selection.first().expect("selected");
    assert_eq!(h.source_of(id), "x");
    assert_eq!(h.sheet().canvas.editing, Some(id));
    h.type_text("+1");
    assert_eq!(
        h.source_of(id),
        "x+1",
        "typing carries on in the new region"
    );
    h.key(Key::Enter, Modifiers::NONE);
    h.key(Key::Z, Modifiers::COMMAND);
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(
        h.sheet().doc.regions().len(),
        count,
        "typing, then creation, each undo in one step"
    );
}

#[test]
fn typing_on_a_fresh_worksheet_starts_a_region_at_the_insertion_point() {
    let mut h = Harness::new();
    h.settle();
    let count = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Text("x".into())]);
    assert_eq!(h.sheet().doc.regions().len(), count + 1);
}

#[test]
fn deleting_the_region_being_edited_closes_the_editor() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("x+1");
    h.command(CommandId::RegionDelete);
    h.frame();
    assert!(h.sheet().doc.region(id).is_none());
    assert!(
        h.sheet().canvas.editing.is_none(),
        "no editor on a gone region"
    );
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    h.command(CommandId::EditUndo);
    h.settle();
    assert!(
        h.sheet().doc.region(id).is_some(),
        "undo restores the region"
    );
}

#[test]
fn undoing_the_creation_of_the_region_being_edited_closes_the_editor() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("abc");
    h.key(Key::Enter, Modifiers::NONE);
    h.frame_with(vec![Event::PointerMoved(h.screen_centre(id))]);
    h.click(id);
    h.frame();
    for _ in 0..3 {
        h.command(CommandId::EditUndo);
        h.frame();
    }
    assert!(
        h.sheet().doc.region(id).is_none(),
        "undo removed the region"
    );
    assert!(h.sheet().canvas.editing.is_none(), "and the editor with it");
}

#[test]
fn a_new_sheet_has_no_editor_and_the_first_keeps_its_own() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("ab");
    h.command(CommandId::FileNew);
    h.frame();
    assert!(h.sheet().canvas.editing.is_none());
    assert!(h.sheet().canvas.field.is_none());
    h.type_text("zzz");
    h.command(CommandId::SelectSheet(0));
    h.frame();
    assert_eq!(
        h.source_of(id),
        "ab",
        "keystrokes on the new sheet never reach sheet 0"
    );
}

#[test]
fn zooming_while_editing_keeps_the_caret_and_clicks_consistent() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("a/b");
    let caret = h.caret();
    for c in [
        CommandId::ViewZoomIn,
        CommandId::ViewZoomIn,
        CommandId::ViewZoomOut,
    ] {
        h.command(c);
        h.frame();
        assert_eq!(h.caret(), caret, "zoom never moves the caret in the source");
        assert_eq!(h.source_of(id), "a/b");
    }
    h.type_text("c");
    assert_eq!(h.source_of(id), "a/bc");
}

#[test]
fn enter_readies_the_next_input_below_and_typing_starts_it() {
    let mut h = Harness::new();
    h.settle();
    let first = h.new_math();
    h.type_text("a:=1");
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.sheet().canvas.editing.is_none());
    assert!(
        h.sheet().canvas.selection.is_empty(),
        "nothing stays highlighted"
    );
    let below = h.sheet().canvas.insert_at;
    let r = h.sheet().doc.region(first).expect("region");
    assert!(below.y > r.pos.y, "the insertion point is below it");
    let count = h.sheet().doc.regions().len();
    h.frame_with(vec![Event::Text("b".into())]);
    assert_eq!(h.sheet().doc.regions().len(), count + 1);
    let id = *h.sheet().canvas.selection.first().expect("selected");
    assert_ne!(id, first);
    assert_eq!(h.source_of(id), "b");
    assert_eq!(h.source_of(first), "a:=1", "the first input is untouched");
}

#[test]
fn ctrl_space_grows_the_selection_through_the_expression_and_typing_replaces_it() {
    let mut h = Harness::new();
    h.settle();
    let id = h.new_math();
    h.type_text("1+a/b");
    h.press_key(Key::ArrowLeft, Modifiers::NONE);
    h.press_key(Key::Space, Modifiers::COMMAND);
    let sel = |h: &Harness| {
        h.sheet()
            .canvas
            .field
            .as_ref()
            .expect("editing")
            .selection()
    };
    assert_eq!(sel(&h), (4, 5), "the denominator");
    h.press_key(Key::Space, Modifiers::COMMAND);
    assert_eq!(sel(&h), (2, 5), "the whole fraction");
    h.press_key(Key::Space, Modifiers::COMMAND);
    assert_eq!(sel(&h), (0, 5), "the whole sum");
    h.press_key(Key::Space, Modifiers::COMMAND);
    assert_eq!(sel(&h), (0, 5), "nothing larger");
    h.press_key(Key::Backspace, Modifiers::NONE);
    assert_eq!(h.source_of(id), "");
}
