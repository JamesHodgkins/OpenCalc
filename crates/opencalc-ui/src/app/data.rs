//! Moving numbers between a table region and a CSV file. Both are commands that
//! copy data in or out: evaluating a worksheet never touches a file.

use std::path::{Path, PathBuf};

use opencalc_core::csv;
use opencalc_core::document::{RegionId, TableSpec};
use opencalc_core::edit::Edit;
use opencalc_core::persist::write_atomic;

use super::App;
use crate::canvas::CanvasCmd;
use crate::panels::table_window::selected_table;

impl App {
    fn selected_table_copy(&self) -> Option<(RegionId, TableSpec)> {
        selected_table(&self.state).map(|(id, spec)| (id, spec.clone()))
    }

    /// The folder of the active worksheet, where file dialogs start.
    fn sheet_folder(&self) -> Option<PathBuf> {
        self.state
            .active_sheet()
            .and_then(|s| s.path.clone())
            .and_then(|p| p.parent().map(Path::to_owned))
    }

    /// Ask for a CSV file and put its numbers into the selected table, as one
    /// undo step. The table keeps its name. A file that cannot be used changes
    /// nothing and says why.
    pub(super) fn import_csv(&mut self) {
        let Some((id, current)) = self.selected_table_copy() else {
            return;
        };
        let folder = self.sheet_folder();
        let Some(path) = self.files.pick_data_open(folder.as_deref()) else {
            return;
        };
        match read_table(&path) {
            Ok(table) => {
                let spec = TableSpec {
                    name: current.name,
                    ..table
                };
                let i = self.state.active.unwrap_or(0);
                let cmds = vec![
                    CanvasCmd::Edit(Edit::SetTable {
                        id,
                        spec: Box::new(spec),
                    }),
                    CanvasCmd::Seal,
                ];
                super::dispatch::apply_canvas(&mut self.state, i, cmds);
                self.recalculate();
            }
            Err(reason) => self.state.message(
                "Could not import",
                format!("{}\n\n{reason}\n\nNothing was changed.", path.display()),
            ),
        }
    }

    /// Ask where to put it and write the selected table as CSV.
    pub(super) fn export_csv(&mut self) {
        let Some((_, spec)) = self.selected_table_copy() else {
            return;
        };
        let name = format!(
            "{}.csv",
            if spec.name.trim().is_empty() {
                "table"
            } else {
                spec.name.trim()
            }
        );
        let folder = self.sheet_folder();
        let Some(mut path) = self.files.pick_data_save(&name, folder.as_deref()) else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("csv");
        }
        match write_atomic(&path, spec.to_csv(',').as_bytes()) {
            Ok(()) => self.state.message("Exported", path.display().to_string()),
            Err(e) => self.state.message(
                "Could not export",
                format!("{}\n\n{e}\n\nNothing was changed.", path.display()),
            ),
        }
    }
}

/// Reads a CSV or text file into a table.
fn read_table(path: &Path) -> Result<TableSpec, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > csv::MAX_BYTES as u64 {
        return Err(csv::CsvError::TooLarge.to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes)
        .map_err(|_| "the file is not UTF-8 text (save it as UTF-8 CSV)".to_owned())?;
    TableSpec::from_csv(&text)
}
