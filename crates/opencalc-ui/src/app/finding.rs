//! Find and replace: the state behind the Find window, and what its commands do.
//!
//! The window only reads [`FindState`] and asks for changes ([`FindCmd`] for what
//! is typed, commands for the buttons); nothing is changed from a panel (rule R4).
//! Matches come from core (`opencalc_core::find`) and are recomputed whenever the
//! worksheet, the query or the options change.

use std::collections::HashSet;

use opencalc_core::document::RegionId;
use opencalc_core::find::{find_all, replace_edit, Match, Options};

use super::dispatch::{apply_canvas, Effect};
use super::state::AppState;
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;

#[derive(Default)]
pub struct FindState {
    pub open: bool,
    /// The replace row is showing.
    pub replace: bool,
    pub query: String,
    pub replacement: String,
    pub options: Options,
    /// Take keyboard focus in the query box on the next frame.
    pub focus: bool,
    pub matches: Vec<Match>,
    /// Which match is the current one.
    pub current: usize,
    /// What `matches` was computed from: sheet, document revision, query, options.
    key: Option<(usize, u64, String, Options)>,
}

/// What the window changes by typing or ticking a box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindCmd {
    Query(String),
    Replacement(String),
    MatchCase(bool),
    WholeWord(bool),
    /// The query box has taken the focus that was asked for.
    FocusTaken,
}

/// Which regions hold matches, for the canvas to highlight.
#[derive(Default)]
pub struct Found {
    pub regions: HashSet<RegionId>,
    pub current: Option<RegionId>,
}

impl FindState {
    pub fn current_match(&self) -> Option<&Match> {
        self.matches.get(self.current)
    }
}

impl AppState {
    /// What the canvas highlights: nothing while the window is closed.
    pub fn found(&self) -> Found {
        if !self.find.open {
            return Found::default();
        }
        Found {
            regions: self.find.matches.iter().map(|m| m.region).collect(),
            current: self.find.current_match().map(|m| m.region),
        }
    }

    /// Recompute the matches if the worksheet, query or options changed.
    pub fn refresh_find(&mut self) {
        let Some(i) = self.active else {
            self.find.matches.clear();
            self.find.key = None;
            return;
        };
        let doc = &self.sheets[i].doc;
        let key = (
            i,
            doc.revision(),
            self.find.query.clone(),
            self.find.options,
        );
        if self.find.key.as_ref() == Some(&key) {
            return;
        }
        self.find.matches = find_all(doc, &self.find.query, self.find.options);
        self.find.current = self
            .find
            .current
            .min(self.find.matches.len().saturating_sub(1));
        self.find.key = Some(key);
    }

    pub fn apply_find(&mut self, cmd: FindCmd) {
        let find = &mut self.find;
        match cmd {
            FindCmd::Query(q) => {
                find.query = q;
                find.current = 0;
                self.refresh_find();
                self.go_to_match();
            }
            FindCmd::Replacement(r) => find.replacement = r,
            FindCmd::MatchCase(on) => {
                find.options.match_case = on;
                find.current = 0;
            }
            FindCmd::WholeWord(on) => {
                find.options.whole_word = on;
                find.current = 0;
            }
            FindCmd::FocusTaken => find.focus = false,
        }
    }

    /// Select the current match's region and bring it into view.
    pub fn go_to_match(&mut self) {
        let (Some(m), Some(i)) = (self.find.current_match().cloned(), self.active) else {
            return;
        };
        let viewport = self.viewport.size();
        let sheet = &mut self.sheets[i];
        sheet.canvas.selection = vec![m.region];
        sheet.canvas.editing = None;
        sheet.canvas.field = None;
        sheet.reveal(m.region, viewport);
    }

    fn step_match(&mut self, forward: bool) {
        self.refresh_find();
        let n = self.find.matches.len();
        if n == 0 {
            return;
        }
        self.find.current = if forward {
            (self.find.current + 1) % n
        } else {
            (self.find.current + n - 1) % n
        };
        self.go_to_match();
    }

    /// Replace the current match, or every match.
    fn replace(&mut self, all: bool) -> Vec<Effect> {
        self.refresh_find();
        let Some(i) = self.active else {
            return Vec::new();
        };
        let before = self.find.matches.len();
        let chosen: &[Match] = if all {
            &self.find.matches
        } else {
            match self.find.matches.get(self.find.current..=self.find.current) {
                Some(one) => one,
                None => return Vec::new(),
            }
        };
        let Some(edit) = replace_edit(&self.sheets[i].doc, chosen, &self.find.replacement) else {
            return Vec::new();
        };
        let cmds = vec![CanvasCmd::Seal, CanvasCmd::Edit(edit), CanvasCmd::Seal];
        let effects = apply_canvas(self, i, cmds);
        self.refresh_find();
        if !all && self.find.matches.len() >= before {
            // The replacement still matches (or nothing shrank): move past it.
            self.find.current += 1;
        }
        self.find.current = self
            .find
            .current
            .min(self.find.matches.len().saturating_sub(1));
        self.go_to_match();
        effects
    }
}

/// Commands of the Find window. `None`: not one of these.
pub fn dispatch_find(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    use CommandId::*;
    let mut fx = Vec::new();
    match command {
        EditFind | EditReplace => {
            state.find.open = true;
            state.find.replace = command == EditReplace;
            state.find.focus = true;
            state.refresh_find();
        }
        FindClose => state.find.open = false,
        FindNext | FindPrev => {
            if !state.find.open {
                state.find.open = true;
            }
            state.step_match(command == FindNext);
        }
        FindReplaceOne => fx = state.replace(false),
        FindReplaceAll => fx = state.replace(true),
        _ => return None,
    }
    Some(fx)
}
