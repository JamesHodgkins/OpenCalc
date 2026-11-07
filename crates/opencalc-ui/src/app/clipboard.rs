//! Cut, copy and paste of regions.
//!
//! Copying keeps two things: the regions themselves, inside the app, so pasting
//! them back is exact; and a plain-text version on the system clipboard (the math
//! sources and text, one per line) for other programs. Paste reads the system
//! clipboard: if it still holds what this app put there, the regions come back as
//! they were; if something else was copied since, that text becomes a text region.
//! While a math region is open for editing, the same commands work on the text
//! inside it.

use opencalc_core::document::{Point, Region, RegionKind};
use opencalc_core::edit::Edit;
use opencalc_core::persist::{MAX_REGIONS, MAX_TEXT_CHARS};

use super::dispatch::apply_canvas;
use super::App;
use crate::canvas::view::{snap, GRID};
use crate::canvas::CanvasCmd;
use crate::commands::CommandId;

/// The system clipboard, as far as this app needs it: text in, text out.
pub trait Clipboard {
    fn get(&mut self) -> Option<String>;
    fn set(&mut self, text: &str);
}

/// The operating system's clipboard.
pub struct NativeClipboard;

impl Clipboard for NativeClipboard {
    fn get(&mut self) -> Option<String> {
        arboard::Clipboard::new().ok()?.get_text().ok()
    }

    fn set(&mut self, text: &str) {
        if let Ok(mut board) = arboard::Clipboard::new() {
            let _ = board.set_text(text.to_owned());
        }
    }
}

/// A clipboard that lives inside the program (tests, and a fallback).
#[derive(Default)]
pub struct MemoryClipboard(pub Option<String>);

impl Clipboard for MemoryClipboard {
    fn get(&mut self) -> Option<String> {
        self.0.clone()
    }

    fn set(&mut self, text: &str) {
        self.0 = Some(text.to_owned());
    }
}

/// Regions that were copied: positions are relative to the group's top-left.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub regions: Vec<Region>,
    /// What went on the system clipboard, to recognise it again.
    pub text: String,
}

/// The plain text of regions, top to bottom: math sources and text regions.
fn plain_text(regions: &[&Region]) -> String {
    let mut lines: Vec<(f32, f32, String)> = Vec::new();
    for r in regions {
        let text = match &r.kind {
            RegionKind::Math(m) => m.source().to_owned(),
            RegionKind::Text { text } => text.clone(),
            RegionKind::Plot(p) if !p.title.is_empty() => p.title.clone(),
            _ => continue,
        };
        lines.push((r.pos.y, r.pos.x, text));
    }
    lines.sort_by(|a, b| {
        (a.0, a.1)
            .partial_cmp(&(b.0, b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let text: Vec<String> = lines.into_iter().map(|l| l.2).collect();
    if text.is_empty() {
        format!("OpenCalc: {} region(s)", regions.len())
    } else {
        text.join("\n")
    }
}

impl App {
    /// Copy the selected regions, or the selected text of a math region being edited.
    pub(super) fn copy(&mut self) {
        let Some(i) = self.state.active else { return };
        let sheet = &self.state.sheets[i];
        if let Some(field) = sheet.canvas.field.as_ref().filter(|f| f.has_selection()) {
            let text = field.selected_text();
            self.clipboard.set(&text);
            self.state.clip = None;
            return;
        }
        let chosen: Vec<&Region> = sheet
            .doc
            .regions()
            .iter()
            .filter(|r| sheet.canvas.selection.contains(&r.id))
            .collect();
        if chosen.is_empty() {
            return;
        }
        let left = chosen.iter().map(|r| r.pos.x).fold(f32::INFINITY, f32::min);
        let top = chosen.iter().map(|r| r.pos.y).fold(f32::INFINITY, f32::min);
        let text = plain_text(&chosen);
        let regions = chosen
            .iter()
            .map(|r| {
                let mut r = (*r).clone();
                r.pos = Point::new(r.pos.x - left, r.pos.y - top);
                r
            })
            .collect();
        self.clipboard.set(&text);
        self.state.clip = Some(Clip { regions, text });
    }

    /// Copy, then delete what was copied.
    pub(super) fn cut(&mut self) {
        let Some(i) = self.state.active else { return };
        let sheet = &self.state.sheets[i];
        if let Some(field) = sheet.canvas.field.as_ref().filter(|f| f.has_selection()) {
            let text = field.selected_text();
            self.clipboard.set(&text);
            self.state.clip = None;
            // Typing over the selection with nothing removes it.
            self.edit_field("");
        } else {
            self.copy();
            self.queue.push(CommandId::RegionDelete);
        }
    }

    /// Put `text` in place of the selection (at the caret if there is none) in the
    /// math region being edited, as an ordinary edit.
    fn edit_field(&mut self, text: &str) {
        let Some(i) = self.state.active else { return };
        let canvas = &self.state.sheets[i].canvas;
        let (Some(id), Some(field)) = (canvas.editing, canvas.field.as_ref()) else {
            return;
        };
        let mut after = field.clone();
        after.insert(text);
        let edit = Edit::SetMathSource {
            id,
            source: after.text(),
        };
        let effects = apply_canvas(&mut self.state, i, vec![CanvasCmd::Edit(edit)]);
        if !effects.is_empty() {
            self.recalculate();
        }
    }

    /// Paste: into the math region being edited, or as regions at the insertion point.
    pub(super) fn paste(&mut self) {
        let Some(i) = self.state.active else { return };
        let text = self.clipboard.get();
        if self.state.sheets[i].canvas.field.is_some() {
            if let Some(text) = text {
                self.edit_field(&text);
            }
            return;
        }
        let ours = match (&self.state.clip, &text) {
            (Some(clip), Some(text)) if clip.text == *text => Some(clip.regions.clone()),
            (Some(clip), None) => Some(clip.regions.clone()),
            _ => None,
        };
        let regions = match (ours, text) {
            (Some(regions), _) => regions,
            (None, Some(text)) => match external_text(&text) {
                Some(region_text) => vec![Region {
                    id: opencalc_core::document::RegionId(0),
                    pos: Point::default(),
                    size: RegionKind::Text {
                        text: String::new(),
                    }
                    .default_size(),
                    kind: RegionKind::Text { text: region_text },
                    style: Default::default(),
                    locked: false,
                    label: String::new(),
                }],
                None => return,
            },
            (None, None) => return,
        };
        self.paste_regions(i, regions);
    }

    fn paste_regions(&mut self, sheet_index: usize, regions: Vec<Region>) {
        let sheet = &mut self.state.sheets[sheet_index];
        if sheet.doc.regions().len() + regions.len() > MAX_REGIONS {
            self.state.message(
                "Paste",
                format!("A worksheet can hold at most {MAX_REGIONS} regions."),
            );
            return;
        }
        let at = sheet.canvas.insert_at;
        let base = sheet.doc.regions().len();
        let mut inserted = Vec::with_capacity(regions.len());
        let mut bottom = at.y;
        for (n, original) in regions.into_iter().enumerate() {
            let pos = Point::new(
                (at.x + original.pos.x).max(0.0),
                (at.y + original.pos.y).max(0.0),
            );
            let mut region = sheet.doc.new_region(original.kind, pos);
            region.size = original.size;
            region.style = original.style;
            region.label = original.label;
            bottom = bottom.max(region.pos.y + region.size.h);
            inserted.push((base + n, region));
        }
        let ids: Vec<_> = inserted.iter().map(|(_, r)| r.id).collect();
        let cmds = vec![
            CanvasCmd::Seal,
            CanvasCmd::Edit(Edit::InsertRegions { regions: inserted }),
            CanvasCmd::Seal,
        ];
        let effects = apply_canvas(&mut self.state, sheet_index, cmds);
        let sheet = &mut self.state.sheets[sheet_index];
        sheet.canvas.selection = ids;
        sheet.canvas.editing = None;
        sheet.canvas.field = None;
        // The next paste or insertion goes just below this one.
        sheet.canvas.insert_at = snap(eframe::egui::Pos2::new(at.x, bottom + 12.0), GRID);
        if !effects.is_empty() {
            self.recalculate();
        }
    }
}

/// Clipboard text from elsewhere, tidied into the text of a region: line breaks
/// normalised, cut to what a region can hold. `None` if there is nothing.
fn external_text(raw: &str) -> Option<String> {
    let text: String = raw
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        .take(MAX_TEXT_CHARS)
        .collect();
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_text_is_tidied_and_blank_text_is_nothing() {
        assert_eq!(external_text("a\r\nb\rc").as_deref(), Some("a\nb\nc"));
        assert_eq!(external_text("a\u{0}b\u{7}").as_deref(), Some("ab"));
        assert_eq!(external_text("  \n\t "), None);
        assert_eq!(external_text(""), None);
        let long = "x".repeat(MAX_TEXT_CHARS + 10);
        assert_eq!(
            external_text(&long).unwrap().chars().count(),
            MAX_TEXT_CHARS
        );
    }
}
