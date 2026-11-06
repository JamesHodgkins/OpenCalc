//! Regions: the things placed on a worksheet.

use serde::{Deserialize, Serialize};

use super::geometry::{Point, Size};
use crate::expr::Expr;
use crate::parser::parse;

/// Stable identity of a region within one worksheet. Never a list position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RegionId(pub u64);

/// A Math region: typed source text and the expression parsed from it.
///
/// The **source text is canonical** (decision D5 in `docs/ARCHITECTURE.md`): it is
/// what is saved, and the expression is always derived from it. The two fields are
/// private and change together, so they can never disagree. This keeps the file
/// format independent of the AST until the structured math editor exists.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "MathSource", into = "MathSource")]
pub struct MathRegion {
    source: String,
    expr: Expr,
}

/// The saved form of a [`MathRegion`]: the source text only.
#[derive(Serialize, Deserialize)]
struct MathSource {
    source: String,
}

impl From<MathSource> for MathRegion {
    fn from(s: MathSource) -> MathRegion {
        MathRegion::new(&s.source)
    }
}

impl From<MathRegion> for MathSource {
    fn from(m: MathRegion) -> MathSource {
        MathSource { source: m.source }
    }
}

impl MathRegion {
    /// Parse `source`. Never fails: a parse error becomes `Expr::Error`.
    pub fn new(source: &str) -> MathRegion {
        MathRegion {
            source: source.to_owned(),
            expr: parse(source),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn expr(&self) -> &Expr {
        &self.expr
    }

    pub(crate) fn set_source(&mut self, source: String) {
        self.expr = parse(&source);
        self.source = source;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RegionKind {
    Math(MathRegion),
    Text { text: String },
    Area { title: String },
    Plot,
    Table,
    Picture,
}

impl RegionKind {
    pub fn math(source: &str) -> RegionKind {
        RegionKind::Math(MathRegion::new(source))
    }

    /// Short name of the region type, for display and diagnostics.
    pub fn label(&self) -> &'static str {
        match self {
            RegionKind::Math(_) => "Math",
            RegionKind::Text { .. } => "Text",
            RegionKind::Area { .. } => "Area",
            RegionKind::Plot => "Plot",
            RegionKind::Table => "Table",
            RegionKind::Picture => "Picture",
        }
    }

    /// Starting size in document units.
    pub fn default_size(&self) -> Size {
        match self {
            RegionKind::Math(_) => Size::new(160.0, 32.0),
            RegionKind::Text { .. } => Size::new(240.0, 40.0),
            RegionKind::Area { .. } => Size::new(320.0, 200.0),
            RegionKind::Plot | RegionKind::Table | RegionKind::Picture => Size::new(260.0, 170.0),
        }
    }

    /// Whether the region's content size comes from typesetting rather than the
    /// stored size. For these the stored size is only a starting guess, and the UI
    /// measures the real size (it is UI state, not document content).
    pub fn is_auto_sized(&self) -> bool {
        matches!(self, RegionKind::Math(_) | RegionKind::Text { .. })
    }

    /// Whether the content can be edited in place.
    pub fn is_editable(&self) -> bool {
        self.is_auto_sized()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub id: RegionId,
    /// Top-left corner in document space.
    pub pos: Point,
    pub size: Size,
    pub kind: RegionKind,
}
