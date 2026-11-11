//! Undo and redo (SPEC section 5.5): a command history, not snapshots.
//!
//! The history stores, for each applied edit, the edit that reverses it. A burst
//! of the same kind of edit on the same target (typing in one region, dragging
//! the same regions) merges into one undo step: while the run stays open the
//! first reversal is kept and later ones are dropped, so one undo returns to the
//! state before the whole burst.
//!
//! The merge key is derived from the edit itself, never supplied by the caller.
//! Merging is only correct for "set to" edits of the same target, and deriving
//! the key makes it impossible to merge, say, a delete into a move.

use super::{Applied, Edit, EditError};
use crate::document::{RegionId, Worksheet};

/// What a run of mergeable edits is acting on.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RunKey {
    Move(Vec<RegionId>),
    Source(RegionId),
    Text(RegionId),
    Runs(RegionId),
    Size(RegionId),
    Box(RegionId),
    Plot(RegionId),
    Table(RegionId),
    Shape(RegionId),
    Line(RegionId),
    Page,
    Headers,
    Format(RegionId),
    Label(RegionId),
}

impl RunKey {
    /// The key for an edit, or `None` if this kind of edit never merges.
    fn of(edit: &Edit) -> Option<RunKey> {
        match edit {
            Edit::MoveRegions { moves } => {
                let mut ids: Vec<RegionId> = moves.iter().map(|(id, _)| *id).collect();
                ids.sort();
                Some(RunKey::Move(ids))
            }
            Edit::SetMathSource { id, .. } => Some(RunKey::Source(*id)),
            Edit::SetText { id, .. } => Some(RunKey::Text(*id)),
            Edit::SetRuns { id, .. } => Some(RunKey::Runs(*id)),
            Edit::SetRegionSize { id, .. } => Some(RunKey::Size(*id)),
            Edit::SetRegionBox { id, .. } => Some(RunKey::Box(*id)),
            Edit::SetPlot { id, .. } => Some(RunKey::Plot(*id)),
            Edit::SetTable { id, .. } => Some(RunKey::Table(*id)),
            Edit::SetShape { id, .. } => Some(RunKey::Shape(*id)),
            Edit::SetLineEnds { id, .. } => Some(RunKey::Line(*id)),
            Edit::SetPageSetup(_) => Some(RunKey::Page),
            Edit::SetHeaders(_) => Some(RunKey::Headers),
            Edit::SetResultFormat { id, .. } => Some(RunKey::Format(*id)),
            Edit::SetLabel { id, .. } => Some(RunKey::Label(*id)),
            Edit::InsertRegions { .. }
            | Edit::DeleteRegions { .. }
            | Edit::SetPicture { .. }
            | Edit::SetZ { .. }
            | Edit::SetBorder { .. }
            | Edit::SetFontStyle { .. }
            | Edit::SetLocked { .. }
            | Edit::SetUnitSystem(_)
            | Edit::SetCustomUnits(_)
            | Edit::SetTitle(_)
            | Edit::Batch(_) => None,
        }
    }
}

pub struct History {
    /// Each entry is the edit that reverts one step.
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// The run currently being merged into the top undo entry.
    open: Option<RunKey>,
    limit: usize,
}

impl Default for History {
    fn default() -> History {
        History::new(200)
    }
}

impl History {
    /// `limit` is the most undo steps kept; older ones are dropped.
    pub fn new(limit: usize) -> History {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
            open: None,
            limit: limit.max(1),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// End the current run, so the next edit starts a new undo step even if it
    /// would otherwise merge. Call when a gesture ends: mouse released, in-place
    /// editing finished, focus moved.
    pub fn seal(&mut self) {
        self.open = None;
    }

    /// Apply an edit and record how to undo it. Returns whether anything changed;
    /// a no-op is not recorded and leaves redo and the open run intact.
    pub fn apply(&mut self, doc: &mut Worksheet, edit: Edit) -> Result<bool, EditError> {
        let key = RunKey::of(&edit);
        let Applied { inverse, changed } = doc.apply(edit)?;
        if !changed {
            return Ok(false);
        }
        self.redo.clear();
        let merge = key.is_some() && key == self.open && !self.undo.is_empty();
        if !merge {
            self.undo.push(inverse);
            if self.undo.len() > self.limit {
                self.undo.remove(0);
            }
        }
        self.open = key;
        Ok(true)
    }

    /// Undo the last step. `Ok(false)` if there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Worksheet) -> Result<bool, EditError> {
        self.open = None;
        let Some(edit) = self.undo.pop() else {
            return Ok(false);
        };
        let applied = doc.apply(edit)?;
        self.redo.push(applied.inverse);
        Ok(true)
    }

    /// Redo the last undone step. `Ok(false)` if there is nothing to redo.
    pub fn redo(&mut self, doc: &mut Worksheet) -> Result<bool, EditError> {
        self.open = None;
        let Some(edit) = self.redo.pop() else {
            return Ok(false);
        };
        let applied = doc.apply(edit)?;
        self.undo.push(applied.inverse);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Point, RegionKind, Size};

    fn setup() -> (Worksheet, History, RegionId, RegionId) {
        let mut w = Worksheet::new("t");
        let mut h = History::default();
        let a = w.new_region(RegionKind::math("a := 1"), Point::default());
        let t = w.new_region(
            RegionKind::Text {
                text: String::new(),
            },
            Point::default(),
        );
        let (a_id, t_id) = (a.id, t.id);
        h.apply(
            &mut w,
            Edit::InsertRegions {
                regions: vec![(0, a), (1, t)],
            },
        )
        .unwrap();
        h.seal();
        (w, h, a_id, t_id)
    }

    fn source(w: &Worksheet, id: RegionId) -> String {
        match &w.region(id).unwrap().kind {
            RegionKind::Math(m) => m.source().to_owned(),
            _ => panic!("math"),
        }
    }

    fn type_into(w: &mut Worksheet, h: &mut History, id: RegionId, text: &str) {
        h.apply(
            w,
            Edit::SetMathSource {
                id,
                source: text.into(),
            },
        )
        .unwrap();
    }

    fn move_to(w: &mut Worksheet, h: &mut History, id: RegionId, x: f32, y: f32) {
        h.apply(
            w,
            Edit::MoveRegions {
                moves: vec![(id, Point::new(x, y))],
            },
        )
        .unwrap();
    }

    #[test]
    fn undo_and_redo_walk_back_and_forth() {
        let (mut w, mut h, a, _) = setup();
        move_to(&mut w, &mut h, a, 5.0, 5.0);
        h.seal();
        move_to(&mut w, &mut h, a, 9.0, 9.0);
        assert!(h.undo(&mut w).unwrap());
        assert_eq!(w.region(a).unwrap().pos, Point::new(5.0, 5.0));
        assert!(h.undo(&mut w).unwrap());
        assert_eq!(w.region(a).unwrap().pos, Point::default());
        assert!(h.redo(&mut w).unwrap());
        assert!(h.redo(&mut w).unwrap());
        assert_eq!(w.region(a).unwrap().pos, Point::new(9.0, 9.0));
        assert!(!h.can_redo());
    }

    #[test]
    fn nothing_to_undo_or_redo_is_not_an_error() {
        let mut w = Worksheet::new("t");
        let mut h = History::default();
        assert_eq!(h.undo(&mut w), Ok(false));
        assert_eq!(h.redo(&mut w), Ok(false));
        assert!(!h.can_undo() && !h.can_redo());
    }

    #[test]
    fn a_new_edit_clears_the_redo_stack() {
        let (mut w, mut h, a, _) = setup();
        move_to(&mut w, &mut h, a, 1.0, 1.0);
        h.undo(&mut w).unwrap();
        assert!(h.can_redo());
        move_to(&mut w, &mut h, a, 2.0, 2.0);
        assert!(!h.can_redo());
    }

    #[test]
    fn typing_in_one_region_undoes_as_one_step() {
        let (mut w, mut h, a, _) = setup();
        for text in ["a := 2", "a := 20", "a := 200", "a := 2000"] {
            type_into(&mut w, &mut h, a, text);
        }
        assert_eq!(source(&w, a), "a := 2000");
        h.undo(&mut w).unwrap();
        assert_eq!(
            source(&w, a),
            "a := 1",
            "the whole burst, not one keystroke"
        );
        h.redo(&mut w).unwrap();
        assert_eq!(
            source(&w, a),
            "a := 2000",
            "redo restores the end of the burst"
        );
    }

    #[test]
    fn sealing_starts_a_new_step() {
        let (mut w, mut h, a, _) = setup();
        type_into(&mut w, &mut h, a, "a := 2");
        h.seal(); // editing finished
        type_into(&mut w, &mut h, a, "a := 3");
        h.undo(&mut w).unwrap();
        assert_eq!(source(&w, a), "a := 2");
        h.undo(&mut w).unwrap();
        assert_eq!(source(&w, a), "a := 1");
    }

    #[test]
    fn edits_of_different_targets_do_not_merge() {
        let (mut w, mut h, a, t) = setup();
        type_into(&mut w, &mut h, a, "a := 2");
        h.apply(
            &mut w,
            Edit::SetText {
                id: t,
                text: "hello".into(),
            },
        )
        .unwrap();
        h.undo(&mut w).unwrap();
        assert_eq!(source(&w, a), "a := 2", "only the text edit was undone");
    }

    #[test]
    fn a_delete_never_merges_into_a_preceding_move() {
        let (mut w, mut h, a, _) = setup();
        move_to(&mut w, &mut h, a, 7.0, 7.0);
        h.apply(&mut w, Edit::DeleteRegions { ids: vec![a] })
            .unwrap();
        h.undo(&mut w).unwrap();
        assert_eq!(
            w.region(a).unwrap().pos,
            Point::new(7.0, 7.0),
            "the region is back where the move left it"
        );
        h.undo(&mut w).unwrap();
        assert_eq!(w.region(a).unwrap().pos, Point::default());
    }

    #[test]
    fn a_drag_is_one_step() {
        let (mut w, mut h, a, _) = setup();
        for x in 1..=30 {
            move_to(&mut w, &mut h, a, x as f32, 0.0);
        }
        h.seal(); // mouse released
        assert_eq!(w.region(a).unwrap().pos.x, 30.0);
        h.undo(&mut w).unwrap();
        assert_eq!(w.region(a).unwrap().pos, Point::default());
    }

    #[test]
    fn dragging_a_different_set_of_regions_starts_a_new_step() {
        let (mut w, mut h, a, t) = setup();
        move_to(&mut w, &mut h, a, 1.0, 0.0);
        move_to(&mut w, &mut h, t, 2.0, 0.0); // same kind of edit, other target, no seal
        h.undo(&mut w).unwrap();
        assert_eq!(w.region(t).unwrap().pos, Point::default());
        assert_eq!(w.region(a).unwrap().pos, Point::new(1.0, 0.0));
    }

    #[test]
    fn a_no_op_is_not_recorded_and_keeps_redo() {
        let (mut w, mut h, a, _) = setup();
        move_to(&mut w, &mut h, a, 4.0, 4.0);
        h.undo(&mut w).unwrap();
        assert!(h.can_redo());
        // Dragging "to where it already is" happens every frame of a still mouse.
        let changed = h
            .apply(
                &mut w,
                Edit::MoveRegions {
                    moves: vec![(a, Point::default())],
                },
            )
            .unwrap();
        assert!(!changed);
        assert!(h.can_redo(), "a no-op must not destroy redo");
    }

    #[test]
    fn a_failed_edit_leaves_history_alone() {
        let (mut w, mut h, ..) = setup();
        let before = h.can_undo();
        assert!(h
            .apply(
                &mut w,
                Edit::DeleteRegions {
                    ids: vec![RegionId(404)]
                }
            )
            .is_err());
        assert_eq!(h.can_undo(), before);
    }

    #[test]
    fn the_history_is_bounded() {
        let mut w = Worksheet::new("t");
        let mut h = History::new(3);
        let r = w.new_region(RegionKind::plot(), Point::default());
        let id = r.id;
        h.apply(
            &mut w,
            Edit::InsertRegions {
                regions: vec![(0, r)],
            },
        )
        .unwrap();
        for x in 1..=10 {
            move_to(&mut w, &mut h, id, x as f32, 0.0);
            h.seal();
        }
        let mut steps = 0;
        while h.undo(&mut w).unwrap() {
            steps += 1;
        }
        assert_eq!(steps, 3);
    }

    /// A deterministic pseudo-random walk over every kind of edit, sealing at
    /// random. After any sequence, undoing everything must give back the start
    /// and redoing everything must give back the end.
    #[test]
    fn undo_all_then_redo_all_is_exact_for_random_edit_sequences() {
        for seed in 1..=60u64 {
            let mut state = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let mut next = move |n: usize| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((state >> 33) as usize) % n.max(1)
            };
            let mut w = Worksheet::new("t");
            let mut h = History::new(1000);
            let start = w.clone();
            for step in 0..80 {
                let ids: Vec<RegionId> = w.regions().iter().map(|r| r.id).collect();
                let target = if ids.is_empty() {
                    None
                } else {
                    Some(ids[next(ids.len())])
                };
                let edit = match (next(6), target) {
                    (0, _) | (_, None) => {
                        let kind = match next(3) {
                            0 => RegionKind::math("v := 1"),
                            1 => RegionKind::Text {
                                text: String::new(),
                            },
                            _ => RegionKind::plot(),
                        };
                        let r = w.new_region(kind, Point::new(step as f32, 0.0));
                        let at = next(w.regions().len() + 1);
                        Edit::InsertRegions {
                            regions: vec![(at, r)],
                        }
                    }
                    (1, Some(id)) => Edit::DeleteRegions { ids: vec![id] },
                    (2 | 3, Some(id)) => Edit::MoveRegions {
                        moves: vec![(id, Point::new(next(500) as f32, next(500) as f32))],
                    },
                    (4, Some(id)) => match w.region(id).map(|r| r.kind.label()) {
                        Some("Math") => Edit::SetMathSource {
                            id,
                            source: format!("v := {}", next(99)),
                        },
                        Some("Text") => Edit::SetText {
                            id,
                            text: format!("t{}", next(99)),
                        },
                        _ => Edit::SetRegionSize {
                            id,
                            size: Size::new(next(300) as f32, 50.0),
                        },
                    },
                    (_, Some(id)) => Edit::SetRegionSize {
                        id,
                        size: Size::new(next(300) as f32, 70.0),
                    },
                };
                h.apply(&mut w, edit).unwrap();
                if next(3) == 0 {
                    h.seal();
                }
            }
            let end = w.clone();
            while h.undo(&mut w).unwrap() {}
            assert_eq!(w, start, "seed {seed}: undo everything");
            while h.redo(&mut w).unwrap() {}
            assert_eq!(w, end, "seed {seed}: redo everything");
        }
    }
}
