//! Lining regions up: in a row (the same top) or in a column (the same left edge).
//! Regions only move along one axis, so nothing is moved further than it must be.

use opencalc_core::document::{Point, Region, RegionId};

/// The new top-left of each selected region that has to move. Aligned in a `row`,
/// every region takes the top of the highest; in a column, the left edge of the
/// leftmost. A locked region counts for the target but is not moved. Fewer than
/// two regions selected: nothing moves.
pub fn align(regions: &[Region], selected: &[RegionId], row: bool) -> Vec<(RegionId, Point)> {
    let chosen: Vec<&Region> = regions
        .iter()
        .filter(|r| selected.contains(&r.id))
        .collect();
    if chosen.len() < 2 {
        return Vec::new();
    }
    let edge = |r: &Region| if row { r.pos.y } else { r.pos.x };
    let target = chosen.iter().map(|r| edge(r)).fold(f32::INFINITY, f32::min);
    chosen
        .into_iter()
        .filter(|r| edge(r) != target && !r.locked)
        .map(|r| {
            let pos = if row {
                Point::new(r.pos.x, target)
            } else {
                Point::new(target, r.pos.y)
            };
            (r.id, pos)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::{RegionKind, Size};

    fn at(id: u64, x: f32, y: f32) -> Region {
        Region {
            id: RegionId(id),
            pos: Point::new(x, y),
            size: Size::new(50.0, 30.0),
            kind: RegionKind::table(),
            style: Default::default(),
            locked: false,
            border: false,
            runs: Vec::new(),
            label: String::new(),
            z: 0,
        }
    }

    #[test]
    fn a_row_takes_the_highest_top_and_a_column_the_leftmost_edge() {
        let r = [at(1, 10.0, 100.0), at(2, 200.0, 40.0), at(3, 90.0, 70.0)];
        let all = [RegionId(1), RegionId(2), RegionId(3)];
        let mut row = align(&r, &all, true);
        row.sort_by_key(|m| m.0);
        assert_eq!(
            row,
            [
                (RegionId(1), Point::new(10.0, 40.0)),
                (RegionId(3), Point::new(90.0, 40.0))
            ],
            "the one already at the top stays; x never changes"
        );
        let mut column = align(&r, &all, false);
        column.sort_by_key(|m| m.0);
        assert_eq!(
            column,
            [
                (RegionId(2), Point::new(10.0, 40.0)),
                (RegionId(3), Point::new(10.0, 70.0))
            ]
        );
    }

    #[test]
    fn shapes_including_lines_align_by_their_box_like_any_region() {
        use opencalc_core::document::ShapeKind;
        let mut line = at(1, 10.0, 100.0);
        line.kind = RegionKind::shape(ShapeKind::Line);
        line.size = Size::new(80.0, 1.0);
        let mut frame = at(2, 200.0, 40.0);
        frame.kind = RegionKind::shape(ShapeKind::Rectangle);
        let both = [RegionId(1), RegionId(2)];
        assert_eq!(
            align(&[line.clone(), frame.clone()], &both, true),
            [(RegionId(1), Point::new(10.0, 40.0))]
        );
        assert_eq!(
            align(&[line, frame], &both, false),
            [(RegionId(2), Point::new(10.0, 40.0))]
        );
    }

    #[test]
    fn only_the_selected_regions_count_and_one_is_not_enough() {
        let r = [at(1, 10.0, 100.0), at(2, 200.0, 40.0), at(3, 90.0, 5.0)];
        assert_eq!(align(&r, &[RegionId(1), RegionId(2)], true).len(), 1);
        assert!(align(&r, &[RegionId(1)], true).is_empty());
        assert!(align(&r, &[], false).is_empty());
        assert!(align(&r, &[RegionId(9), RegionId(1)], true).is_empty());
        // Already lined up: nothing to do.
        let flat = [at(1, 10.0, 40.0), at(2, 200.0, 40.0)];
        assert!(align(&flat, &[RegionId(1), RegionId(2)], true).is_empty());
    }
}
