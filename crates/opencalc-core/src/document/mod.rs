//! The document model: what is saved. Pure data, no UI types (rules R1, R2).
//!
//! A [`Worksheet`] is changed only through [`crate::edit::Edit`]s, so undo,
//! autosave and the file format share one choke point (rule R3).

mod geometry;
mod page;
mod picture;
mod plot;
mod plot_style;
mod region;
mod result_format;
mod table;

use serde::{Deserialize, Serialize};

pub use geometry::{Point, Size};
pub use page::{PageSetup, PaperSize, MARGIN_RANGE_MM, UNITS_PER_MM};
pub use picture::{
    PictureSpec, MAX_PICTURES, MAX_PICTURE_BYTES, MAX_PICTURE_PIXELS, MAX_PICTURE_SIDE,
    MAX_PICTURE_TEXT,
};
pub use plot::{PlotInput, PlotSeriesInput, PlotSpec, MAX_SERIES};
pub use plot_style::{
    LegendMode, Marker, PlotKind, PlotOptions, SeriesKind, SeriesStyle, YAxis, PALETTE_SIZE,
};
pub use region::{
    clean_label, FontStyle, MathRegion, Region, RegionId, RegionKind, MAX_LABEL_CHARS,
};
pub use result_format::{DisplayUnit, Notation, ResultFormat, MAX_DECIMALS};
pub use table::{TableColumn, TableSpec, MAX_CELLS, MAX_COLUMNS, MAX_TEXT as MAX_TABLE_TEXT};

use crate::expr::Expr;
use crate::registry::{RegistryError, UnitRegistry, UnitSystem};
use crate::units::UnitDef;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Worksheet {
    pub(crate) title: String,
    /// Back to front. This is also creation order, which decides which of several
    /// definitions of one name wins; moving a region never changes it.
    pub(crate) regions: Vec<Region>,
    /// Next region ID to hand out. Only ever grows, so an ID is never reused even
    /// after the region is deleted and the deletion undone.
    pub(crate) next_id: u64,
    /// Which units results are displayed in. Display only; values are SI.
    #[serde(default)]
    pub(crate) unit_system: UnitSystem,
    /// Units defined by this worksheet, beyond the built-in ones.
    #[serde(default)]
    pub(crate) custom_units: Vec<UnitDef>,
    /// Paper size, orientation and margins the regions are laid out on.
    #[serde(default)]
    pub(crate) page: PageSetup,
    /// Bumped by every applied edit. Not saved; lets caches notice change.
    #[serde(skip)]
    pub(crate) revision: u64,
    /// Bumped only by edits that can change what the evaluator computes (math
    /// source, or adding or removing a math region). Moving a region does not.
    #[serde(skip)]
    pub(crate) eval_revision: u64,
}

/// Two worksheets are equal if they have the same content. `next_id` is
/// allocation state and `revision` is bookkeeping, so neither counts.
impl PartialEq for Worksheet {
    fn eq(&self, other: &Worksheet) -> bool {
        self.title == other.title
            && self.regions == other.regions
            && self.unit_system == other.unit_system
            && self.page == other.page
            && self.custom_units == other.custom_units
    }
}

impl Worksheet {
    pub fn new(title: &str) -> Worksheet {
        Worksheet {
            title: title.to_owned(),
            regions: Vec::new(),
            next_id: 1,
            unit_system: UnitSystem::default(),
            custom_units: Vec::new(),
            page: PageSetup::default(),
            revision: 0,
            eval_revision: 0,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn page(&self) -> PageSetup {
        self.page
    }

    pub fn unit_system(&self) -> UnitSystem {
        self.unit_system
    }

    pub fn custom_units(&self) -> &[UnitDef] {
        &self.custom_units
    }

    /// The units this worksheet can use. Edits only accept valid definitions, so
    /// this fails only for a file that was edited by hand; loading must check it.
    pub fn units(&self) -> Result<UnitRegistry, RegistryError> {
        UnitRegistry::new(self.custom_units.clone())
    }

    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    pub fn region(&self, id: RegionId) -> Option<&Region> {
        self.regions.iter().find(|r| r.id == id)
    }

    pub fn index_of(&self, id: RegionId) -> Option<usize> {
        self.regions.iter().position(|r| r.id == id)
    }

    /// Increases with every applied edit.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Increases only when an edit could change evaluation results. The UI
    /// recalculates when this changes, not on every move or resize.
    pub fn eval_revision(&self) -> u64 {
        self.eval_revision
    }

    /// Make a region with a fresh ID, not yet part of the worksheet. Add it with
    /// [`crate::edit::Edit::InsertRegions`].
    pub fn new_region(&mut self, kind: RegionKind, pos: Point) -> Region {
        let id = RegionId(self.next_id);
        self.next_id += 1;
        let size = kind.default_size();
        Region {
            id,
            pos,
            size,
            kind,
            style: FontStyle::default(),
            locked: false,
            border: false,
            label: String::new(),
        }
    }

    /// Plot regions in layout order, as the evaluator takes them: the parsed x
    /// and series expressions of each. Blank series are left out.
    pub fn plot_inputs(&self) -> Vec<(RegionId, PlotInput)> {
        self.regions
            .iter()
            .filter_map(|r| match &r.kind {
                RegionKind::Plot(p) => Some((
                    r.id,
                    PlotInput {
                        x: p.x.expr().clone(),
                        series: p
                            .series
                            .iter()
                            .enumerate()
                            .filter(|(_, s)| !s.source().trim().is_empty())
                            .map(|(k, s)| {
                                let style = p.style(k);
                                PlotSeriesInput {
                                    y: s.expr().clone(),
                                    x: (!style.x.trim().is_empty())
                                        .then(|| MathRegion::new(&style.x).expr().clone()),
                                    right: style.axis == YAxis::Right,
                                }
                            })
                            .collect(),
                        x_blank: p.x.source().trim().is_empty(),
                        polar: p.options.kind == PlotKind::Polar,
                        x_log: p.options.x_log,
                        y_log: p.options.y_log,
                        y2_log: p.options.y2_log,
                        limits: p
                            .options
                            .limits()
                            .map(|m| (!m.source().trim().is_empty()).then(|| m.expr().clone())),
                    },
                )),
                _ => None,
            })
            .collect()
    }

    /// Math regions in layout order, as the evaluator takes them. A table that
    /// has a name and numbers is one too: it defines its name as a matrix.
    pub fn math_regions(&self) -> Vec<(RegionId, Expr)> {
        self.regions
            .iter()
            .filter_map(|r| match &r.kind {
                RegionKind::Math(m) => Some((r.id, m.expr().clone())),
                RegionKind::Table(t) => t.expr().map(|e| (r.id, e)),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_regions_get_increasing_ids_even_if_never_inserted() {
        let mut w = Worksheet::new("t");
        let a = w.new_region(RegionKind::plot(), Point::default());
        let b = w.new_region(RegionKind::table(), Point::default());
        assert_eq!((a.id, b.id), (RegionId(1), RegionId(2)));
        assert!(w.regions().is_empty(), "allocating an ID does not add it");
    }

    #[test]
    fn math_region_expression_always_matches_its_source() {
        let mut m = MathRegion::new("x := 1 + 2");
        assert_eq!(m.expr(), &crate::parser::parse("x := 1 + 2"));
        m.set_source("y := 3".to_owned());
        assert_eq!(m.source(), "y := 3");
        assert_eq!(m.expr(), &crate::parser::parse("y := 3"));
    }

    #[test]
    fn worksheets_compare_by_content_not_bookkeeping() {
        let mut a = Worksheet::new("t");
        let b = Worksheet::new("t");
        let _ = a.new_region(RegionKind::plot(), Point::default()); // bumps next_id only
        a.revision = 9;
        assert_eq!(a, b);
        assert_ne!(Worksheet::new("other"), b);
    }

    #[test]
    fn only_math_regions_are_evaluated() {
        let mut w = Worksheet::new("t");
        for kind in [
            RegionKind::plot(),
            RegionKind::math("a := 1"),
            RegionKind::Text { text: "x".into() },
            RegionKind::math("b := 2"),
        ] {
            let r = w.new_region(kind, Point::default());
            w.regions.push(r);
        }
        let ids: Vec<u64> = w.math_regions().iter().map(|(id, _)| id.0).collect();
        assert_eq!(ids, [2, 4]);
    }

    fn sample() -> Worksheet {
        let mut w = Worksheet::new("Beam stress");
        for (kind, x) in [
            (RegionKind::math("F := 10 kN"), 40.0),
            (
                RegionKind::Text {
                    text: "Notes \u{3c3}".into(),
                },
                80.0,
            ),
            (
                RegionKind::Area {
                    title: "Loads".into(),
                },
                120.0,
            ),
            (RegionKind::plot(), 160.0),
            (RegionKind::table(), 200.0),
            (RegionKind::picture(), 240.0),
        ] {
            let r = w.new_region(kind, Point::new(x, 20.0));
            w.regions.push(r);
        }
        w
    }

    #[test]
    fn saves_math_as_source_text_and_not_as_an_expression_tree() {
        let json = serde_json::to_string(&sample()).unwrap();
        assert!(json.contains("F := 10 kN"), "{json}");
        // Decision D5: the AST is not part of the file format.
        assert!(
            !json.contains("Quantity"),
            "AST leaked into the file: {json}"
        );
        assert!(!json.contains("Assign"), "AST leaked into the file: {json}");
        assert!(
            !json.contains("revision"),
            "bookkeeping leaked into the file"
        );
    }

    #[test]
    fn every_region_type_round_trips_through_json() {
        let original = sample();
        let json = serde_json::to_string(&original).unwrap();
        let back: Worksheet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, original);
        // The expression was re-derived from the source on load.
        let f = back.regions().first().and_then(|r| match &r.kind {
            RegionKind::Math(m) => Some(m.expr().clone()),
            _ => None,
        });
        assert_eq!(f, Some(crate::parser::parse("F := 10 kN")));
        // IDs keep counting from where they were, so none is ever reused.
        let mut back = back;
        assert_eq!(
            back.new_region(RegionKind::plot(), Point::default()).id,
            RegionId(7)
        );
    }

    #[test]
    fn loading_applies_the_parser_limits_to_hostile_source() {
        let long = "(".repeat(100_000);
        let json = format!(
            r#"{{"title":"t","regions":[{{"id":1,"pos":{{"x":0,"y":0}},"size":{{"w":1,"h":1}},"kind":{{"Math":{{"source":"{long}"}}}}}}],"next_id":2}}"#
        );
        let w: Worksheet = serde_json::from_str(&json).unwrap();
        let expr = match &w.regions()[0].kind {
            RegionKind::Math(m) => m.expr().clone(),
            _ => panic!("expected a math region"),
        };
        assert!(matches!(expr, Expr::Error(_)));
    }

    #[test]
    fn unit_settings_are_saved_and_old_files_without_them_still_load() {
        let mut w = sample();
        w.unit_system = UnitSystem::Uscs;
        let back: Worksheet = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(back.unit_system(), UnitSystem::Uscs);
        // A file from before unit settings existed has neither field.
        let mut value: serde_json::Value = serde_json::to_value(sample()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("unit_system");
        object.remove("custom_units");
        let old: Worksheet = serde_json::from_value(value).unwrap();
        assert_eq!(old.unit_system(), UnitSystem::Si);
        assert!(old.custom_units().is_empty());
    }

    #[test]
    fn page_setup_is_saved_and_old_files_get_a4_portrait() {
        let mut w = sample();
        w.page = PageSetup {
            paper: PaperSize::Letter,
            landscape: true,
            margin_mm: 12.5,
        };
        let back: Worksheet = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(back.page(), w.page());
        let mut value: serde_json::Value = serde_json::to_value(sample()).unwrap();
        value.as_object_mut().unwrap().remove("page");
        let old: Worksheet = serde_json::from_value(value).unwrap();
        assert_eq!(old.page(), PageSetup::default());
    }
}
