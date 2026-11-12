//! Check Worksheet: find what is wrong, list it, and step from problem to problem.
//!
//! The findings come from core's `check`, the same as `opencalc-cli check`, so the
//! window, the keys and the command line agree. They are listed in reading order.

use opencalc_core::check::{check, Finding};

use super::dispatch::Effect;
use super::state::AppState;
use crate::commands::CommandId;

/// What the Check window shows: the findings for one revision of one worksheet.
#[derive(Default)]
pub struct CheckState {
    pub open: bool,
    pub findings: Vec<Finding>,
    /// Which worksheet and revision `findings` were made for.
    for_sheet: Option<(u32, u64)>,
}

/// Reading order: top to bottom, then left to right.
fn order(a: &Finding, b: &Finding) -> std::cmp::Ordering {
    (a.at.1, a.at.0)
        .partial_cmp(&(b.at.1, b.at.0))
        .unwrap_or(std::cmp::Ordering::Equal)
}

impl AppState {
    /// Bring the findings up to date with the active worksheet (cheap when it has
    /// not changed since).
    pub fn refresh_findings(&mut self) {
        let Some(sheet) = self.active_sheet() else {
            self.check.findings.clear();
            self.check.for_sheet = None;
            return;
        };
        let stamp = (sheet.key, sheet.doc.revision());
        if self.check.for_sheet == Some(stamp) {
            return;
        }
        let mut findings = check(&sheet.doc);
        findings.sort_by(order);
        self.check.findings = findings;
        self.check.for_sheet = Some(stamp);
    }

    /// Select the region the finding is about and bring it into view.
    fn go_to_finding(&mut self, index: usize) {
        let (Some(f), Some(i)) = (self.check.findings.get(index), self.active) else {
            return;
        };
        let id = f.region;
        let viewport = self.viewport.size();
        let sheet = &mut self.sheets[i];
        sheet.canvas.selection = vec![id];
        sheet.canvas.editing = None;
        sheet.canvas.field = None;
        sheet.reveal(id, viewport);
    }

    /// The next (or previous) finding after the selected region, wrapping round.
    fn step_finding(&mut self, forward: bool) {
        self.refresh_findings();
        let findings = &self.check.findings;
        if findings.is_empty() {
            self.message("Check Worksheet", "No problems found.".to_owned());
            return;
        }
        let here = self.active_sheet().and_then(|s| {
            let id = *s.canvas.selection.first()?;
            let r = s.doc.region(id)?;
            Some((r.pos.y, r.pos.x))
        });
        let after = |f: &Finding| here.is_none_or(|h| (f.at.1, f.at.0) > h);
        let before = |f: &Finding| here.is_none_or(|h| (f.at.1, f.at.0) < h);
        let index = if forward {
            findings.iter().position(after).unwrap_or(0)
        } else {
            findings
                .iter()
                .rposition(before)
                .unwrap_or(findings.len() - 1)
        };
        self.go_to_finding(index);
    }
}

/// Run a Check command. `None` if `command` is not one of them.
pub fn dispatch_check(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    match command {
        CommandId::CalcCheck => {
            state.check.open = true;
            state.refresh_findings();
        }
        CommandId::CheckClose => state.check.open = false,
        CommandId::WarnNext => state.step_finding(true),
        CommandId::WarnPrev => state.step_finding(false),
        CommandId::GoToFinding(i) => state.go_to_finding(usize::from(i)),
        _ => return None,
    }
    Some(Vec::new())
}
