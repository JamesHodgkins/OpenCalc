//! Selecting and copying regions of an open worksheet: Select all, Tab through the
//! regions, Duplicate.

use opencalc_core::document::{Point, Region, RegionId};
use opencalc_core::edit::Edit;

use super::OpenSheet;

impl OpenSheet {
    /// Copy the selected regions one grid step down and to the right, as one undo
    /// step, and select the copies. A copy keeps the original's look, text and
    /// size but not its label (a label names one region) or its lock. Returns
    /// whether anything was copied.
    pub fn duplicate_selection(&mut self) -> bool {
        let step = self.canvas.grid.step();
        let originals: Vec<_> = self
            .doc
            .regions()
            .iter()
            .filter(|r| self.canvas.selection.contains(&r.id))
            .cloned()
            .collect();
        if originals.is_empty() {
            return false;
        }
        let base = self.doc.regions().len();
        let copies: Vec<(usize, Region)> = originals
            .into_iter()
            .enumerate()
            .map(|(n, original)| {
                let pos = Point::new(original.pos.x + step, original.pos.y + step);
                let mut copy = self.doc.new_region(original.kind, pos);
                copy.size = original.size;
                copy.style = original.style;
                copy.runs = original.runs;
                copy.border = original.border;
                (base + n, copy)
            })
            .collect();
        let ids: Vec<RegionId> = copies.iter().map(|(_, r)| r.id).collect();
        self.history.seal();
        self.edit(Edit::InsertRegions { regions: copies });
        self.history.seal();
        self.canvas.selection = ids;
        self.canvas.editing = None;
        self.canvas.field = None;
        true
    }

    /// Select the next (or previous) region in reading order, top to bottom and left
    /// to right, wrapping round, and bring it into view. From a group it goes on
    /// from the last (or first) of them. Returns the region now selected.
    pub fn cycle_selection(
        &mut self,
        forward: bool,
        viewport: eframe::egui::Vec2,
    ) -> Option<RegionId> {
        let mut order: Vec<(f32, f32, RegionId)> = self
            .doc
            .regions()
            .iter()
            .map(|r| (r.pos.y, r.pos.x, r.id))
            .collect();
        order.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        let index_of = |id: &RegionId| order.iter().position(|(_, _, o)| o == id);
        let here = if forward {
            self.canvas.selection.iter().filter_map(index_of).max()
        } else {
            self.canvas.selection.iter().filter_map(index_of).min()
        };
        let n = order.len();
        let next = match (here, forward) {
            (None, true) => 0,
            (None, false) => n.checked_sub(1)?,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        };
        let id = order.get(next)?.2;
        self.canvas.selection = vec![id];
        self.canvas.editing = None;
        self.canvas.field = None;
        self.reveal(id, viewport);
        Some(id)
    }

    /// Select every region (and stop editing, so the arrow keys move them).
    pub fn select_all(&mut self) {
        self.canvas.selection = self.doc.regions().iter().map(|r| r.id).collect();
        self.canvas.editing = None;
        self.canvas.field = None;
    }
}
