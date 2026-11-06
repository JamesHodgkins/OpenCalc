//! Every change to a worksheet is an [`Edit`] (rule R3).
//!
//! [`Worksheet::apply`] is the only way to change a worksheet's content. It
//! applies an edit completely or not at all, and returns the edit that undoes
//! it, which is what undo/redo and [`History`] are built from.
//!
//! Edits are "set to" statements (move these regions *to* these places) rather
//! than deltas. That makes them idempotent to apply, and lets two consecutive
//! edits of the same thing merge by keeping the last one.

mod history;

use crate::document::{Point, Region, RegionId, RegionKind, Size, Worksheet};

pub use history::History;

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Insert regions. Each index is the region's final position in the list;
    /// indices must be ascending.
    InsertRegions {
        regions: Vec<(usize, Region)>,
    },
    DeleteRegions {
        ids: Vec<RegionId>,
    },
    /// Move regions to new top-left positions. Each ID at most once.
    MoveRegions {
        moves: Vec<(RegionId, Point)>,
    },
    SetMathSource {
        id: RegionId,
        source: String,
    },
    SetText {
        id: RegionId,
        text: String,
    },
    /// A size chosen by the user (resizing). Not the measured content size.
    SetRegionSize {
        id: RegionId,
        size: Size,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("no region with id {0:?}")]
    NoSuchRegion(RegionId),
    #[error("region {0:?} is mentioned twice or already exists")]
    Duplicate(RegionId),
    #[error("insert position {index} is out of range (worksheet has {len} regions)")]
    BadIndex { index: usize, len: usize },
    #[error("insert positions must be ascending")]
    UnorderedIndices,
    #[error("region {0:?} is not the right kind for this edit")]
    WrongKind(RegionId),
}

/// The result of applying an edit.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    /// The edit that exactly reverses this one.
    pub inverse: Edit,
    /// False if the edit set things to what they already were. A no-op leaves the
    /// worksheet, and its revision, untouched.
    pub changed: bool,
}

impl Worksheet {
    /// Apply an edit, all or nothing. On error the worksheet is unchanged.
    pub fn apply(&mut self, edit: Edit) -> Result<Applied, EditError> {
        let inverse = self.apply_inner(&edit)?;
        let changed = inverse != edit;
        if changed {
            self.revision += 1;
        }
        Ok(Applied { inverse, changed })
    }

    fn apply_inner(&mut self, edit: &Edit) -> Result<Edit, EditError> {
        match edit {
            Edit::InsertRegions { regions } => self.insert(regions),
            Edit::DeleteRegions { ids } => self.delete(ids),
            Edit::MoveRegions { moves } => self.move_regions(moves),
            Edit::SetMathSource { id, source } => {
                let region = self.region_mut(*id)?;
                let RegionKind::Math(m) = &mut region.kind else {
                    return Err(EditError::WrongKind(*id));
                };
                let old = m.source().to_owned();
                m.set_source(source.clone());
                Ok(Edit::SetMathSource {
                    id: *id,
                    source: old,
                })
            }
            Edit::SetText { id, text } => {
                let region = self.region_mut(*id)?;
                let RegionKind::Text { text: current } = &mut region.kind else {
                    return Err(EditError::WrongKind(*id));
                };
                let old = std::mem::replace(current, text.clone());
                Ok(Edit::SetText { id: *id, text: old })
            }
            Edit::SetRegionSize { id, size } => {
                let region = self.region_mut(*id)?;
                let old = std::mem::replace(&mut region.size, *size);
                Ok(Edit::SetRegionSize { id: *id, size: old })
            }
        }
    }

    fn region_mut(&mut self, id: RegionId) -> Result<&mut Region, EditError> {
        self.regions
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(EditError::NoSuchRegion(id))
    }

    fn insert(&mut self, regions: &[(usize, Region)]) -> Result<Edit, EditError> {
        // Validate everything first so a bad edit changes nothing.
        let mut seen: Vec<RegionId> = Vec::new();
        let mut previous: Option<usize> = None;
        for (k, (index, region)) in regions.iter().enumerate() {
            if self.region(region.id).is_some() || seen.contains(&region.id) {
                return Err(EditError::Duplicate(region.id));
            }
            if previous.is_some_and(|p| *index <= p) {
                return Err(EditError::UnorderedIndices);
            }
            let len = self.regions.len() + k;
            if *index > len {
                return Err(EditError::BadIndex { index: *index, len });
            }
            seen.push(region.id);
            previous = Some(*index);
        }
        for (index, region) in regions {
            self.regions.insert(*index, region.clone());
        }
        Ok(Edit::DeleteRegions { ids: seen })
    }

    fn delete(&mut self, ids: &[RegionId]) -> Result<Edit, EditError> {
        for (k, id) in ids.iter().enumerate() {
            if ids[..k].contains(id) {
                return Err(EditError::Duplicate(*id));
            }
            if self.region(*id).is_none() {
                return Err(EditError::NoSuchRegion(*id));
            }
        }
        // Remember each region with its position so the inverse can restore it.
        let mut removed: Vec<(usize, Region)> = Vec::new();
        for (index, r) in self.regions.iter().enumerate() {
            if ids.contains(&r.id) {
                removed.push((index, r.clone()));
            }
        }
        self.regions.retain(|r| !ids.contains(&r.id));
        Ok(Edit::InsertRegions { regions: removed })
    }

    fn move_regions(&mut self, moves: &[(RegionId, Point)]) -> Result<Edit, EditError> {
        for (k, (id, _)) in moves.iter().enumerate() {
            if moves[..k].iter().any(|(other, _)| other == id) {
                return Err(EditError::Duplicate(*id));
            }
            if self.region(*id).is_none() {
                return Err(EditError::NoSuchRegion(*id));
            }
        }
        let mut undo = Vec::with_capacity(moves.len());
        for (id, to) in moves {
            let region = self.region_mut(*id)?;
            undo.push((*id, std::mem::replace(&mut region.pos, *to)));
        }
        Ok(Edit::MoveRegions { moves: undo })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> (Worksheet, Vec<RegionId>) {
        let mut w = Worksheet::new("t");
        let mut ids = Vec::new();
        for (i, kind) in [
            RegionKind::math("a := 1"),
            RegionKind::Text { text: "hi".into() },
            RegionKind::Plot,
            RegionKind::math("b := a + 1"),
            RegionKind::Table,
        ]
        .into_iter()
        .enumerate()
        {
            let r = w.new_region(kind, Point::new(i as f32 * 10.0, 0.0));
            ids.push(r.id);
            let n = w.regions.len();
            w.apply(Edit::InsertRegions {
                regions: vec![(n, r)],
            })
            .unwrap();
        }
        (w, ids)
    }

    /// Applying an edit and then its inverse must restore the original content.
    fn round_trips(edit: Edit) {
        let (mut w, _) = sheet();
        let before = w.clone();
        let applied = w.apply(edit.clone()).unwrap();
        assert!(applied.changed, "{edit:?} should change something");
        assert_ne!(w, before);
        let undone = w.apply(applied.inverse).unwrap();
        assert_eq!(w, before, "inverse of {edit:?}");
        // And the inverse of the inverse is the original edit.
        assert_eq!(undone.inverse, edit);
    }

    #[test]
    fn every_edit_has_an_exact_inverse() {
        let (w, ids) = sheet();
        round_trips(Edit::MoveRegions {
            moves: vec![
                (ids[0], Point::new(5.0, 6.0)),
                (ids[2], Point::new(7.0, 8.0)),
            ],
        });
        round_trips(Edit::SetMathSource {
            id: ids[0],
            source: "a := 99".into(),
        });
        round_trips(Edit::SetText {
            id: ids[1],
            text: "changed".into(),
        });
        round_trips(Edit::SetRegionSize {
            id: ids[2],
            size: Size::new(1.0, 2.0),
        });
        round_trips(Edit::DeleteRegions {
            ids: vec![ids[1], ids[3]],
        });
        let mut w = w;
        let extra = w.new_region(RegionKind::Picture, Point::default());
        round_trips_on(
            w,
            Edit::InsertRegions {
                regions: vec![(2, extra)],
            },
        );
    }

    fn round_trips_on(mut w: Worksheet, edit: Edit) {
        let before = w.clone();
        let applied = w.apply(edit).unwrap();
        w.apply(applied.inverse).unwrap();
        assert_eq!(w, before);
    }

    #[test]
    fn deleting_non_adjacent_regions_restores_their_original_positions() {
        let (mut w, ids) = sheet();
        let order_before: Vec<RegionId> = w.regions().iter().map(|r| r.id).collect();
        let applied = w
            .apply(Edit::DeleteRegions {
                ids: vec![ids[3], ids[1]], // deliberately out of list order
            })
            .unwrap();
        assert_eq!(w.regions().len(), 3);
        w.apply(applied.inverse).unwrap();
        let order_after: Vec<RegionId> = w.regions().iter().map(|r| r.id).collect();
        assert_eq!(order_after, order_before);
    }

    #[test]
    fn math_source_edits_keep_the_parsed_expression_in_step() {
        let (mut w, ids) = sheet();
        w.apply(Edit::SetMathSource {
            id: ids[0],
            source: "a := 5 m".into(),
        })
        .unwrap();
        let RegionKind::Math(m) = &w.region(ids[0]).unwrap().kind else {
            panic!("math region");
        };
        assert_eq!(m.expr(), &crate::parser::parse("a := 5 m"));
    }

    #[test]
    fn a_failed_edit_changes_nothing() {
        let (mut w, ids) = sheet();
        let before = w.clone();
        let rev = w.revision();
        let missing = RegionId(999);
        let bad = [
            Edit::DeleteRegions {
                ids: vec![ids[0], missing],
            },
            Edit::DeleteRegions {
                ids: vec![ids[0], ids[0]],
            },
            Edit::MoveRegions {
                moves: vec![(ids[0], Point::new(1.0, 1.0)), (missing, Point::default())],
            },
            Edit::MoveRegions {
                moves: vec![(ids[0], Point::default()), (ids[0], Point::default())],
            },
            Edit::SetMathSource {
                id: ids[1],
                source: "x".into(),
            }, // a text region
            Edit::SetText {
                id: ids[0],
                text: "x".into(),
            }, // a math region
            Edit::SetRegionSize {
                id: missing,
                size: Size::default(),
            },
        ];
        for edit in bad {
            assert!(w.apply(edit.clone()).is_err(), "{edit:?}");
            assert_eq!(w, before, "{edit:?} must not partially apply");
            assert_eq!(w.revision(), rev);
        }
    }

    #[test]
    fn insert_validation() {
        let (mut w, ids) = sheet();
        let before = w.clone();
        let existing = w.region(ids[0]).unwrap().clone();
        let fresh = w.new_region(RegionKind::Plot, Point::default());
        let fresh2 = w.new_region(RegionKind::Plot, Point::default());
        // Duplicate of an existing ID; duplicate within the edit; index past the end;
        // indices not ascending.
        let cases = [
            (vec![(0, existing)], EditError::Duplicate(ids[0])),
            (
                vec![(0, fresh.clone()), (1, fresh.clone())],
                EditError::Duplicate(fresh.id),
            ),
            (
                vec![(99, fresh.clone())],
                EditError::BadIndex { index: 99, len: 5 },
            ),
            (
                vec![(3, fresh.clone()), (2, fresh2.clone())],
                EditError::UnorderedIndices,
            ),
        ];
        for (regions, expected) in cases {
            assert_eq!(w.apply(Edit::InsertRegions { regions }), Err(expected));
            assert_eq!(w, before);
        }
    }

    #[test]
    fn setting_something_to_what_it_already_is_a_no_op() {
        let (mut w, ids) = sheet();
        let rev = w.revision();
        let pos = w.region(ids[0]).unwrap().pos;
        let applied = w
            .apply(Edit::MoveRegions {
                moves: vec![(ids[0], pos)],
            })
            .unwrap();
        assert!(!applied.changed);
        assert_eq!(w.revision(), rev, "a no-op must not bump the revision");
        let same = w.apply(Edit::SetMathSource {
            id: ids[0],
            source: "a := 1".into(),
        });
        assert!(!same.unwrap().changed);
    }

    #[test]
    fn revision_increases_only_when_something_changes() {
        let (mut w, ids) = sheet();
        let r0 = w.revision();
        w.apply(Edit::MoveRegions {
            moves: vec![(ids[0], Point::new(500.0, 500.0))],
        })
        .unwrap();
        assert_eq!(w.revision(), r0 + 1);
    }
}
