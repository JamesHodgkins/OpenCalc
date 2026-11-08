//! The disk side of saving and opening: asking for a path, reading, writing.
//!
//! The questions to the user go through [`Files`] so tests can answer them
//! without a native dialog. Everything here reports failure as a message in front
//! of the user and leaves the worksheet as it was (principle 1 in
//! `docs/PERSISTENCE.md`).

use std::path::{Path, PathBuf};

use opencalc_core::edit::Edit;
use opencalc_core::persist::{self, EXTENSION};

use super::filing::{with_extension, Then};
use super::App;
use crate::commands::CommandId;
use crate::sheet::OpenSheet;

/// Asks the user for a file path.
pub trait Files {
    fn pick_open(&mut self) -> Option<PathBuf>;
    /// `name` is the suggested file name.
    fn pick_save(&mut self, name: &str, folder: Option<&Path>) -> Option<PathBuf>;
    /// An image file (PNG) to put in a picture region.
    fn pick_image_open(&mut self, folder: Option<&Path>) -> Option<PathBuf>;
    /// A CSV or text file to read numbers from.
    fn pick_data_open(&mut self, folder: Option<&Path>) -> Option<PathBuf>;
    /// Where to write a CSV file; `name` is suggested.
    fn pick_data_save(&mut self, name: &str, folder: Option<&Path>) -> Option<PathBuf>;
    /// Where to export to: `name` is suggested, `kind` decides the file type.
    fn pick_export(
        &mut self,
        name: &str,
        kind: crate::export::Kind,
        folder: Option<&Path>,
    ) -> Option<PathBuf>;
}

/// The operating system's own dialogs.
pub struct NativeFiles;

fn filter(dialog: rfd::FileDialog) -> rfd::FileDialog {
    dialog.add_filter("OpenCalc worksheet", &[EXTENSION])
}

impl Files for NativeFiles {
    fn pick_open(&mut self) -> Option<PathBuf> {
        filter(rfd::FileDialog::new())
            .set_title("Open worksheet")
            .pick_file()
    }

    fn pick_image_open(&mut self, folder: Option<&Path>) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .set_title("Choose image");
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        dialog.pick_file()
    }

    fn pick_data_open(&mut self, folder: Option<&Path>) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("CSV or text", &["csv", "txt", "tsv"])
            .set_title("Import numbers");
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        dialog.pick_file()
    }

    fn pick_data_save(&mut self, name: &str, folder: Option<&Path>) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_title("Export table")
            .set_file_name(name);
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        dialog.save_file()
    }

    fn pick_save(&mut self, name: &str, folder: Option<&Path>) -> Option<PathBuf> {
        let mut dialog = filter(rfd::FileDialog::new())
            .set_title("Save worksheet")
            .set_file_name(name);
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        dialog.save_file()
    }

    fn pick_export(
        &mut self,
        name: &str,
        kind: crate::export::Kind,
        folder: Option<&Path>,
    ) -> Option<PathBuf> {
        let (label, title) = match kind {
            crate::export::Kind::Pdf => ("PDF document", "Export as PDF"),
            crate::export::Kind::Png => ("PNG image", "Export as PNG"),
        };
        let mut dialog = rfd::FileDialog::new()
            .add_filter(label, &[kind.extension()])
            .set_title(title)
            .set_file_name(name);
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        dialog.save_file()
    }
}

impl App {
    /// Open the worksheet at `path`, or switch to it if it is already open.
    pub fn open_path(&mut self, path: &Path) {
        if let Some(i) = self.state.sheet_for(path) {
            self.queue.push(CommandId::SelectSheet(i));
            return;
        }
        let loaded = persist::read_limited(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| persist::load_bytes(&bytes).map_err(|e| e.to_string()));
        match loaded {
            Ok(loaded) => {
                let sheet = OpenSheet::loaded(loaded.worksheet, Some(path.to_owned()));
                self.state.show_opened(sheet);
                self.state.note_recent(path);
                self.recalculate();
                self.offer_recovery(path);
                if !loaded.repairs.is_empty() {
                    let list: Vec<String> = loaded.repairs.iter().map(|r| r.to_string()).collect();
                    self.state.message(
                        "Opened with repairs",
                        format!(
                            "{} had {} problem(s), which were repaired:\n\n{}\n\nThe file on disk is unchanged until you save.",
                            path.display(),
                            list.len(),
                            list.join("\n")
                        ),
                    );
                }
            }
            Err(reason) => {
                self.state.recent.retain(|p| p != path);
                self.state
                    .message("Could not open", format!("{}\n\n{reason}", path.display()));
            }
        }
    }

    pub(super) fn choose_open(&mut self) {
        if let Some(path) = self.files.pick_open() {
            self.open_path(&path);
        }
    }

    /// Save worksheet `index`. Asks for a path when it has none, or when `ask`.
    pub(super) fn save_sheet(&mut self, index: usize, then: Option<Then>, ask: bool) {
        let Some(sheet) = self.state.sheets.get(index) else {
            return;
        };
        let path = match (&sheet.path, ask) {
            (Some(p), false) => p.clone(),
            _ => {
                let name = format!("{}.{EXTENSION}", sheet.title());
                let folder = sheet.path.as_deref().and_then(Path::parent);
                match self.files.pick_save(&name, folder) {
                    Some(p) => with_extension(p),
                    None => return, // cancelled: nothing changes, and nothing waits on it
                }
            }
        };
        if self.write_sheet(index, &path) {
            match then {
                Some(Then::Close(i)) => self.queue.push(CommandId::CloseSheet(i)),
                Some(Then::Quit) => self.queue.push(CommandId::AppQuit),
                None => {}
            }
        }
    }

    /// Write worksheet `index` to `path`. A save to a new path names the worksheet
    /// after the file. Returns whether the file was written.
    fn write_sheet(&mut self, index: usize, path: &Path) -> bool {
        let sheet = &mut self.state.sheets[index];
        let new_path = sheet.path.as_deref() != Some(path);
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        let rename = stem.filter(|s| new_path && !s.is_empty() && s != sheet.doc.title());
        // Build the file from a copy, so a failure leaves the worksheet untouched.
        let mut copy = sheet.doc.clone();
        if let Some(title) = &rename {
            let _ = copy.apply(Edit::SetTitle(title.clone()));
        }
        let written = persist::save_bytes(&copy)
            .map_err(|e| e.to_string())
            .and_then(|bytes| persist::write_atomic(path, &bytes).map_err(|e| e.to_string()));
        match written {
            Ok(()) => {
                if let Some(title) = rename {
                    sheet.seal();
                    sheet.edit(Edit::SetTitle(title));
                    sheet.seal();
                }
                sheet.mark_saved(path);
                self.state.note_recent(path);
                true
            }
            Err(reason) => {
                self.state.message(
                    "Could not save",
                    format!(
                        "{}\n\n{reason}\n\nThe worksheet is still open and still marked as modified.",
                        path.display()
                    ),
                );
                false
            }
        }
    }
}
