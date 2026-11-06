//! Application state: plain data, no egui context, no worker thread. Panels read
//! it (rule R4); only `dispatch` changes it, in response to a command.

use eframe::egui::{Pos2, Rect};
use opencalc_core::eval::Output;
use opencalc_core::registry::UnitSystem;

use crate::commands::{CommandContext, CommandId};
use crate::sheet::OpenSheet;
use crate::theme::{PaletteKey, Theme};

/// Where evaluation stands. The worker itself lives in `App`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalcStatus {
    /// How many recalculations have been requested. Tests watch it to prove that
    /// edits which cannot change results do not trigger one.
    pub revision: u64,
    /// The revision whose result is currently displayed.
    pub applied: u64,
    /// False once the worker has stopped accepting requests.
    pub worker_ok: bool,
}

pub struct AppState {
    pub theme: Theme,
    pub palette: PaletteKey,
    pub sheets: Vec<OpenSheet>,
    /// `None` shows the Home page.
    pub active: Option<usize>,
    /// Counts untitled worksheets, for naming them.
    pub untitled: u32,
    pub output: Output,
    pub calc: CalcStatus,
    pub show_grid: bool,
    /// The page in view (from 1) and the number of pages, from the last frame.
    pub page: (usize, usize),
    pub snap: bool,
    pub live_recalc: bool,
    /// The optional Functions window.
    pub show_functions: bool,
    pub show_props: bool,
    /// Bumped by "Reset layout" so dock panels forget their saved widths.
    pub layout_gen: u32,
    /// Window width the default dock widths are a share of: the initial window
    /// width, refreshed by "Reset layout".
    pub dock_basis: f32,
    /// Screen rectangle of the canvas on the last frame.
    pub viewport: Rect,
    pub about_open: bool,
}

impl AppState {
    pub fn new(palette: PaletteKey) -> AppState {
        AppState {
            theme: Theme::palette(palette),
            palette,
            sheets: vec![OpenSheet::new("Untitled")],
            active: Some(0),
            untitled: 1,
            output: Output::default(),
            calc: CalcStatus {
                revision: 0,
                applied: 0,
                worker_ok: true,
            },
            show_grid: false,
            page: (1, 1),
            snap: true,
            live_recalc: true,
            show_functions: false,
            show_props: false,
            layout_gen: 0,
            dock_basis: crate::INITIAL_WIDTH,
            viewport: Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(800.0, 600.0)),
            about_open: false,
        }
    }

    /// A state that opens the example worksheet (for tests that need its regions).
    #[cfg(test)]
    pub fn with_sample(palette: PaletteKey) -> AppState {
        let mut s = AppState::new(palette);
        s.sheets = vec![OpenSheet::sample()];
        s
    }

    pub fn active_sheet(&self) -> Option<&OpenSheet> {
        self.active.and_then(|i| self.sheets.get(i))
    }

    /// What decides which commands can run right now.
    pub fn command_context(&self) -> CommandContext {
        let sheet = self.active_sheet();
        CommandContext {
            has_sheet: sheet.is_some(),
            has_selection: sheet.is_some_and(|s| !s.canvas.selection.is_empty()),
            can_undo: sheet.is_some_and(OpenSheet::can_undo),
            can_redo: sheet.is_some_and(OpenSheet::can_redo),
        }
    }

    /// Toggle commands currently switched on, for the ribbon's active look.
    pub fn active_toggles(&self) -> Vec<CommandId> {
        [
            (self.show_grid, CommandId::ViewGrid),
            (self.snap, CommandId::ViewSnap),
            (self.live_recalc, CommandId::ViewLive),
            (self.show_functions, CommandId::ViewFunctions),
            (self.show_props, CommandId::ViewProperties),
        ]
        .into_iter()
        .filter_map(|(on, command)| on.then_some(command))
        .chain(self.unit_system_command())
        .collect()
    }

    /// The unit system of the active worksheet, as its command.
    pub fn unit_system_command(&self) -> Option<CommandId> {
        self.active_sheet().map(|s| match s.doc.unit_system() {
            UnitSystem::Si => CommandId::UnitsSi,
            UnitSystem::Uscs => CommandId::UnitsUscs,
            UnitSystem::Cgs => CommandId::UnitsCgs,
        })
    }

    pub fn status_text(&self) -> String {
        if !self.calc.worker_ok {
            "worker stopped".to_owned()
        } else if self.calc.applied == self.calc.revision {
            format!("OpenCalc {} \u{b7} up to date", opencalc_core::version())
        } else {
            "calculating\u{2026}".to_owned()
        }
    }

    pub fn zoom_percent(&self) -> f32 {
        self.active_sheet().map_or(1.0, |s| s.view.zoom) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_state_opens_one_blank_untitled_worksheet_and_is_not_calculating() {
        let s = AppState::new(PaletteKey::Neutral);
        assert_eq!(s.active, Some(0));
        assert_eq!(s.sheets.len(), 1);
        assert_eq!(s.sheets[0].title(), "Untitled");
        assert!(s.sheets[0].doc.regions().is_empty());
        assert_eq!(s.calc.revision, s.calc.applied);
        assert!(s.status_text().contains("up to date"));
    }

    #[test]
    fn status_text_reports_calculating_and_a_stopped_worker() {
        let mut s = AppState::new(PaletteKey::Neutral);
        s.calc.revision = 5;
        assert!(s.status_text().contains("calculating"));
        s.calc.worker_ok = false;
        assert_eq!(s.status_text(), "worker stopped");
    }

    #[test]
    fn the_command_context_follows_the_active_sheet() {
        let mut s = AppState::with_sample(PaletteKey::Neutral);
        let c = s.command_context();
        assert!(c.has_sheet && !c.has_selection && !c.can_undo && !c.can_redo);
        let first = s.sheets[0].doc.regions()[0].id;
        s.sheets[0].canvas.selection = vec![first];
        assert!(s.command_context().has_selection);
        s.active = None;
        let home = s.command_context();
        assert!(!home.has_sheet && !home.has_selection);
    }

    #[test]
    fn active_toggles_list_exactly_the_switched_on_ones() {
        let mut s = AppState::new(PaletteKey::Neutral);
        // The grid and both optional panels start off; snapping, live recalculation
        // and the unit system are on.
        assert_eq!(s.active_toggles().len(), 3);
        assert!(!s.active_toggles().contains(&CommandId::ViewGrid));
        s.show_grid = true;
        s.snap = false;
        s.show_functions = true;
        let on = s.active_toggles();
        assert!(!on.contains(&CommandId::ViewSnap) && !on.contains(&CommandId::ViewProperties));
        assert!(on.contains(&CommandId::ViewFunctions));
        assert!(on.contains(&CommandId::ViewGrid) && on.contains(&CommandId::ViewLive));
    }

    #[test]
    fn zoom_is_one_hundred_percent_on_the_home_page() {
        let mut s = AppState::new(PaletteKey::Neutral);
        assert_eq!(s.zoom_percent(), 100.0);
        s.sheets[0].view.zoom = 0.5;
        assert_eq!(s.zoom_percent(), 50.0);
        s.active = None;
        assert_eq!(s.zoom_percent(), 100.0);
    }
}
