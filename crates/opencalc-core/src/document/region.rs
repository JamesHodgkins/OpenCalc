//! Regions: the things placed on a worksheet.

use serde::{Deserialize, Serialize};

use super::geometry::{Point, Size};
use super::picture::PictureSpec;
use super::plot::PlotSpec;
use super::result_format::ResultFormat;
use super::table::TableSpec;
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
    format: ResultFormat,
}

/// The saved form of a [`MathRegion`]: the source text only.
#[derive(Serialize, Deserialize)]
struct MathSource {
    source: String,
    #[serde(default, skip_serializing_if = "ResultFormat::is_default")]
    format: ResultFormat,
}

impl From<MathSource> for MathRegion {
    fn from(s: MathSource) -> MathRegion {
        let mut m = MathRegion::new(&s.source);
        m.format = s.format;
        m
    }
}

impl From<MathRegion> for MathSource {
    fn from(m: MathRegion) -> MathSource {
        MathSource {
            source: m.source,
            format: m.format,
        }
    }
}

impl MathRegion {
    /// Parse `source`. Never fails: a parse error becomes `Expr::Error`.
    pub fn new(source: &str) -> MathRegion {
        MathRegion {
            source: source.to_owned(),
            expr: parse(source),
            format: ResultFormat::default(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn expr(&self) -> &Expr {
        &self.expr
    }

    /// How the result is shown.
    pub fn format(&self) -> ResultFormat {
        self.format.clone()
    }

    pub(crate) fn set_format(&mut self, format: ResultFormat) {
        self.format = format;
    }

    pub(crate) fn set_source(&mut self, source: String) {
        self.expr = parse(&source);
        self.source = source;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RegionKind {
    Math(MathRegion),
    Text {
        text: String,
    },
    Area {
        title: String,
    },
    Plot(PlotSpec),
    /// Boxed, like a plot's options, to keep every region small.
    Table(Box<TableSpec>),
    /// Boxed for the same reason as the table.
    Picture(Box<PictureSpec>),
}

impl RegionKind {
    pub fn math(source: &str) -> RegionKind {
        RegionKind::Math(MathRegion::new(source))
    }

    /// An empty X-Y plot.
    pub fn plot() -> RegionKind {
        RegionKind::Plot(PlotSpec::default())
    }

    /// A picture with no image yet.
    pub fn picture() -> RegionKind {
        RegionKind::Picture(Box::default())
    }

    /// A blank table.
    pub fn table() -> RegionKind {
        RegionKind::Table(Box::default())
    }

    /// Short name of the region type, for display and diagnostics.
    pub fn label(&self) -> &'static str {
        match self {
            RegionKind::Math(_) => "Math",
            RegionKind::Text { .. } => "Text",
            RegionKind::Area { .. } => "Area",
            RegionKind::Plot(_) => "Plot",
            RegionKind::Table(_) => "Table",
            RegionKind::Picture(_) => "Picture",
        }
    }

    /// Starting size in document units.
    pub fn default_size(&self) -> Size {
        match self {
            RegionKind::Math(_) => Size::new(160.0, 32.0),
            RegionKind::Text { .. } => Size::new(240.0, 40.0),
            RegionKind::Area { .. } => Size::new(320.0, 200.0),
            RegionKind::Plot(_) | RegionKind::Table(_) | RegionKind::Picture(_) => {
                Size::new(260.0, 170.0)
            }
        }
    }

    /// Whether the region's content size comes from typesetting rather than the
    /// stored size. For these the stored size is only a starting guess, and the UI
    /// measures the real size (it is UI state, not document content).
    pub fn is_auto_sized(&self) -> bool {
        matches!(
            self,
            RegionKind::Math(_) | RegionKind::Text { .. } | RegionKind::Table(_)
        )
    }

    /// Whether the content can be edited in place.
    pub fn is_editable(&self) -> bool {
        matches!(self, RegionKind::Math(_) | RegionKind::Text { .. })
    }
}

/// Bold and italic for a text region (the whole region, not parts of it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontStyle {
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
}

impl FontStyle {
    pub fn is_default(&self) -> bool {
        *self == FontStyle::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub id: RegionId,
    /// Top-left corner in document space.
    pub pos: Point,
    pub size: Size,
    pub kind: RegionKind,
    /// Bold and italic. Only a text region uses them; others keep the default.
    #[serde(default, skip_serializing_if = "FontStyle::is_default")]
    pub style: FontStyle,
    /// A locked region cannot be moved, resized, edited, restyled or deleted until
    /// it is unlocked. Core refuses such edits, so no part of the program can
    /// change one by mistake.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Draw a line round the region (on the page and in exports).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub border: bool,
    /// Styled stretches of a text region (see `rich`); empty for plain text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<super::rich::Run>,
    /// A short name for finding the region again (empty: none). Shown beside the
    /// region while editing, never on the printed page.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

/// Longest label the program sets, in characters.
pub const MAX_LABEL_CHARS: usize = 60;

/// A label as it will be kept: control characters and surrounding blanks removed,
/// cut to [`MAX_LABEL_CHARS`].
pub fn clean_label(raw: &str) -> String {
    let flat: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.trim().chars().take(MAX_LABEL_CHARS).collect()
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn a_label_is_one_line_trimmed_and_cut_to_length() {
        assert_eq!(clean_label("  Beam check  "), "Beam check");
        assert_eq!(clean_label("a\nb\tc"), "a b c");
        assert_eq!(clean_label("   "), "");
        let long = "x".repeat(MAX_LABEL_CHARS + 20);
        assert_eq!(clean_label(&long).chars().count(), MAX_LABEL_CHARS);
        assert_eq!(clean_label("\u{3c0} r\u{b2}"), "\u{3c0} r\u{b2}");
    }
}
