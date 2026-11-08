//! Auto-save and recovery. See `docs/PERSISTENCE.md`, section 7.
//!
//! While a worksheet has unsaved changes, a timer writes them to a separate
//! recovery file: `<file>.recovery` beside a saved worksheet, or in the app's own
//! folder for one that was never saved. The real file changes only on an explicit
//! save. A save, or closing the worksheet, deletes the recovery file; if the
//! program stops any other way the file is still there, and the next start offers
//! to restore it.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use eframe::egui::Context;
use opencalc_core::persist::{self, EXTENSION};

use super::filing::Recovery;
use super::App;
use crate::sheet::OpenSheet;

/// Seconds between recovery writes while a worksheet is modified.
pub const EVERY: f64 = 30.0;

const SUFFIX: &str = "recovery";

/// Where the recovery file for a saved worksheet goes.
pub fn beside(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(SUFFIX);
    PathBuf::from(name)
}

/// The app's own folder for recovery files of worksheets that were never saved.
pub fn default_dir() -> Option<PathBuf> {
    let var = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        var("APPDATA")?
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library").join("Application Support")
    } else {
        var("XDG_STATE_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("state")))?
    };
    Some(base.join("OpenCalc").join("recovery"))
}

fn is_recovery(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == SUFFIX)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl App {
    /// Turn on auto-save, keeping the recovery files of unsaved worksheets in `dir`.
    pub fn enable_autosave(&mut self, dir: Option<PathBuf>) {
        self.autosave = true;
        self.recovery_dir = dir;
    }

    fn recovery_target(&self, sheet: &OpenSheet) -> Option<PathBuf> {
        match (&sheet.path, &sheet.adopted_recovery, &self.recovery_dir) {
            (Some(p), _, _) => Some(beside(p)),
            (None, Some(file), _) => Some(file.clone()),
            (None, None, Some(dir)) => Some(dir.join(format!(
                "untitled-{}-{}.{EXTENSION}.{SUFFIX}",
                std::process::id(),
                sheet.key
            ))),
            (None, None, None) => None,
        }
    }

    /// Once a frame: write due recovery files, delete those no longer needed.
    pub(super) fn sync_recovery(&mut self, ctx: &Context) {
        if !self.autosave {
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_autosave >= self.autosave_every {
            self.last_autosave = now;
            for i in 0..self.state.sheets.len() {
                self.write_recovery(i);
            }
        }
        // A recovery file is needed only while its worksheet is modified.
        let state = &self.state;
        let wanted = |key: u32| state.sheets.iter().any(|s| s.key == key && s.dirty());
        self.recovery_files.retain(|key, file| {
            let keep = wanted(*key);
            if !keep {
                let _ = std::fs::remove_file(&*file);
            }
            keep
        });
        if self.state.sheets.iter().any(|s| s.dirty()) {
            ctx.request_repaint_after(Duration::from_secs_f64(self.autosave_every));
        }
    }

    fn write_recovery(&mut self, index: usize) {
        let sheet = &self.state.sheets[index];
        if !sheet.dirty() || sheet.autosaved == Some(sheet.doc.revision()) {
            return;
        }
        let Some(target) = self.recovery_target(sheet) else {
            return;
        };
        if let Some(dir) = target.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // A failure is not worth interrupting the user for: the next tick tries again.
        let written = persist::save_bytes(&sheet.doc)
            .map_err(|e| e.to_string())
            .and_then(|bytes| persist::write_atomic(&target, &bytes).map_err(|e| e.to_string()));
        if written.is_ok() {
            let sheet = &mut self.state.sheets[index];
            sheet.autosaved = Some(sheet.doc.revision());
            self.recovery_files.insert(sheet.key, target);
        }
    }

    /// Queue questions for the recovery files left by a run that stopped without
    /// saving: those of unsaved worksheets, and those beside recent files.
    pub fn scan_recovery(&mut self) {
        let mut found: Vec<Recovery> = Vec::new();
        if let Some(dir) = &self.recovery_dir {
            let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| is_recovery(p))
                .collect();
            files.sort();
            found.extend(files.into_iter().map(|file| Recovery {
                file,
                original: None,
            }));
        }
        for original in self.state.recent.clone() {
            if let Some(r) = newer_recovery(&original) {
                found.push(r);
            }
        }
        for r in found {
            if !self.state.recoveries.contains(&r) {
                self.state.recoveries.push(r);
            }
        }
        self.state.next_recovery();
    }

    /// After opening `path`, ask about a recovery file that is newer than it.
    pub(super) fn offer_recovery(&mut self, path: &Path) {
        if let Some(r) = newer_recovery(path) {
            if !self.state.recoveries.contains(&r) {
                self.state.recoveries.push(r);
            }
            self.state.next_recovery();
        }
    }

    /// Put the changes in a recovery file back as a modified worksheet.
    pub(super) fn restore(&mut self, r: Recovery) {
        let loaded = persist::read_limited(&r.file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| persist::load_bytes(&bytes).map_err(|e| e.to_string()));
        let loaded = match loaded {
            Ok(l) => l,
            Err(reason) => {
                self.state.message(
                    "Could not restore",
                    format!(
                        "{}\n\n{reason}\n\nThe recovery file was kept.",
                        r.file.display()
                    ),
                );
                return;
            }
        };
        let mut sheet = OpenSheet::loaded(loaded.worksheet, r.original.clone());
        sheet.mark_unsaved();
        if r.original.is_none() {
            sheet.adopted_recovery = Some(r.file.clone());
        }
        self.recovery_files.insert(sheet.key, r.file.clone());
        // The restored changes replace the worksheet open from the same file.
        match r.original.as_deref().and_then(|p| self.state.sheet_for(p)) {
            Some(i) => {
                self.state.sheets[i] = sheet;
                self.state.active = Some(i);
            }
            None => self.state.show_opened(sheet),
        }
        if let Some(p) = &r.original {
            self.state.note_recent(p);
        }
        self.recalculate();
    }
}

/// The recovery file beside `original`, if it is newer than `original` itself (or
/// the original is gone).
fn newer_recovery(original: &Path) -> Option<Recovery> {
    let file = beside(original);
    let theirs = modified(&file)?;
    let newer = modified(original).is_none_or(|ours| theirs > ours);
    newer.then(|| Recovery {
        file,
        original: Some(original.to_owned()),
    })
}
