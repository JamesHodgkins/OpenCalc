//! Snapping to the edges of shapes while dragging: a dragged object's left, middle
//! and right (and top, middle and bottom) are pulled onto those of the shapes it is
//! not part of, when they come within a few pixels. (Other kinds of region are not
//! targets: they sit close together, and snapping to all of them would fight the grid.) All in
//! document space and pure, so it is tested without a window.

use eframe::egui::{Pos2, Rect};
use opencalc_core::document::{Region, RegionId};

use super::adapt::{rect_of, Sizes};

/// How near, in screen pixels, an edge must come to be pulled onto another.
pub const REACH_PX: f32 = 6.0;

/// The places an edge can snap to: x values for vertical edges and y values for
/// horizontal ones.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Targets {
    pub xs: Vec<f32>,
    pub ys: Vec<f32>,
}

/// A line to show where something snapped: at `at` along x if `vertical`, else y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub at: f32,
}

/// What snapping decided: how far to move on each axis (none: nothing near) and
/// the guides to show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
    pub dx: Option<f32>,
    pub dy: Option<f32>,
    pub guides: Vec<Guide>,
}

/// The edges and middles of every shape except those in `exclude`.
pub fn targets(regions: &[Region], exclude: &[RegionId], sizes: &Sizes) -> Targets {
    let mut t = Targets::default();
    let shapes = regions
        .iter()
        .filter(|r| matches!(r.kind, opencalc_core::document::RegionKind::Shape(_)));
    for r in shapes.filter(|r| !exclude.contains(&r.id)) {
        let rect = rect_of(r, sizes);
        t.xs.extend([rect.min.x, rect.center().x, rect.max.x]);
        t.ys.extend([rect.min.y, rect.center().y, rect.max.y]);
    }
    t
}

/// The smallest move that puts one of `values` on one of `targets`, if any is
/// within `reach`. Returns the move and the place it lands.
fn nearest(values: &[f32], targets: &[f32], reach: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for &v in values {
        for &t in targets {
            let d = t - v;
            if d.abs() <= reach && best.is_none_or(|(b, _)| d.abs() < b.abs()) {
                best = Some((d, t));
            }
        }
    }
    best
}

/// Snap the edges of `rect` (a box being moved).
pub fn snap_rect(rect: Rect, t: &Targets, reach: f32) -> Snap {
    let x = nearest(&[rect.min.x, rect.center().x, rect.max.x], &t.xs, reach);
    let y = nearest(&[rect.min.y, rect.center().y, rect.max.y], &t.ys, reach);
    Snap {
        dx: x.map(|(d, _)| d),
        dy: y.map(|(d, _)| d),
        guides: [
            x.map(|(_, at)| Guide { vertical: true, at }),
            y.map(|(_, at)| Guide {
                vertical: false,
                at,
            }),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

/// Snap a point being pulled, on the axes asked for.
pub fn snap_point(p: Pos2, t: &Targets, reach: f32, on_x: bool, on_y: bool) -> Snap {
    let x = on_x.then(|| nearest(&[p.x], &t.xs, reach)).flatten();
    let y = on_y.then(|| nearest(&[p.y], &t.ys, reach)).flatten();
    Snap {
        dx: x.map(|(d, _)| d),
        dy: y.map(|(d, _)| d),
        guides: [
            x.map(|(_, at)| Guide { vertical: true, at }),
            y.map(|(_, at)| Guide {
                vertical: false,
                at,
            }),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

/// `p` moved by whatever `snap` decided.
#[cfg(test)]
fn applied(p: Pos2, snap: &Snap) -> Pos2 {
    Pos2::new(p.x + snap.dx.unwrap_or(0.0), p.y + snap.dy.unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;
    use opencalc_core::document::{Point, RegionKind, Size};

    fn t() -> Targets {
        Targets {
            xs: vec![100.0, 200.0, 300.0],
            ys: vec![50.0, 90.0],
        }
    }

    #[test]
    fn an_edge_within_reach_is_pulled_on_and_the_nearest_wins() {
        let b = Rect::from_min_size(Pos2::new(103.0, 300.0), vec2(40.0, 20.0));
        let s = snap_rect(b, &t(), 6.0);
        assert_eq!(s.dx, Some(-3.0), "the left edge goes onto 100");
        assert_eq!(s.dy, None, "nothing near vertically");
        assert_eq!(
            s.guides,
            vec![Guide {
                vertical: true,
                at: 100.0
            }]
        );
        // Two candidates: the right edge 296 is 4 from 300; the left 103 is 3 from 100.
        let b = Rect::from_min_size(Pos2::new(103.0, 0.0), vec2(150.0, 10.0));
        assert_eq!(snap_rect(b, &t(), 6.0).dx, Some(-3.0));
    }

    #[test]
    fn the_middle_snaps_too_and_nothing_beyond_reach_does() {
        let b = Rect::from_center_size(Pos2::new(204.0, 92.0), vec2(30.0, 10.0));
        let s = snap_rect(b, &t(), 6.0);
        assert_eq!((s.dx, s.dy), (Some(-4.0), Some(-2.0)));
        assert_eq!(snap_rect(b, &t(), 1.0), Snap::default());
    }

    #[test]
    fn a_pulled_point_snaps_only_on_the_axes_asked_for() {
        let p = Pos2::new(102.0, 52.0);
        let both = snap_point(p, &t(), 6.0, true, true);
        assert_eq!(applied(p, &both), Pos2::new(100.0, 50.0));
        let only_x = snap_point(p, &t(), 6.0, true, false);
        assert_eq!(applied(p, &only_x), Pos2::new(100.0, 52.0));
        assert_eq!(snap_point(p, &t(), 6.0, false, false), Snap::default());
    }

    #[test]
    fn targets_come_from_every_other_shape_and_no_other_kind() {
        let mk = |id: u64, x: f32| Region {
            id: RegionId(id),
            pos: Point::new(x, 10.0),
            size: Size::new(40.0, 20.0),
            kind: RegionKind::shape(opencalc_core::document::ShapeKind::Rectangle),
            style: Default::default(),
            locked: false,
            border: false,
            runs: Vec::new(),
            label: String::new(),
            z: 0,
        };
        let mut table = mk(3, 500.0);
        table.kind = RegionKind::table();
        let regions = [mk(1, 0.0), mk(2, 100.0), table];
        let got = targets(&regions, &[RegionId(1)], &Sizes::new());
        assert_eq!(got.xs, vec![100.0, 120.0, 140.0]);
        assert_eq!(got.ys, vec![10.0, 20.0, 30.0]);
    }
}
