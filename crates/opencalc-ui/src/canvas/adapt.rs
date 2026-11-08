//! The boundary between the document (core types) and the canvas (egui types).
//! Core never names an egui type (rule R1), so conversions live here.

use std::collections::HashMap;

use eframe::egui::{Pos2, Rect, Vec2};
use opencalc_core::document::{Point, Region, RegionId, RegionKind, Size};

use crate::icons;

/// Content sizes measured by the last paint, per region, in document units.
///
/// This is UI state, not document content: it depends on fonts and zoom
/// rounding, so storing it in the document would make zooming dirty the file.
pub type Sizes = HashMap<RegionId, Vec2>;

pub fn pos(p: Point) -> Pos2 {
    Pos2::new(p.x, p.y)
}

pub fn point(p: Pos2) -> Point {
    Point::new(p.x, p.y)
}

pub fn vec(s: Size) -> Vec2 {
    Vec2::new(s.w, s.h)
}

/// A region's rectangle in document space: the measured size for regions sized by
/// their content (before the first paint, the stored starting size), the stored
/// size for the rest.
pub fn rect_of(r: &Region, sizes: &Sizes) -> Rect {
    let size = match sizes.get(&r.id) {
        Some(measured) if r.kind.is_auto_sized() => *measured,
        _ => vec(r.size),
    };
    Rect::from_min_size(pos(r.pos), size)
}

pub fn kind_icon(kind: &RegionKind) -> char {
    match kind {
        RegionKind::Math(_) => icons::MATH,
        RegionKind::Text { .. } => icons::TEXT,
        RegionKind::Area { .. } => icons::AREA,
        RegionKind::Plot(_) => icons::PLOT,
        RegionKind::Table(_) => icons::TABLE,
        RegionKind::Picture => icons::PICTURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(kind: RegionKind) -> Region {
        let size = kind.default_size();
        Region {
            id: RegionId(1),
            pos: Point::new(10.0, 20.0),
            size,
            kind,
            style: Default::default(),
            locked: false,
            label: String::new(),
        }
    }

    #[test]
    fn conversions_round_trip() {
        let p = Point::new(1.5, -2.5);
        assert_eq!(point(pos(p)), p);
        let s = Size::new(3.0, 4.0);
        assert_eq!(vec(s), Vec2::new(3.0, 4.0));
    }

    #[test]
    fn content_sized_regions_use_the_measured_size() {
        let r = region(RegionKind::math("x := 1"));
        let mut sizes = Sizes::new();
        assert_eq!(
            rect_of(&r, &sizes).size(),
            vec(r.size),
            "before the first paint"
        );
        sizes.insert(r.id, Vec2::new(77.0, 33.0));
        assert_eq!(rect_of(&r, &sizes).size(), Vec2::new(77.0, 33.0));
        assert_eq!(rect_of(&r, &sizes).min, Pos2::new(10.0, 20.0));
    }

    #[test]
    fn fixed_size_regions_ignore_measurements() {
        let r = region(RegionKind::plot());
        let mut sizes = Sizes::new();
        sizes.insert(r.id, Vec2::new(1.0, 1.0));
        assert_eq!(rect_of(&r, &sizes).size(), vec(r.size));
    }
}
