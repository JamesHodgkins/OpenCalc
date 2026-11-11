//! Draw order: which region is painted over which.
//!
//! Each region has a `z` number; a higher one is painted later, so it lies on top.
//! Regions with the same `z` keep their list order. The list order itself never
//! changes when something is moved forward or back: it is creation order, which
//! decides which of several definitions of one name wins, so reordering the
//! drawing must not touch the calculation.

use super::{RegionId, Worksheet};

/// A change of draw order for the chosen regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrange {
    /// Above everything.
    ToFront,
    /// Below everything.
    ToBack,
    /// Above the next region that overlaps it.
    Forward,
    /// Below the next region that overlaps it.
    Backward,
}

impl Worksheet {
    /// Region IDs from the back of the page to the front: by `z`, then list order.
    pub fn paint_order(&self) -> Vec<RegionId> {
        let mut order: Vec<(i32, usize, RegionId)> = self
            .regions
            .iter()
            .enumerate()
            .map(|(i, r)| (r.z, i, r.id))
            .collect();
        order.sort_by_key(|&(z, i, _)| (z, i));
        order.into_iter().map(|(.., id)| id).collect()
    }

    /// The `z` a new region needs to land on top of everything.
    pub fn top_z(&self) -> i32 {
        self.regions.iter().map(|r| r.z).max().unwrap_or(0)
    }

    /// The new `z` values for an [`Arrange`] of `chosen`, as `(id, z)` pairs for
    /// `Edit::SetZ`; regions that keep their `z` are left out. `overlaps` says
    /// whether two regions' boxes overlap (core does not know the measured size of
    /// a typeset region, so the caller answers). Empty when nothing would move.
    pub fn arrange(
        &self,
        chosen: &[RegionId],
        how: Arrange,
        overlaps: &dyn Fn(RegionId, RegionId) -> bool,
    ) -> Vec<(RegionId, i32)> {
        let before = self.paint_order();
        let mut order = before.clone();
        let picked = |id: &RegionId| chosen.contains(id);
        match how {
            Arrange::ToFront => order.sort_by_key(picked), // stable: false before true
            Arrange::ToBack => order.sort_by_key(|id| !picked(id)),
            Arrange::Forward => {
                // From the front, so a group moves together and one that is
                // already in front stays put.
                for p in (0..order.len()).rev() {
                    if !picked(&order[p]) {
                        continue;
                    }
                    let id = order[p];
                    let over = (p + 1..order.len())
                        .find(|&q| !picked(&order[q]) && overlaps(id, order[q]));
                    if let Some(q) = over {
                        order.remove(p);
                        order.insert(q, id);
                    }
                }
            }
            Arrange::Backward => {
                for p in 0..order.len() {
                    if !picked(&order[p]) {
                        continue;
                    }
                    let id = order[p];
                    let under = (0..p)
                        .rev()
                        .find(|&q| !picked(&order[q]) && overlaps(id, order[q]));
                    if let Some(q) = under {
                        order.remove(p);
                        order.insert(q, id);
                    }
                }
            }
        }
        if order == before {
            return Vec::new();
        }
        order
            .iter()
            .enumerate()
            .filter_map(|(rank, id)| {
                let z = i32::try_from(rank).unwrap_or(i32::MAX);
                (self.region(*id)?.z != z).then_some((*id, z))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Point, RegionKind};
    use crate::edit::Edit;

    /// Four plain regions A, B, C, D in creation order.
    fn four() -> (Worksheet, Vec<RegionId>) {
        let mut w = Worksheet::new("t");
        let ids: Vec<RegionId> = (0..4)
            .map(|i| {
                let r = w.new_region(
                    RegionKind::Text {
                        text: String::new(),
                    },
                    Point::new(i as f32, 0.0),
                );
                let id = r.id;
                w.apply(Edit::InsertRegions {
                    regions: vec![(i, r)],
                })
                .unwrap();
                id
            })
            .collect();
        (w, ids)
    }

    fn run(w: &mut Worksheet, chosen: &[RegionId], how: Arrange) {
        let moves = w.arrange(chosen, how, &|_, _| true);
        if !moves.is_empty() {
            w.apply(Edit::SetZ { moves }).unwrap();
        }
    }

    #[test]
    fn new_regions_paint_in_creation_order() {
        let (w, ids) = four();
        assert_eq!(w.paint_order(), ids);
    }

    #[test]
    fn to_front_and_to_back_keep_the_chosen_regions_in_their_own_order() {
        let (mut w, ids) = four();
        run(&mut w, &[ids[0], ids[1]], Arrange::ToFront);
        assert_eq!(w.paint_order(), [ids[2], ids[3], ids[0], ids[1]]);
        run(&mut w, &[ids[3], ids[2]], Arrange::ToBack);
        assert_eq!(w.paint_order(), [ids[2], ids[3], ids[0], ids[1]]);
        run(&mut w, &[ids[1]], Arrange::ToBack);
        assert_eq!(w.paint_order(), [ids[1], ids[2], ids[3], ids[0]]);
    }

    #[test]
    fn forward_and_backward_step_over_one_overlapping_region() {
        let (mut w, ids) = four();
        run(&mut w, &[ids[1]], Arrange::Forward);
        assert_eq!(w.paint_order(), [ids[0], ids[2], ids[1], ids[3]]);
        run(&mut w, &[ids[1]], Arrange::Backward);
        assert_eq!(w.paint_order(), ids);
    }

    #[test]
    fn a_step_skips_regions_it_does_not_overlap() {
        let (w, ids) = four();
        let only_d = |a: RegionId, b: RegionId| a == ids[0] && b == ids[3];
        let moves = w.arrange(&[ids[0]], Arrange::Forward, &only_d);
        let mut w = w;
        w.apply(Edit::SetZ { moves }).unwrap();
        assert_eq!(w.paint_order(), [ids[1], ids[2], ids[3], ids[0]]);
    }

    #[test]
    fn already_at_the_end_changes_nothing() {
        let (w, ids) = four();
        assert!(w
            .arrange(&[ids[3]], Arrange::Forward, &|_, _| true)
            .is_empty());
        assert!(w
            .arrange(&[ids[0]], Arrange::Backward, &|_, _| true)
            .is_empty());
        assert!(w
            .arrange(&[ids[3]], Arrange::ToFront, &|_, _| true)
            .is_empty());
    }

    #[test]
    fn reordering_never_changes_the_list_and_undo_restores_the_order() {
        let (mut w, ids) = four();
        let before = w.clone();
        let moves = w.arrange(&[ids[0]], Arrange::ToFront, &|_, _| true);
        let applied = w.apply(Edit::SetZ { moves }).unwrap();
        assert!(applied.changed);
        let list: Vec<RegionId> = w.regions().iter().map(|r| r.id).collect();
        assert_eq!(list, ids, "calculation order is untouched");
        assert_eq!(w.paint_order()[3], ids[0]);
        w.apply(applied.inverse).unwrap();
        assert_eq!(w, before);
    }

    #[test]
    fn a_new_region_lands_on_top_after_a_reorder() {
        let (mut w, ids) = four();
        run(&mut w, &[ids[0]], Arrange::ToFront);
        let r = w.new_region(
            RegionKind::Text {
                text: String::new(),
            },
            Point::default(),
        );
        let id = r.id;
        w.apply(Edit::InsertRegions {
            regions: vec![(4, r)],
        })
        .unwrap();
        assert_eq!(*w.paint_order().last().unwrap(), id);
    }
}
