//! Application state: plain data, no egui context, no worker thread. Panels read
//! it (rule R4); only `dispatch` changes it, in response to a command.

use std::path::PathBuf;

use eframe::egui::{Pos2, Rect};
use opencalc_core::eval::Output;
use opencalc_core::registry::UnitSystem;

use super::filing::Dialog;
use crate::commands::{CommandContext, CommandId};
use crate::settings::Settings;
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
    /// The worksheet (by `OpenSheet::key`) whose results are on screen.
    pub shown_for: Option<u32>,
}

pub struct AppState {
    pub theme: Theme,
    pub palette: PaletteKey,
    /// What the Options window sets, kept between sessions.
    pub settings: Settings,
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
    /// Automatic calculation. Off is manual mode: nothing is calculated until the
    /// Calculate command. A session setting, never saved in a file.
    pub live_recalc: bool,
    /// The optional Functions window.
    pub show_functions: bool,
    /// Bumped by "Reset layout" so dock panels forget their saved widths.
    pub layout_gen: u32,
    /// Window width the default dock widths are a share of: the initial window
    /// width, refreshed by "Reset layout".
    pub dock_basis: f32,
    /// Screen rectangle of the canvas on the last frame.
    pub viewport: Rect,
    /// The File menu behind the app button.
    pub app_menu_open: bool,
    pub about_open: bool,
    /// The Page setup window.
    pub page_setup_open: bool,
    /// A text box (not the math editor) held the keyboard last frame.
    pub text_box_focus: bool,
    /// A short note for the status bar ("Saved ..."), and when it was made.
    pub notice: Option<(String, std::time::Instant)>,
    /// Draft View (one continuous sheet) instead of Page View.
    pub draft: bool,
    /// The Options window (settings such as the theme).
    pub options_open: bool,
    /// Whether `.ocalc` can be registered here, and whether it is (read when Options opens).
    pub file_type: (bool, bool),
    /// The Units window (custom units).
    pub units_open: bool,
    /// The region whose Label window is open.
    pub label_for: Option<opencalc_core::document::RegionId>,
    /// The Labels window: the labelled regions, to go to.
    pub labels_open: bool,
    /// The Keyboard shortcuts window.
    pub shortcuts_open: bool,
    /// The Insert Matrix window.
    pub matrix_open: bool,
    /// The Check Worksheet window and the problems it lists.
    pub check: super::checking::CheckState,
    /// The Find and Replace window.
    pub find: super::finding::FindState,
    /// The question or message in front of everything, if any.
    pub dialog: Option<Dialog>,
    /// Recently opened or saved files, newest first.
    pub recent: Vec<PathBuf>,
    /// Files pinned to the top of the Home page. They stay on the recent list
    /// however long it grows.
    pub pinned: Vec<PathBuf>,
    /// Pictures of the recent files, drawn in the background.
    pub thumbs: super::thumbs::Thumbs,
    /// The regions last copied inside the app.
    pub clip: Option<super::clipboard::Clip>,
    /// Recovery files found and not yet asked about.
    pub recoveries: Vec<super::filing::Recovery>,
}

impl AppState {
    pub fn new(palette: PaletteKey) -> AppState {
        AppState {
            theme: Theme::palette(palette),
            palette,
            settings: Settings::default(),
            sheets: vec![OpenSheet::new("Untitled")],
            active: Some(0),
            untitled: 1,
            output: Output::default(),
            calc: CalcStatus {
                revision: 0,
                applied: 0,
                worker_ok: true,
                shown_for: None,
            },
            show_grid: true,
            page: (1, 1),
            snap: true,
            live_recalc: true,
            show_functions: false,
            layout_gen: 0,
            dock_basis: crate::INITIAL_WIDTH,
            viewport: Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(800.0, 600.0)),
            app_menu_open: false,
            about_open: false,
            page_setup_open: false,
            text_box_focus: false,
            notice: None,
            draft: false,
            options_open: false,
            file_type: (false, false),
            units_open: false,
            label_for: None,
            labels_open: false,
            shortcuts_open: false,
            matrix_open: false,
            check: Default::default(),
            find: Default::default(),
            dialog: None,
            recent: Vec::new(),
            pinned: Vec::new(),
            thumbs: Default::default(),
            clip: None,
            recoveries: Vec::new(),
        }
    }

    /// A state that opens the example worksheet (for tests that need its regions).
    #[cfg(test)]
    pub fn with_sample(palette: PaletteKey) -> AppState {
        let mut s = AppState::new(palette);
        s.sheets = vec![OpenSheet::sample()];
        s
    }

    /// A new, empty worksheet with the unit system and page from the settings.
    /// Not modified, and nothing to undo: these are where it starts.
    pub fn blank_sheet(&self, title: &str) -> OpenSheet {
        let s = &self.settings;
        let mut doc = opencalc_core::document::Worksheet::new(title);
        // A setting the worksheet already has is not an edit; the rest cannot fail.
        let _ = doc.apply(opencalc_core::edit::Edit::SetUnitSystem(s.units));
        let _ = doc.apply(opencalc_core::edit::Edit::SetPageSetup(s.page));
        let mut sheet = OpenSheet::from_doc(doc);
        sheet.set_grid(s.grid);
        sheet
    }

    /// Start with no worksheet open, so the program shows the Home page (the
    /// opening blank worksheet is only there for tests and for `AppState::new`).
    pub fn open_on_home(&mut self) {
        self.sheets.clear();
        self.active = None;
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
            selects_math: !super::formatting::selected_formats(self).is_empty(),
            selects_several: sheet.is_some_and(|s| s.canvas.selection.len() > 1),
            selects_text: !super::formatting::selected_styles(self).is_empty(),
            can_undo: sheet.is_some_and(OpenSheet::can_undo),
            can_redo: sheet.is_some_and(OpenSheet::can_redo),
            editing_math: sheet.is_some_and(|s| s.canvas.field.is_some()),
        }
    }

    /// Toggle commands currently switched on, for the ribbon's active look.
    pub fn active_toggles(&self) -> Vec<CommandId> {
        [
            (self.show_grid, CommandId::ViewGrid),
            (self.snap, CommandId::ViewSnap),
            (self.live_recalc, CommandId::CalcAuto),
            (self.show_functions, CommandId::ViewFunctions),
        ]
        .into_iter()
        .filter_map(|(on, command)| on.then_some(command))
        .chain(self.unit_system_command())
        .chain(self.notation_commands())
        .chain(Some(if self.draft {
            CommandId::ViewDraftView
        } else {
            CommandId::ViewPageView
        }))
        .chain(Some(match self.palette {
            PaletteKey::Neutral => CommandId::ThemeNeutral,
            PaletteKey::Slate => CommandId::ThemeSlate,
        }))
        .collect()
    }

    /// The notation commands switched on for the selected results.
    fn notation_commands(&self) -> Vec<CommandId> {
        use opencalc_core::document::Notation;
        [
            (Notation::Scientific, CommandId::FormatScientific),
            (Notation::Engineering, CommandId::FormatEngineering),
        ]
        .into_iter()
        .filter(|(n, _)| super::formatting::all_in(self, *n))
        .map(|(_, c)| c)
        .chain(super::formatting::style_commands(self))
        .chain(super::arranging::all_locked(self).then_some(CommandId::RegionLock))
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

    /// Manual mode, and the active worksheet has changed since its results were
    /// asked for, so the numbers shown are out of date.
    pub fn results_stale(&self) -> bool {
        !self.live_recalc
            && self
                .active_sheet()
                .is_some_and(|s| s.calculated != Some(s.doc.eval_revision()))
    }

    /// Whether an edit's request to recalculate is to be held back: manual mode,
    /// out of date, and the results on screen are this worksheet's own. (When they
    /// are another worksheet's, they are replaced; they are no use here.)
    pub fn holds_back_recalc(&self) -> bool {
        self.results_stale()
            && self
                .active_sheet()
                .is_some_and(|s| self.calc.shown_for == Some(s.key))
    }

    /// Whether to tell the user that the numbers on the page are out of date: manual
    /// mode, edited since the last calculation, and not already being calculated.
    pub fn stale_notice(&self) -> bool {
        self.results_stale() && self.calc.worker_ok && self.calc.applied == self.calc.revision
    }

    /// Whether the Tab key steps through the regions rather than through the
    /// controls: a region is selected, nothing is being typed into, and no window,
    /// menu or dialog is open to want Tab for itself. With nothing selected Tab
    /// still reaches the ribbon, so the keyboard can get there (Escape lets go of
    /// the selection).
    pub fn tab_steps_regions(&self, typing: bool) -> bool {
        let Some(sheet) = self.active_sheet() else {
            return false;
        };
        let c = &sheet.canvas;
        !typing
            && !self.text_box_focus
            && !c.selection.is_empty()
            && c.editing.is_none()
            && c.unit_edit.is_none()
            && self.dialog.is_none()
            && !(self.app_menu_open
                || self.about_open
                || self.page_setup_open
                || self.options_open
                || self.units_open
                || self.labels_open
                || self.shortcuts_open
                || self.matrix_open
                || self.check.open
                || self.find.open
                || self.label_for.is_some())
    }

    /// Say something short on the status bar, where it fades after a few seconds.
    /// For things that went well; a problem still gets a dialog.
    pub fn notify(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), std::time::Instant::now()));
    }

    /// How many errors and warnings the active worksheet's results hold, as of the
    /// last calculation (none while the results on show belong to another sheet).
    pub fn problem_counts(&self) -> (usize, usize) {
        use opencalc_core::document::RegionKind;
        let Some(sheet) = self.active_sheet() else {
            return (0, 0);
        };
        if self.calc.shown_for != Some(sheet.key) {
            return (0, 0);
        }
        let out = &self.output;
        let errors = sheet
            .doc
            .regions()
            .iter()
            .filter(|r| match &r.kind {
                RegionKind::Math(m) => {
                    !m.source().trim().is_empty() && matches!(out.values.get(&r.id), Some(Err(_)))
                }
                RegionKind::Plot(_) => matches!(out.plots.get(&r.id), Some(Err(_))),
                _ => false,
            })
            .count();
        (errors, out.warnings.len())
    }

    /// The calculation indicator on the status bar.
    pub fn calc_label(&self) -> &'static str {
        if !self.calc.worker_ok {
            "Calculation stopped"
        } else if self.calc.applied != self.calc.revision {
            "Calculating\u{2026}"
        } else if self.live_recalc {
            "Calculated"
        } else if self.results_stale() {
            "Manual: results out of date (Ctrl+F9 to calculate)"
        } else {
            "Manual"
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
        assert_eq!(s.calc_label(), "Calculated");
    }

    #[test]
    fn the_calculation_indicator_follows_the_worker_and_the_mode() {
        let mut s = AppState::new(PaletteKey::Neutral);
        assert_eq!(s.calc_label(), "Calculated");
        s.live_recalc = false;
        let rev = s.sheets[0].doc.eval_revision();
        s.sheets[0].calculated = Some(rev);
        assert_eq!(s.calc_label(), "Manual");
        s.sheets[0].calculated = None;
        assert!(s.calc_label().starts_with("Manual: results out of date"));
        s.sheets[0].calculated = Some(rev);
        s.calc.revision = 3;
        assert!(s.calc_label().starts_with("Calculating"));
        s.calc.worker_ok = false;
        assert_eq!(s.calc_label(), "Calculation stopped");
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
        // Both optional panels start off; the grid, snapping, automatic
        // calculation, the unit system and the theme choice are on.
        assert_eq!(s.active_toggles().len(), 6);
        assert!(s.active_toggles().contains(&CommandId::ViewGrid));
        s.snap = false;
        s.show_functions = true;
        let on = s.active_toggles();
        assert!(!on.contains(&CommandId::ViewSnap));
        assert!(on.contains(&CommandId::ViewFunctions));
        assert!(on.contains(&CommandId::ViewGrid) && on.contains(&CommandId::CalcAuto));
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
