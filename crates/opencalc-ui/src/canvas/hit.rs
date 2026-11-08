//! Hit-testing, marquee selection and drag arithmetic (spec section 4.2).
//! All in document space, all pure, so tested without a window.

use eframe::egui::{Pos2, Rect, Vec2};
use opencalc_core::document::{Region, RegionId};

use super::adapt::{rect_of, Sizes};
use super::view::snap;

/// The topmost region containing `p`. Regions are stored back to front, so
/// search from the end.
pub fn hit_test(regions: &[Region], sizes: &Sizes, p: Pos2) -> Option<RegionId> {
    regions
        .iter()
        .rev()
        .find(|r| rect_of(r, sizes).contains(p))
        .map(|r| r.id)
}

/// Regions touched by a marquee rectangle, in back-to-front order.
pub fn in_marquee(regions: &[Region], sizes: &Sizes, marquee: Rect) -> Vec<RegionId> {
    regions
        .iter()
        .filter(|r| rect_of(r, sizes).intersects(marquee))
        .map(|r| r.id)
        .collect()
}

/// New positions for a dragged group. `origins` are the positions when the
/// drag began and `total` is the whole drag so far, so the result does not
/// accumulate error. With a grid, the first region snaps and the rest keep their
/// offsets from it, so a dragged group keeps its shape.
pub fn drag_positions(
    origins: &[(RegionId, Pos2)],
    total: Vec2,
    grid: Option<f32>,
) -> Vec<(RegionId, Pos2)> {
    let Some(&(_, lead)) = origins.first() else {
        return Vec::new();
    };
    let delta = match grid {
        Some(g) => snap(lead + total, g) - lead,
        None => total,
    };
    origins.iter().map(|&(id, p)| (id, p + delta)).collect()
}

/// Apply a click to a selection. `extend` (shift or ctrl) toggles the hit
/// region; otherwise the hit region replaces the selection. A miss clears it
/// unless extending.
pub fn click_selection(current: &[RegionId], hit: Option<RegionId>, extend: bool) -> Vec<RegionId> {
    match (hit, extend) {
        (Some(id), false) => vec![id],
        (Some(id), true) => {
            let mut v: Vec<RegionId> = current.to_vec();
            match v.iter().position(|x| *x == id) {
                Some(i) => {
                    v.remove(i);
                }
                None => v.push(id),
            }
            v
        }
        (None, false) => Vec::new(),
        (None, true) => current.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencalc_core::document::{Point, RegionKind};

    fn region(id: u64, x: f32, y: f32, kind: RegionKind) -> Region {
        let size = kind.default_size();
        Region {
            id: RegionId(id),
            pos: Point::new(x, y),
            size,
            kind,
            style: Default::default(),
            locked: false,
            label: String::new(),
        }
    }

    /// Plot and Table are 260x170. `top` overlaps the lower right of `back`.
    fn regions() -> (Vec<Region>, RegionId, RegionId, RegionId) {
        let v = vec![
            region(1, 0.0, 0.0, RegionKind::plot()),
            region(2, 200.0, 100.0, RegionKind::table()),
            region(3, 1000.0, 1000.0, RegionKind::Picture),
        ];
        (v, RegionId(1), RegionId(2), RegionId(3))
    }

    #[test]
    fn click_on_overlap_picks_the_topmost() {
        let (s, back, top, _) = regions();
        let sizes = Sizes::new();
        assert_eq!(hit_test(&s, &sizes, Pos2::new(230.0, 130.0)), Some(top));
        assert_eq!(hit_test(&s, &sizes, Pos2::new(50.0, 50.0)), Some(back));
    }

    #[test]
    fn click_on_empty_space_hits_nothing() {
        let (s, ..) = regions();
        let sizes = Sizes::new();
        assert_eq!(hit_test(&s, &sizes, Pos2::new(700.0, 700.0)), None);
        assert_eq!(hit_test(&[], &sizes, Pos2::ZERO), None);
    }

    #[test]
    fn hit_test_includes_the_edge() {
        let (s, back, ..) = regions();
        let sizes = Sizes::new();
        assert_eq!(hit_test(&s, &sizes, Pos2::new(0.0, 0.0)), Some(back));
        // (260, 50) is on the right edge and clear of the overlapping table.
        assert_eq!(hit_test(&s, &sizes, Pos2::new(260.0, 50.0)), Some(back));
        assert_eq!(hit_test(&s, &sizes, Pos2::new(260.5, 50.0)), None);
        assert_eq!(hit_test(&s, &sizes, Pos2::new(-0.5, 10.0)), None);
    }

    #[test]
    fn content_sized_regions_are_hit_by_their_measured_size() {
        let math = region(1, 0.0, 0.0, RegionKind::math("x := 1"));
        let id = math.id;
        let mut sizes = Sizes::new();
        // Default 160 wide: a point at x = 300 misses until a wider size is measured.
        assert_eq!(
            hit_test(std::slice::from_ref(&math), &sizes, Pos2::new(300.0, 10.0)),
            None
        );
        sizes.insert(id, Vec2::new(400.0, 32.0));
        assert_eq!(
            hit_test(std::slice::from_ref(&math), &sizes, Pos2::new(300.0, 10.0)),
            Some(id)
        );
    }

    #[test]
    fn marquee_selects_everything_it_touches() {
        let (s, back, top, far) = regions();
        let sizes = Sizes::new();
        let m = Rect::from_min_max(Pos2::new(250.0, 150.0), Pos2::new(300.0, 200.0));
        assert_eq!(in_marquee(&s, &sizes, m), vec![back, top]);
        let all = Rect::from_min_max(Pos2::new(-10.0, -10.0), Pos2::new(2000.0, 2000.0));
        assert_eq!(in_marquee(&s, &sizes, all), vec![back, top, far]);
        let none = Rect::from_min_max(Pos2::new(600.0, 600.0), Pos2::new(700.0, 700.0));
        assert!(in_marquee(&s, &sizes, none).is_empty());
    }

    #[test]
    fn drag_without_grid_moves_by_the_exact_amount() {
        let a = (RegionId(1), Pos2::new(10.0, 10.0));
        let b = (RegionId(2), Pos2::new(50.0, 90.0));
        let out = drag_positions(&[a, b], Vec2::new(7.5, -3.25), None);
        assert_eq!(out[0].1, Pos2::new(17.5, 6.75));
        assert_eq!(out[1].1, Pos2::new(57.5, 86.75));
    }

    #[test]
    fn drag_with_grid_snaps_the_lead_and_keeps_group_offsets() {
        let a = (RegionId(1), Pos2::new(13.0, 13.0));
        let b = (RegionId(2), Pos2::new(53.0, 93.0));
        let out = drag_positions(&[a, b], Vec2::new(9.0, 9.0), Some(20.0));
        assert_eq!(out[0].1, Pos2::new(20.0, 20.0));
        assert_eq!(out[1].1 - out[0].1, b.1 - a.1);
    }

    #[test]
    fn drag_is_computed_from_the_start_so_error_does_not_accumulate() {
        let a = (RegionId(1), Pos2::new(0.0, 0.0));
        let mut total = Vec2::ZERO;
        for _ in 0..1000 {
            total += Vec2::new(0.1, 0.1);
        }
        let stepped = drag_positions(&[a], total, Some(20.0));
        let direct = drag_positions(&[a], Vec2::new(100.0, 100.0), Some(20.0));
        assert_eq!(stepped[0].1, direct[0].1);
    }

    #[test]
    fn empty_drag_is_empty() {
        assert!(drag_positions(&[], Vec2::new(5.0, 5.0), None).is_empty());
    }

    #[test]
    fn click_selection_rules() {
        let (a, b) = (RegionId(1), RegionId(2));
        assert_eq!(click_selection(&[a], Some(b), false), vec![b]);
        assert_eq!(click_selection(&[a], Some(b), true), vec![a, b]);
        assert_eq!(click_selection(&[a, b], Some(a), true), vec![b]);
        assert!(click_selection(&[a, b], None, false).is_empty());
        assert_eq!(click_selection(&[a, b], None, true), vec![a, b]);
    }
}
