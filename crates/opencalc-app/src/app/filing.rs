//! The state side of saving, opening and closing: which question the user is
//! being asked, and how commands move through it. No disk or dialog work happens
//! here (that is `files.rs`); this only decides what is to be done.

use std::path::PathBuf;

use super::dispatch::{add_sheet, close_sheet, Effect};
use super::state::AppState;
use crate::commands::CommandId;

/// Most recently used files remembered.
pub const MAX_RECENT: usize = 10;

/// What to do once an unsaved worksheet has been dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    Close(usize),
    Quit,
}

/// A modal question or message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// The worksheet at this index has changes that are not saved.
    Unsaved {
        sheet: usize,
        then: Then,
    },
    Message {
        title: String,
        text: String,
    },
    /// Changes that were not saved when the program last stopped.
    Recover(Recovery),
    /// An export was asked for while the numbers on the page are out of date.
    StaleExport(crate::export::Kind),
}

/// A recovery file found at startup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
    pub file: PathBuf,
    /// The worksheet it belongs to, or `None` for one that was never saved.
    pub original: Option<PathBuf>,
}

impl AppState {
    /// Show the next recovery question if nothing else is in front.
    pub fn next_recovery(&mut self) {
        if self.dialog.is_none() && !self.recoveries.is_empty() {
            self.dialog = Some(Dialog::Recover(self.recoveries.remove(0)));
        }
    }

    pub fn message(&mut self, title: &str, text: String) {
        self.dialog = Some(Dialog::Message {
            title: title.to_owned(),
            text,
        });
    }

    /// Put `path` first in the recent list.
    pub fn note_recent(&mut self, path: &std::path::Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_owned());
        self.recent.truncate(MAX_RECENT);
    }

    /// The index of the worksheet that is open from `path`, if any.
    pub fn sheet_for(&self, path: &std::path::Path) -> Option<usize> {
        let same = |a: &std::path::Path, b: &std::path::Path| {
            a == b
                || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)),
                    (Ok(x), Ok(y)) if x == y)
        };
        self.sheets
            .iter()
            .position(|s| s.path.as_deref().is_some_and(|p| same(p, path)))
    }

    /// Add an opened worksheet, replacing a blank untouched one rather than
    /// leaving it open beside the file.
    pub fn show_opened(&mut self, sheet: crate::sheet::OpenSheet) {
        if let [only] = self.sheets.as_slice() {
            if only.is_pristine() {
                self.sheets.clear();
            }
        }
        add_sheet(self, sheet);
    }
}

/// Close worksheet `i`, asking first if it has unsaved changes.
pub fn request_close(state: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    let Some(sheet) = state.sheets.get(i) else {
        return;
    };
    if sheet.dirty() {
        state.active = Some(i);
        state.dialog = Some(Dialog::Unsaved {
            sheet: i,
            then: Then::Close(i),
        });
    } else {
        close_sheet(state, i);
        fx.push(Effect::Recalculate);
    }
}

/// Quit, asking about each modified worksheet in turn first.
pub fn request_quit(state: &mut AppState, fx: &mut Vec<Effect>) {
    match state.sheets.iter().position(|s| s.dirty()) {
        Some(i) => {
            state.active = Some(i);
            state.dialog = Some(Dialog::Unsaved {
                sheet: i,
                then: Then::Quit,
            });
            fx.push(Effect::Recalculate);
        }
        None => fx.push(Effect::Quit),
    }
}

/// Commands about files and the dialogs around them. `None`: not one of these.
pub fn dispatch_file(state: &mut AppState, command: CommandId) -> Option<Vec<Effect>> {
    use CommandId::*;
    let mut fx = Vec::new();
    match command {
        FileOpen => fx.push(Effect::ChooseOpen),
        OpenRecent(n) => {
            if let Some(path) = state.recent.get(usize::from(n)) {
                fx.push(Effect::Open(path.clone()));
            }
        }
        FileSave => {
            if let Some(sheet) = state.active {
                fx.push(Effect::Save {
                    sheet,
                    then: None,
                    ask: false,
                });
            }
        }
        FileSaveAs => {
            if let Some(sheet) = state.active {
                fx.push(Effect::Save {
                    sheet,
                    then: None,
                    ask: true,
                });
            }
        }
        FileClose => {
            if let Some(i) = state.active {
                request_close(state, i, &mut fx);
            }
        }
        CloseSheet(i) => request_close(state, i, &mut fx),
        AppQuit => request_quit(state, &mut fx),
        DialogSave => {
            if let Some(Dialog::Unsaved { sheet, then }) = state.dialog.take() {
                fx.push(Effect::Save {
                    sheet,
                    then: Some(then),
                    ask: false,
                });
            }
        }
        DialogDiscard => {
            if let Some(Dialog::Unsaved { sheet, then }) = state.dialog.take() {
                match then {
                    Then::Close(i) => {
                        close_sheet(state, i);
                        fx.push(Effect::Recalculate);
                    }
                    Then::Quit => {
                        if let Some(s) = state.sheets.get_mut(sheet) {
                            s.mark_clean();
                        }
                        request_quit(state, &mut fx);
                    }
                }
            }
        }
        DialogRestore => {
            if let Some(Dialog::Recover(r)) = state.dialog.take() {
                fx.push(Effect::Restore(r));
            }
        }
        DialogDropRecovery => {
            if let Some(Dialog::Recover(r)) = state.dialog.take() {
                fx.push(Effect::DeleteFile(r.file));
            }
        }
        DialogExportAnyway => {
            if let Some(Dialog::StaleExport(kind)) = state.dialog.take() {
                fx.push(Effect::ExportNow(kind));
            }
        }
        DialogRecalculate => {
            if let Some(Dialog::StaleExport(_)) = state.dialog.take() {
                fx.push(Effect::CalculateNow);
            }
        }
        DialogCancel | DialogOk => state.dialog = None,
        _ => return None,
    }
    Some(fx)
}

/// Where a file that is not open yet goes: the path to remember for it.
pub fn with_extension(mut path: PathBuf) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension(opencalc_core::persist::EXTENSION);
    }
    path
}
