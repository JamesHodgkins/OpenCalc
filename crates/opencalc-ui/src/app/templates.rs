//! Templates: starting a new worksheet from a saved one, and saving the active
//! worksheet to start others from. A template is a worksheet file with the
//! extension `.ocxt` and `"kind": "template"` in its manifest, so there is one
//! format to keep working.

use std::path::Path;

use opencalc_core::edit::Edit;
use opencalc_core::persist::{self, TEMPLATE_EXTENSION};

use super::App;
use crate::sheet::OpenSheet;

impl App {
    /// Ask for a template and open a copy of it as a new untitled worksheet. The
    /// template file is never opened for editing here, so Save cannot overwrite it.
    pub(super) fn new_from_template(&mut self) {
        let Some(path) = self.files.pick_template_open() else {
            return;
        };
        let loaded = persist::read_limited(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| persist::load_bytes(&bytes).map_err(|e| e.to_string()));
        match loaded {
            Ok(loaded) => {
                self.state.untitled += 1;
                let title = format!("Untitled {}", self.state.untitled);
                let mut doc = loaded.worksheet;
                let _ = doc.apply(Edit::SetTitle(title));
                let mut sheet = OpenSheet::loaded(doc, None);
                // Not on disk yet: closing it asks about saving.
                sheet.mark_unsaved();
                self.state.show_opened(sheet);
                self.recalculate();
            }
            Err(reason) => self.state.message(
                "Could not use this template",
                format!("{}\n\n{reason}", path.display()),
            ),
        }
    }

    /// Ask where to write the active worksheet as a template. The worksheet itself
    /// is unchanged and keeps its own file.
    pub(super) fn save_as_template(&mut self) {
        let Some(sheet) = self.state.active_sheet() else {
            return;
        };
        let name = format!("{}.{TEMPLATE_EXTENSION}", sheet.title());
        let folder = sheet.path.as_deref().and_then(Path::parent);
        let Some(mut path) = self.files.pick_template_save(&name, folder) else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension(TEMPLATE_EXTENSION);
        }
        let doc = sheet.doc.clone();
        let written = persist::save_template_bytes(&doc)
            .map_err(|e| e.to_string())
            .and_then(|bytes| persist::write_atomic(&path, &bytes).map_err(|e| e.to_string()));
        match written {
            Ok(()) => self
                .state
                .message("Saved template", path.display().to_string()),
            Err(reason) => self.state.message(
                "Could not save the template",
                format!("{}\n\n{reason}\n\nNothing was changed.", path.display()),
            ),
        }
    }
}
