//! The worksheet model the shell edits. Regions are placed freely in document
//! space and are referred to by stable ID, never by list position.

use eframe::egui::{Pos2, Rect, Vec2};
use opencalc_core::eval::RegionId;
use opencalc_core::expr::Expr;
use opencalc_core::parser::parse;

use crate::canvas::{CanvasUi, CanvasView};
use crate::icons;

pub enum RegionKind {
    Math { source: String, expr: Expr },
    Text { text: String },
    Area { title: String },
    Plot,
    Table,
    Picture,
}

impl RegionKind {
    pub fn math(source: &str) -> RegionKind {
        RegionKind::Math {
            source: source.to_owned(),
            expr: parse(source),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RegionKind::Math { .. } => "Math",
            RegionKind::Text { .. } => "Text",
            RegionKind::Area { .. } => "Area",
            RegionKind::Plot => "Plot",
            RegionKind::Table => "Table",
            RegionKind::Picture => "Picture",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            RegionKind::Math { .. } => icons::MATH,
            RegionKind::Text { .. } => icons::TEXT,
            RegionKind::Area { .. } => icons::AREA,
            RegionKind::Plot => icons::PLOT,
            RegionKind::Table => icons::TABLE,
            RegionKind::Picture => icons::PICTURE,
        }
    }

    /// Starting size in document units. Math and text regions are re-measured
    /// from their content by the canvas.
    pub fn default_size(&self) -> Vec2 {
        match self {
            RegionKind::Math { .. } => Vec2::new(160.0, 32.0),
            RegionKind::Text { .. } => Vec2::new(240.0, 40.0),
            RegionKind::Area { .. } => Vec2::new(320.0, 200.0),
            RegionKind::Plot | RegionKind::Table | RegionKind::Picture => Vec2::new(260.0, 170.0),
        }
    }

    /// Whether the content size comes from typesetting rather than the stored size.
    pub fn is_auto_sized(&self) -> bool {
        matches!(self, RegionKind::Math { .. } | RegionKind::Text { .. })
    }

    /// Whether double-click edits the content in place.
    pub fn is_editable(&self) -> bool {
        matches!(self, RegionKind::Math { .. } | RegionKind::Text { .. })
    }
}

pub struct Region {
    pub id: RegionId,
    /// Top-left corner in document space.
    pub pos: Pos2,
    pub size: Vec2,
    pub kind: RegionKind,
}

impl Region {
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.pos, self.size)
    }
}

pub struct Sheet {
    pub title: String,
    /// Back to front. This is also the creation order, which decides which of
    /// several definitions of one name wins; it never changes when regions are
    /// moved or selected.
    pub regions: Vec<Region>,
    next_id: u64,
    pub view: CanvasView,
    /// Selection, editing and drag state. Not part of the document.
    pub state: CanvasUi,
}

impl Sheet {
    pub fn new(title: &str) -> Sheet {
        Sheet {
            title: title.to_owned(),
            regions: Vec::new(),
            next_id: 1,
            view: CanvasView::default(),
            state: CanvasUi::default(),
        }
    }

    /// A small worksheet in deliberately non-dependency order, spread over the page.
    pub fn sample() -> Sheet {
        let mut s = Sheet::new("Beam stress");
        s.add(
            RegionKind::Text {
                text: "Axial stress in a square bar".to_owned(),
            },
            Pos2::new(40.0, 20.0),
        );
        for (src, x, y) in [
            ("\u{03c3} := F / A", 40.0, 220.0),
            ("F := 10 kN", 40.0, 80.0),
            ("b := 100 mm", 40.0, 120.0),
            ("A := b^2", 400.0, 120.0),
            ("s := sqrt(A)", 400.0, 160.0),
            ("\u{03c3} =", 400.0, 220.0),
        ] {
            s.add(RegionKind::math(src), Pos2::new(x, y));
        }
        // New regions go below the worked example, not on top of it.
        s.state.insert_at = Pos2::new(40.0, 300.0);
        s
    }

    pub fn add(&mut self, kind: RegionKind, pos: Pos2) -> RegionId {
        let id = RegionId(self.next_id);
        self.next_id += 1;
        let size = kind.default_size();
        self.regions.push(Region {
            id,
            pos,
            size,
            kind,
        });
        id
    }

    pub fn region(&self, id: RegionId) -> Option<&Region> {
        self.regions.iter().find(|r| r.id == id)
    }

    pub fn region_mut(&mut self, id: RegionId) -> Option<&mut Region> {
        self.regions.iter_mut().find(|r| r.id == id)
    }

    /// Remove regions by ID. Returns how many were removed.
    pub fn remove(&mut self, ids: &[RegionId]) -> usize {
        let before = self.regions.len();
        self.regions.retain(|r| !ids.contains(&r.id));
        self.state.selection.retain(|id| !ids.contains(id));
        if self.state.editing.is_some_and(|e| ids.contains(&e)) {
            self.state.editing = None;
        }
        before - self.regions.len()
    }

    /// Math regions in layout order, for evaluation.
    pub fn snapshot(&self) -> Vec<(RegionId, Expr)> {
        self.regions
            .iter()
            .filter_map(|r| match &r.kind {
                RegionKind::Math { expr, .. } => Some((r.id, expr.clone())),
                _ => None,
            })
            .collect()
    }

    /// Bounding box of every region, in document space.
    pub fn bounds(&self) -> Option<Rect> {
        self.regions
            .iter()
            .map(Region::rect)
            .reduce(|a, b| a.union(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn math(s: &mut Sheet, src: &str) -> RegionId {
        s.add(RegionKind::math(src), Pos2::ZERO)
    }

    #[test]
    fn ids_are_stable_and_never_reused() {
        let mut s = Sheet::new("t");
        let a = math(&mut s, "a := 1");
        let b = math(&mut s, "b := 2");
        s.remove(&[a]);
        let c = math(&mut s, "c := 3");
        assert!(a != b && b != c && a != c);
    }

    #[test]
    fn snapshot_has_only_math_regions_in_creation_order() {
        let s = Sheet::sample();
        let ids: Vec<u64> = s.snapshot().iter().map(|(id, _)| id.0).collect();
        // Id 1 is the text title and is not evaluated.
        assert_eq!(ids, [2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn moving_a_region_does_not_change_evaluation_order() {
        let mut s = Sheet::sample();
        let before: Vec<_> = s.snapshot().iter().map(|(id, _)| *id).collect();
        if let Some(r) = s.region_mut(RegionId(2)) {
            r.pos = Pos2::new(900.0, 900.0);
        }
        let after: Vec<_> = s.snapshot().iter().map(|(id, _)| *id).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn remove_clears_selection_and_editing_for_those_ids() {
        let mut s = Sheet::new("t");
        let a = math(&mut s, "a := 1");
        let b = math(&mut s, "b := 2");
        s.state.selection = vec![a, b];
        s.state.editing = Some(a);
        assert_eq!(s.remove(&[a]), 1);
        assert_eq!(s.state.selection, vec![b]);
        assert_eq!(s.state.editing, None);
    }

    #[test]
    fn bounds_cover_every_region() {
        let mut s = Sheet::new("t");
        assert!(s.bounds().is_none());
        s.add(RegionKind::Plot, Pos2::new(0.0, 0.0));
        s.add(RegionKind::Table, Pos2::new(500.0, 300.0));
        let b = s.bounds().map(|b| (b.min, b.max));
        assert_eq!(b, Some((Pos2::ZERO, Pos2::new(760.0, 470.0))));
    }

    #[test]
    fn every_region_type_can_be_created() {
        let mut s = Sheet::new("t");
        for kind in [
            RegionKind::math("x := 1"),
            RegionKind::Text {
                text: String::new(),
            },
            RegionKind::Area {
                title: "Loads".into(),
            },
            RegionKind::Plot,
            RegionKind::Table,
            RegionKind::Picture,
        ] {
            let label = kind.label();
            let id = s.add(kind, Pos2::ZERO);
            assert_eq!(s.region(id).map(|r| r.kind.label()), Some(label));
        }
        assert_eq!(s.regions.len(), 6);
    }
}
