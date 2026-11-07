//! An open worksheet: the document (owned by core) plus the UI state around it.
//!
//! This is the one place UI code applies edits. Selection, editing, drag, the
//! view and the undo history live here, beside the document and not in it
//! (rule R2).

use std::path::{Path, PathBuf};

use eframe::egui::{Pos2, Rect};
use opencalc_core::document::{PlotSpec, Point, RegionId, RegionKind, Worksheet};
use opencalc_core::edit::{Edit, History};

use crate::canvas::adapt::{point, rect_of};
use crate::canvas::view::{snap, GRID};
use crate::canvas::{CanvasUi, CanvasView};

pub struct OpenSheet {
    pub doc: Worksheet,
    pub history: History,
    pub view: CanvasView,
    pub canvas: CanvasUi,
    /// The file this worksheet was opened from or last saved to.
    pub path: Option<PathBuf>,
    /// The document revision that is on disk. The worksheet is modified when its
    /// revision differs. Undoing back to the saved state still counts as modified,
    /// the safe way to be wrong (docs/PERSISTENCE.md).
    saved_revision: u64,
}

impl OpenSheet {
    pub fn new(title: &str) -> OpenSheet {
        OpenSheet::from_doc(Worksheet::new(title))
    }

    fn from_doc(doc: Worksheet) -> OpenSheet {
        // The first input goes at the top left of the page margin, rounded in to
        // the snap grid so it sits inside the margin, not across its edge.
        let margin = doc.page().margin();
        let corner = (margin / GRID).ceil() * GRID;
        let doc_revision = doc.revision();
        let mut canvas = CanvasUi::default();
        canvas.insert_at = Pos2::new(corner, corner);
        OpenSheet {
            doc,
            history: History::default(),
            view: CanvasView::default(),
            canvas,
            path: None,
            saved_revision: doc_revision,
        }
    }

    /// A worksheet read from `path`, unmodified, with a clean history.
    pub fn loaded(doc: Worksheet, path: PathBuf) -> OpenSheet {
        let mut s = OpenSheet::from_doc(doc);
        s.path = Some(path);
        // New regions go below what is already there, not on top of it.
        if let Some(bottom) = s.bounds().map(|b| b.max.y) {
            s.canvas.insert_at = snap(Pos2::new(s.canvas.insert_at.x, bottom + 12.0), GRID);
        }
        s
    }

    /// Whether the worksheet has changes that are not on disk.
    pub fn dirty(&self) -> bool {
        self.doc.revision() != self.saved_revision
    }

    /// Record that the document as it is now is on disk at `path`.
    pub fn mark_saved(&mut self, path: &Path) {
        self.saved_revision = self.doc.revision();
        self.path = Some(path.to_owned());
    }

    /// Treat the document as unmodified (changes thrown away on purpose).
    pub fn mark_clean(&mut self) {
        self.saved_revision = self.doc.revision();
    }

    /// Nothing typed, nothing saved: a blank sheet that opening a file may replace.
    pub fn is_pristine(&self) -> bool {
        self.path.is_none() && !self.dirty() && self.doc.regions().is_empty()
    }

    /// A small worksheet in deliberately non-dependency order, spread over the
    /// page. Building it is not undoable: undo stops at the finished example.
    pub fn sample() -> OpenSheet {
        let mut s = OpenSheet::new("Beam stress");
        s.add(
            RegionKind::Text {
                text: "Axial stress in a square bar".to_owned(),
            },
            Point::new(40.0, 20.0),
        );
        for (src, x, y) in [
            ("\u{03c3} := F / A", 60.0, 204.0),
            ("F := 10 kN", 60.0, 84.0),
            ("b := 100 mm", 60.0, 120.0),
            ("A := b^2", 60.0, 156.0),
            ("s := sqrt(A)", 360.0, 156.0),
            ("\u{03c3} =", 360.0, 204.0),
            ("w := 50 mm, 60 mm..150 mm", 60.0, 276.0),
        ] {
            s.add(RegionKind::math(src), Point::new(x, y));
        }
        // Stress against bar width, to show a plot.
        let spec = PlotSpec::default()
            .with_title("Stress against width")
            .with_x("w")
            .with_series(0, "F / w ^ 2")
            .with_labels("width", "stress");
        s.add(RegionKind::Plot(spec), Point::new(60.0, 336.0));
        // New regions go below the worked example, not on top of it.
        s.canvas.insert_at = Pos2::new(40.0, 520.0);
        s.history = History::default();
        s.mark_clean();
        s
    }

    pub fn title(&self) -> &str {
        self.doc.title()
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Apply an edit through the history. Returns whether the document changed.
    /// A rejected edit means the caller built it from stale state, which is a bug
    /// in the UI: it is loud in debug builds and harmless in release builds.
    pub fn edit(&mut self, edit: Edit) -> bool {
        match self.history.apply(&mut self.doc, edit) {
            Ok(changed) => {
                if changed {
                    self.canvas.retain_existing(self.doc.regions());
                }
                changed
            }
            Err(e) => {
                debug_assert!(false, "edit rejected: {e}");
                false
            }
        }
    }

    /// End the current gesture: the next edit starts a new undo step.
    pub fn seal(&mut self) {
        self.history.seal();
    }

    pub fn undo(&mut self) -> bool {
        let done = self.history.undo(&mut self.doc);
        self.after_history(done)
    }

    pub fn redo(&mut self) -> bool {
        let done = self.history.redo(&mut self.doc);
        self.after_history(done)
    }

    fn after_history(&mut self, done: Result<bool, opencalc_core::edit::EditError>) -> bool {
        match done {
            Ok(true) => {
                self.canvas.cancel_gesture();
                self.canvas.retain_existing(self.doc.regions());
                true
            }
            Ok(false) => false,
            Err(e) => {
                debug_assert!(false, "history edit rejected: {e}");
                false
            }
        }
    }

    /// Add a region at the end of the z-order as its own undo step.
    pub fn add(&mut self, kind: RegionKind, pos: Point) -> RegionId {
        self.history.seal();
        let region = self.doc.new_region(kind, pos);
        let id = region.id;
        let index = self.doc.regions().len();
        self.edit(Edit::InsertRegions {
            regions: vec![(index, region)],
        });
        self.history.seal();
        id
    }

    /// Add a region at the sheet's insertion point and select it. Math and text
    /// regions open for editing straight away. The next insertion goes just
    /// below this one, on the grid.
    pub fn insert(&mut self, kind: RegionKind) -> RegionId {
        let at = self.canvas.insert_at;
        let editable = kind.is_editable();
        let id = self.add(kind, point(at));
        let height = self.doc.region(id).map_or(40.0, |r| r.size.h);
        self.canvas.insert_at = snap(Pos2::new(at.x, at.y + height + 12.0), GRID);
        self.canvas.selection = vec![id];
        if editable {
            self.canvas.editing = Some(id);
            self.canvas.focus_edit = true;
        }
        id
    }

    /// Delete the selected regions as one undo step. Returns whether any were.
    pub fn delete_selection(&mut self) -> bool {
        let ids = self.canvas.selection.clone();
        if ids.is_empty() {
            return false;
        }
        self.history.seal();
        let changed = self.edit(Edit::DeleteRegions { ids });
        self.history.seal();
        changed
    }

    /// Bounding box of every region in document space, using measured sizes.
    pub fn bounds(&self) -> Option<Rect> {
        self.doc
            .regions()
            .iter()
            .map(|r| rect_of(r, &self.canvas.sizes))
            .reduce(|a, b| a.union(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Vec2;

    fn math_count(s: &OpenSheet) -> usize {
        s.doc.math_regions().len()
    }

    #[test]
    fn building_the_sample_is_not_undoable() {
        let s = OpenSheet::sample();
        assert!(!s.can_undo());
        assert_eq!(s.doc.regions().len(), 9);
        assert_eq!(math_count(&s), 7);
    }

    #[test]
    fn insert_selects_the_region_and_undo_removes_it() {
        let mut s = OpenSheet::sample();
        let id = s.insert(RegionKind::plot());
        assert_eq!(s.canvas.selection, vec![id]);
        assert_eq!(s.doc.regions().len(), 10);
        assert!(s.undo());
        assert_eq!(s.doc.regions().len(), 9);
        assert!(
            s.canvas.selection.is_empty(),
            "the selection followed the region away"
        );
        assert!(s.redo());
        assert!(s.doc.region(id).is_some());
    }

    #[test]
    fn consecutive_inserts_flow_downwards_and_undo_one_at_a_time() {
        let mut s = OpenSheet::new("t");
        let a = s.insert(RegionKind::plot());
        let b = s.insert(RegionKind::plot());
        let (pa, pb) = (s.doc.region(a).unwrap().pos, s.doc.region(b).unwrap().pos);
        assert!(
            pb.y > pa.y + 170.0,
            "second plot below the first: {pa:?} {pb:?}"
        );
        s.undo();
        assert!(s.doc.region(b).is_none() && s.doc.region(a).is_some());
    }

    #[test]
    fn delete_selection_is_one_undoable_step_that_restores_order() {
        let mut s = OpenSheet::sample();
        let before: Vec<RegionId> = s.doc.regions().iter().map(|r| r.id).collect();
        s.canvas.selection = vec![before[1], before[4]];
        assert!(s.delete_selection());
        assert_eq!(s.doc.regions().len(), 7);
        assert!(s.canvas.selection.is_empty());
        assert!(s.undo());
        let after: Vec<RegionId> = s.doc.regions().iter().map(|r| r.id).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn deleting_nothing_does_nothing() {
        let mut s = OpenSheet::sample();
        assert!(!s.delete_selection());
        assert!(!s.can_undo());
    }

    #[test]
    fn a_whole_drag_undoes_in_one_step() {
        let mut s = OpenSheet::sample();
        let id = s.doc.regions()[1].id;
        let start = s.doc.region(id).unwrap().pos;
        for step in 1..=25 {
            s.edit(Edit::MoveRegions {
                moves: vec![(id, Point::new(start.x + step as f32, start.y))],
            });
        }
        s.seal(); // mouse released
        assert!(s.undo());
        assert_eq!(s.doc.region(id).unwrap().pos, start);
        assert!(!s.can_undo(), "nothing else was recorded");
    }

    #[test]
    fn editing_math_source_changes_evaluation_but_moving_does_not() {
        let mut s = OpenSheet::sample();
        let id = s.doc.regions()[2].id; // "F := 10 kN"
        let e0 = s.doc.eval_revision();
        s.edit(Edit::MoveRegions {
            moves: vec![(id, Point::new(500.0, 500.0))],
        });
        assert_eq!(s.doc.eval_revision(), e0);
        s.edit(Edit::SetMathSource {
            id,
            source: "F := 20 kN".into(),
        });
        assert_eq!(s.doc.eval_revision(), e0 + 1);
    }

    #[test]
    fn bounds_use_measured_sizes_for_content_sized_regions() {
        let mut s = OpenSheet::new("t");
        assert!(s.bounds().is_none());
        let id = s.add(RegionKind::math("x := 1"), Point::new(0.0, 0.0));
        let default = s.bounds().unwrap();
        s.canvas.sizes.insert(id, Vec2::new(900.0, 40.0));
        assert!(s.bounds().unwrap().width() > default.width());
    }

    #[test]
    fn undo_cancels_an_in_progress_drag_and_forgets_removed_regions() {
        let mut s = OpenSheet::sample();
        let id = s.insert(RegionKind::plot());
        s.canvas.sizes.insert(id, Vec2::new(1.0, 1.0));
        s.undo();
        assert!(!s.canvas.sizes.contains_key(&id));
    }
}
