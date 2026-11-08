//! Moving regions apart: "Separate regions" removes every overlap by pushing the
//! lower region of each overlapping pair down until it clears the one above.
//! Regions only ever move down, so their order on the page is kept.

use eframe::egui::Rect;
use opencalc_core::document::{Point, Region, RegionId};

use super::adapt::{rect_of, Sizes};
use super::view::GRID;

/// Space left between regions that were pushed apart, in document units.
pub const GAP: f32 = 12.0;
/// More regions than this are not separated (the work grows with the square).
pub const MAX_REGIONS: usize = 5_000;

fn overlap(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

/// The new top-left of each region that has to move, or `None` if there are too
/// many regions. A region is moved onto the grid, so the result stays tidy.
pub fn separate(regions: &[Region], sizes: &Sizes) -> Option<Vec<(RegionId, Point)>> {
    if regions.len() > MAX_REGIONS {
        return None;
    }
    let mut order: Vec<(&Region, Rect)> = regions.iter().map(|r| (r, rect_of(r, sizes))).collect();
    order.sort_by(|a, b| {
        (a.1.min.y, a.1.min.x, a.0.id)
            .partial_cmp(&(b.1.min.y, b.1.min.x, b.0.id))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut placed: Vec<Rect> = Vec::with_capacity(order.len());
    let mut moves = Vec::new();
    for (region, original) in order {
        let mut rect = original;
        // Each push moves it strictly down, past one more region, so this ends.
        while let Some(above) = placed.iter().find(|p| overlap(**p, rect)) {
            let top = ((above.max.y + GAP) / GRID).ceil() * GRID;
            rect = rect.translate(eframe::egui::vec2(0.0, top - rect.min.y));
        }
        if rect != original {
            moves.push((
                region.id,
                Point::new(region.pos.x, region.pos.y + (rect.min.y - original.min.y)),
            ));
        }
        placed.push(rect);
    }
    Some(moves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::{Region, RegionKind, Size};

    fn at(id: u64, x: f32, y: f32, w: f32, h: f32) -> Region {
        Region {
            id: RegionId(id),
            pos: Point::new(x, y),
            size: Size::new(w, h),
            kind: RegionKind::table(),
            style: Default::default(),
            locked: false,
            label: String::new(),
        }
    }

    fn run(regions: &[Region]) -> Vec<(u64, f32)> {
        separate(regions, &Sizes::new())
            .unwrap()
            .into_iter()
            .map(|(id, p)| (id.0, p.y))
            .collect()
    }

    #[test]
    fn regions_that_do_not_overlap_stay_where_they_are() {
        let a = at(1, 0.0, 0.0, 100.0, 50.0);
        let b = at(2, 0.0, 50.0, 100.0, 50.0); // touching is not overlapping
        let c = at(3, 200.0, 10.0, 100.0, 50.0);
        assert!(run(&[a, b, c]).is_empty());
    }

    #[test]
    fn the_lower_region_is_pushed_below_the_upper_one_onto_the_grid() {
        let a = at(1, 0.0, 0.0, 100.0, 50.0);
        let b = at(2, 50.0, 20.0, 100.0, 50.0);
        // Clear of a's bottom (50) plus the gap (12), rounded up to the grid (12): 72.
        assert_eq!(run(&[a, b]), [(2, 72.0)]);
    }

    #[test]
    fn it_does_not_matter_which_was_created_first_only_which_is_higher() {
        let a = at(1, 0.0, 0.0, 100.0, 50.0);
        let b = at(2, 50.0, 20.0, 100.0, 50.0);
        assert_eq!(run(&[b.clone(), a.clone()]), [(2, 72.0)]);
    }

    #[test]
    fn a_pile_is_stacked_without_any_overlap_left() {
        let pile: Vec<Region> = (0..6)
            .map(|i| at(i + 1, 10.0, 10.0 + i as f32, 80.0, 30.0))
            .collect();
        let moves = separate(&pile, &Sizes::new()).unwrap();
        let mut after = pile.clone();
        for (id, p) in &moves {
            after.iter_mut().find(|r| r.id == *id).unwrap().pos = *p;
        }
        for (i, a) in after.iter().enumerate() {
            for b in &after[i + 1..] {
                assert!(
                    !overlap(rect_of(a, &Sizes::new()), rect_of(b, &Sizes::new())),
                    "{a:?} {b:?}"
                );
            }
        }
        // The top one did not move, and none moved sideways or up.
        assert!(moves.iter().all(|(id, _)| id.0 != 1));
        assert!(moves
            .iter()
            .all(|(id, p)| p.y > pile[id.0 as usize - 1].pos.y));
    }

    #[test]
    fn a_push_can_chain_past_several_regions() {
        let a = at(1, 0.0, 0.0, 100.0, 100.0);
        let b = at(2, 0.0, 130.0, 100.0, 100.0); // clear of a, but in the way of what a pushes
        let c = at(3, 0.0, 50.0, 100.0, 100.0); // overlaps both a and b's place
        let moves = run(&[a, b, c]);
        // c is higher than b, so it goes first: below a (100 + 12, on the grid: 120).
        // That puts it on b, which is lower and so is the one pushed: below c (220 + 12 -> 240).
        assert_eq!(moves, [(3, 120.0), (2, 240.0)]);
    }

    #[test]
    fn too_many_regions_are_left_alone() {
        let many: Vec<Region> = (0..=MAX_REGIONS as u64)
            .map(|i| at(i + 1, 0.0, 0.0, 1.0, 1.0))
            .collect();
        assert!(separate(&many, &Sizes::new()).is_none());
    }
}
