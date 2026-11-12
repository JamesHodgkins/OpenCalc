//! Exporting the active worksheet to a PDF or PNG file.

use std::path::{Path, PathBuf};

use opencalc_core::persist::write_atomic;

use super::App;
use crate::export::{self, Kind, Source};

/// The file a numbered page of a multi-page PNG export goes to: `name-2.png`.
pub fn numbered(path: &Path, page: usize) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "page".to_owned(), |s| s.to_string_lossy().into_owned());
    path.with_file_name(format!("{stem}-{}.png", page + 1))
}

impl App {
    /// Ask where to put it, draw the pages and write them. Reports the result, or
    /// what went wrong, in a message; the worksheet is never changed.
    pub(super) fn export_active(&mut self, kind: Kind) {
        // A printout of numbers that no longer match the inputs is the worst
        // surprise this program can give, so ask first.
        if self.state.results_stale() {
            self.state.dialog = Some(crate::app::filing::Dialog::StaleExport(kind));
            return;
        }
        self.export_now(kind);
    }

    /// Export without checking that the results are up to date.
    pub(super) fn export_now(&mut self, kind: Kind) {
        let Some(sheet) = self.state.active_sheet() else {
            return;
        };
        let name = format!("{}.{}", sheet.title(), kind.extension());
        let folder = sheet.path.as_deref().and_then(Path::parent);
        let Some(mut path) = self.files.pick_export(&name, kind, folder) else {
            return; // cancelled
        };
        if path.extension().is_none() {
            path.set_extension(kind.extension());
        }
        let sheet = &self.state.sheets[self.state.active.unwrap_or(0)];
        let source = Source {
            doc: &sheet.doc,
            output: &self.state.output,
            theme: &self.state.theme,
        };
        match write(kind, &source, &path, self.export_shrink) {
            Ok(files) => {
                let name = |f: &Path| {
                    f.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                };
                let text = match files.as_slice() {
                    [one] => format!("Exported {}", name(one)),
                    [first, .., last] => format!(
                        "Exported {} pages: {} to {}",
                        files.len(),
                        name(first),
                        name(last)
                    ),
                    [] => "Exported".to_owned(),
                };
                self.state.notify(text);
            }
            Err(reason) => self.state.message(
                "Could not export",
                format!("{}\n\n{reason}\n\nNothing was changed.", path.display()),
            ),
        }
    }
}

/// Write the export and return the files created.
fn write(kind: Kind, source: &Source, path: &Path, shrink: f32) -> Result<Vec<PathBuf>, String> {
    match kind {
        Kind::Pdf => {
            let bytes = export::pdf_bytes(source, export::PDF_SCALE * shrink)?;
            write_atomic(path, &bytes).map_err(|e| e.to_string())?;
            Ok(vec![path.to_owned()])
        }
        Kind::Png => {
            // Every page is drawn and encoded before anything is written, so a failure
            // leaves no half an export. One page goes to the chosen name; more are
            // numbered from it.
            let mut pages: Vec<(usize, Vec<u8>)> = Vec::new();
            export::render_pages(source, export::PNG_SCALE * shrink, |k, image, _| {
                pages.push((k, export::png_bytes(&image)?));
                Ok(())
            })?;
            let single = pages.len() == 1;
            let mut written = Vec::new();
            for (k, bytes) in pages {
                let target = if single {
                    path.to_owned()
                } else {
                    numbered(path, k)
                };
                write_atomic(&target, &bytes).map_err(|e| e.to_string())?;
                written.push(target);
            }
            Ok(written)
        }
    }
}
