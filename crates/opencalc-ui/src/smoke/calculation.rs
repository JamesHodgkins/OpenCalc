//! Manual calculation mode, and programs and solve blocks typed in the editor.

#![cfg(test)]

use super::*;

#[test]
fn manual_calculation_holds_every_edit_back_until_calculate_is_asked_for() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::CalcAuto); // off: manual
    h.frame();
    assert_eq!(h.app.state.calc_label(), "Manual");
    let before = h.app.state.calc.revision;

    // An edit that is not typing (inserting a region) is held back too.
    h.command(CommandId::InsertMath);
    h.frame();
    assert_eq!(h.app.state.calc.revision, before, "nothing was calculated");
    assert!(h.app.state.results_stale());
    assert!(h
        .app
        .state
        .calc_label()
        .starts_with("Manual: results out of date"));

    // Calculate brings the numbers up to date, and the label goes back.
    h.command(CommandId::CalcAll);
    h.frame();
    assert_eq!(h.app.state.calc.revision, before + 1);
    assert!(!h.app.state.results_stale());
    let end = Instant::now() + Duration::from_secs(10);
    while h.app.state.calc.applied != h.app.state.calc.revision && Instant::now() < end {
        h.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(h.app.state.calc_label(), "Manual");

    // Another held edit, then turning automatic calculation back on catches up.
    h.command(CommandId::InsertMath);
    h.frame();
    assert!(h.app.state.results_stale());
    h.command(CommandId::CalcAuto);
    h.frame();
    assert_eq!(h.app.state.calc.revision, before + 2);
    assert!(!h.app.state.results_stale());
}

#[test]
fn manual_mode_never_leaves_another_sheets_results_on_screen() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::CalcAuto);
    h.command(CommandId::InsertMath); // sheet 0 is now out of date
    h.frame();
    h.command(CommandId::FileNew); // a new sheet is calculated, not held back
    h.frame();
    assert!(!h.app.state.results_stale());
    h.command(CommandId::SelectSheet(0));
    h.frame();
    // Sheet 0's old numbers are gone with the other sheet's output; it is
    // calculated rather than shown with the wrong results.
    assert!(!h.app.state.results_stale());
}

#[test]
fn a_program_is_typed_with_spaces_lines_and_indentation_and_a_blank_line_ends_it() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::InsertMath);
    h.frame();
    let id = *h.sheet().canvas.selection.first().expect("selected");
    let type_text = |h: &mut Harness, text: &str| {
        for ch in text.chars() {
            h.frame_with(vec![Event::Text(ch.to_string())]);
        }
    };
    type_text(&mut h, "f(n):=program");
    h.key(Key::Enter, Modifiers::NONE);
    type_text(&mut h, "s<-0");
    h.key(Key::Enter, Modifiers::NONE);
    type_text(&mut h, "for i in 1..n");
    h.key(Key::Enter, Modifiers::NONE);
    type_text(&mut h, "s<-s+i");
    h.key(Key::Enter, Modifiers::NONE);
    h.key(Key::Tab, Modifiers::SHIFT);
    type_text(&mut h, "s");
    h.key(Key::Enter, Modifiers::NONE);
    // Enter on the blank line after the last one ends the program.
    h.key(Key::Enter, Modifiers::NONE);
    let source = match &h.sheet().doc.region(id).expect("region").kind {
        RegionKind::Math(m) => m.source().to_owned(),
        _ => panic!("math region"),
    };
    assert_eq!(
        source,
        "f(n):=program\n    s\u{2190}0\n    for i in 1..n\n        s\u{2190}s+i\n    s"
    );
    assert!(
        !matches!(
            opencalc_core::parser::parse(&source),
            opencalc_core::expr::Expr::Error(_)
        ),
        "what was typed is a program that parses"
    );
    assert!(
        h.app.state.sheets[0].canvas.editing.is_none(),
        "editing ended"
    );
}

#[test]
fn a_given_block_is_typed_like_a_program_and_solves() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::InsertMath);
    h.frame();
    let id = *h.sheet().canvas.selection.first().expect("selected");
    let type_text = |h: &mut Harness, text: &str| {
        for ch in text.chars() {
            h.frame_with(vec![Event::Text(ch.to_string())]);
        }
    };
    type_text(&mut h, "r:=given");
    h.key(Key::Enter, Modifiers::NONE);
    type_text(&mut h, "x+y==10");
    h.key(Key::Enter, Modifiers::NONE);
    type_text(&mut h, "x-y==2");
    h.key(Key::Enter, Modifiers::NONE);
    // Back out to the indent of the line `given` is on, for the closing line.
    h.key(Key::Tab, Modifiers::SHIFT);
    type_text(&mut h, "find(x,y)");
    h.key(Key::Enter, Modifiers::NONE);
    h.key(Key::Enter, Modifiers::NONE);
    let source = match &h.sheet().doc.region(id).expect("region").kind {
        RegionKind::Math(m) => m.source().to_owned(),
        _ => panic!("math region"),
    };
    assert_eq!(source, "r:=given\n    x+y==10\n    x-y==2\nfind(x,y)");
    assert!(
        !matches!(
            opencalc_core::parser::parse(&source),
            opencalc_core::expr::Expr::Error(_)
        ),
        "what was typed parses"
    );
}
