//! Check Worksheet: the window, and stepping from problem to problem.

#![cfg(test)]

use opencalc_core::document::{Point, RegionId, RegionKind};

use super::Harness;
use crate::commands::CommandId;

/// The example worksheet is clean; add two problems below it, out of order.
fn with_problems() -> (Harness, RegionId, RegionId) {
    let mut h = Harness::new();
    h.settle();
    let sheet = &mut h.app.state.sheets[0];
    let late = sheet.add(RegionKind::math("q := 1 m + 2 s"), Point::new(40.0, 900.0));
    let early = sheet.add(RegionKind::math("zq = 5 m"), Point::new(40.0, 800.0));
    h.app.recalculate();
    h.settle();
    (h, early, late)
}

fn selected(h: &Harness) -> Vec<RegionId> {
    h.sheet().canvas.selection.clone()
}

#[test]
fn the_window_lists_errors_and_warnings_in_reading_order() {
    let (mut h, early, late) = with_problems();
    h.command(CommandId::CalcCheck);
    assert!(h.app.state.check.open);
    let found: Vec<RegionId> = h
        .app
        .state
        .check
        .findings
        .iter()
        .map(|f| f.region)
        .collect();
    assert_eq!(found, [early, late], "top to bottom");
    // Clicking an entry goes to its region.
    h.command(CommandId::GoToFinding(1));
    assert_eq!(selected(&h), [late]);
    h.command(CommandId::CheckClose);
    assert!(!h.app.state.check.open);
}

#[test]
fn next_and_previous_step_through_the_problems_and_wrap() {
    let (mut h, early, late) = with_problems();
    h.app.state.sheets[0].canvas.selection.clear();
    h.command(CommandId::WarnNext);
    assert_eq!(selected(&h), [early]);
    h.command(CommandId::WarnNext);
    assert_eq!(selected(&h), [late]);
    h.command(CommandId::WarnNext);
    assert_eq!(selected(&h), [early], "wraps round");
    h.command(CommandId::WarnPrev);
    assert_eq!(selected(&h), [late], "and back");
    h.command(CommandId::WarnPrev);
    assert_eq!(selected(&h), [early]);
}

#[test]
fn a_clean_worksheet_says_so_and_the_findings_follow_edits() {
    let mut h = Harness::new();
    h.settle();
    h.command(CommandId::WarnNext);
    assert!(
        h.app.state.dialog.is_none(),
        "a clean result is a note, not a dialog"
    );
    assert_eq!(
        h.app.state.notice.as_ref().map(|(t, _)| t.as_str()),
        Some("No problems found")
    );
    h.command(CommandId::CalcCheck);
    assert!(h.app.state.check.findings.is_empty());
    // Fixing a problem takes it off the list at once, with the window open.
    let id = h.app.state.sheets[0].add(RegionKind::math("zq = 5 m"), Point::new(40.0, 900.0));
    h.frame();
    assert_eq!(h.app.state.check.findings.len(), 1);
    h.app.state.sheets[0].edit(opencalc_core::edit::Edit::SetMathSource {
        id,
        source: "zq := 5 m".into(),
    });
    h.frame();
    assert!(h.app.state.check.findings.is_empty());
}

#[test]
fn the_check_commands_need_a_worksheet() {
    let mut h = Harness::new();
    h.settle();
    for c in [
        CommandId::CalcCheck,
        CommandId::WarnNext,
        CommandId::WarnPrev,
    ] {
        assert!(h.app.registry().allows(c), "{c:?}");
    }
    h.command(CommandId::TabHome);
    for c in [
        CommandId::CalcCheck,
        CommandId::WarnNext,
        CommandId::WarnPrev,
    ] {
        assert!(!h.app.registry().allows(c), "{c:?}");
    }
}

#[test]
fn the_status_bar_counts_what_the_check_window_lists() {
    use opencalc_core::check::{check, Severity};
    let mut h = Harness::new();
    h.settle();
    assert_eq!(h.app.state.problem_counts(), (0, 0));
    let sheet = &mut h.app.state.sheets[0];
    sheet.add(RegionKind::math("1 m + 2 s"), Point::new(40.0, 900.0));
    sheet.add(RegionKind::math("zq = 5 m"), Point::new(40.0, 950.0));
    // A blank region is not a problem.
    sheet.add(RegionKind::math("  "), Point::new(40.0, 1000.0));
    h.app.recalculate();
    h.settle();
    let found = check(&h.app.state.sheets[0].doc);
    let errors = found
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = found.len() - errors;
    assert!(errors >= 1 && warnings >= 1, "{found:?}");
    assert_eq!(h.app.state.problem_counts(), (errors, warnings));
}
