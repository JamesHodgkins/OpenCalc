//! How a plot looks and how its axes behave: the settings of the whole plot
//! ([`PlotOptions`]) and of each data series ([`SeriesStyle`], which also says
//! which y axis the series is on and whether it has x values of its own).
//!
//! All of it is saved, and nothing is saved for a setting left at its default, so
//! a plain plot's file is the same size as before these options existed. Axis
//! limits are math sources like the plot's data, so they can carry units
//! (`10 s`) or read a variable; blank means automatic.

use serde::{Deserialize, Serialize};

use super::region::MathRegion;

/// How one data series is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesKind {
    /// Straight lines between the points.
    #[default]
    Line,
    /// A marker at each point, no line.
    Markers,
    /// Lines and a marker at each point.
    LineMarkers,
    /// Horizontal then vertical steps.
    Step,
    /// A vertical line from the baseline up to each point, topped by a marker.
    Stem,
    /// A column from the baseline up to each point.
    Bar,
}

impl SeriesKind {
    pub const ALL: [SeriesKind; 6] = [
        SeriesKind::Line,
        SeriesKind::Markers,
        SeriesKind::LineMarkers,
        SeriesKind::Step,
        SeriesKind::Stem,
        SeriesKind::Bar,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SeriesKind::Line => "Line",
            SeriesKind::Markers => "Markers",
            SeriesKind::LineMarkers => "Line and markers",
            SeriesKind::Step => "Step",
            SeriesKind::Stem => "Stem",
            SeriesKind::Bar => "Bar",
        }
    }
}

/// The shape of a point marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Marker {
    #[default]
    Circle,
    Square,
    Triangle,
    Cross,
}

impl Marker {
    pub const ALL: [Marker; 4] = [
        Marker::Circle,
        Marker::Square,
        Marker::Triangle,
        Marker::Cross,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Marker::Circle => "Circle",
            Marker::Square => "Square",
            Marker::Triangle => "Triangle",
            Marker::Cross => "Cross",
        }
    }
}

/// Which vertical axis a series is measured on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YAxis {
    /// The left axis, shared by every series that does not ask for the right.
    #[default]
    Left,
    /// A second axis on the right, for series that measure something else.
    Right,
}

impl YAxis {
    pub const ALL: [YAxis; 2] = [YAxis::Left, YAxis::Right];

    pub fn label(self) -> &'static str {
        match self {
            YAxis::Left => "Left axis",
            YAxis::Right => "Right axis",
        }
    }
}

/// Number of line colours in the palette; a series picks one by index.
pub const PALETTE_SIZE: u8 = 8;

/// The look of one data series. The default is a plain line in the next palette
/// colour with the series' expression as its legend name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesStyle {
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: SeriesKind,
    #[serde(default, skip_serializing_if = "is_default")]
    pub marker: Marker,
    /// Palette index (below [`PALETTE_SIZE`]), or `None` for the series' own place
    /// in the cycle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<u8>,
    /// Legend name; blank shows the expression.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The vertical axis this series is measured on.
    #[serde(default, skip_serializing_if = "is_default")]
    pub axis: YAxis,
    /// A math source for this series' own x values; blank uses the plot's shared
    /// x values. With one per series a plot can draw curves that are not functions
    /// of a common x, such as a circle (`cos(t)` against `sin(t)`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub x: String,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

impl SeriesStyle {
    pub fn is_default(&self) -> bool {
        *self == SeriesStyle::default()
    }
}

/// Whether the legend is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegendMode {
    /// Shown when there is more than one series.
    #[default]
    Auto,
    Show,
    Hide,
}

impl LegendMode {
    pub const ALL: [LegendMode; 3] = [LegendMode::Auto, LegendMode::Show, LegendMode::Hide];

    pub fn label(self) -> &'static str {
        match self {
            LegendMode::Auto => "Automatic",
            LegendMode::Show => "Show",
            LegendMode::Hide => "Hide",
        }
    }
}

/// Settings of the plot as a whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotOptions {
    #[serde(default = "yes")]
    pub grid: bool,
    #[serde(default)]
    pub legend: LegendMode,
    /// Logarithmic axes: every value on the axis must be positive.
    #[serde(default)]
    pub x_log: bool,
    #[serde(default)]
    pub y_log: bool,
    /// Axis limits as math sources; blank means automatic.
    #[serde(default = "blank")]
    pub x_min: MathRegion,
    #[serde(default = "blank")]
    pub x_max: MathRegion,
    #[serde(default = "blank")]
    pub y_min: MathRegion,
    #[serde(default = "blank")]
    pub y_max: MathRegion,
    /// The right axis, used by series set to [`YAxis::Right`].
    #[serde(default)]
    pub y2_log: bool,
    #[serde(default = "blank")]
    pub y2_min: MathRegion,
    #[serde(default = "blank")]
    pub y2_max: MathRegion,
    /// Caption of the right axis.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub y2_label: String,
}

fn yes() -> bool {
    true
}

fn blank() -> MathRegion {
    MathRegion::new("")
}

impl Default for PlotOptions {
    fn default() -> PlotOptions {
        PlotOptions {
            grid: true,
            legend: LegendMode::Auto,
            x_log: false,
            y_log: false,
            x_min: blank(),
            x_max: blank(),
            y_min: blank(),
            y_max: blank(),
            y2_log: false,
            y2_min: blank(),
            y2_max: blank(),
            y2_label: String::new(),
        }
    }
}

impl PlotOptions {
    pub fn is_default(&self) -> bool {
        *self == PlotOptions::default()
    }

    /// The limit sources in the order x min, x max, y min, y max, right min,
    /// right max.
    pub fn limits(&self) -> [&MathRegion; 6] {
        [
            &self.x_min,
            &self.x_max,
            &self.y_min,
            &self.y_max,
            &self.y2_min,
            &self.y2_max,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_save_as_nothing() {
        assert_eq!(
            serde_json::to_string(&SeriesStyle::default()).unwrap(),
            "{}"
        );
        assert!(PlotOptions::default().is_default());
        assert!(SeriesStyle::default().is_default());
    }

    #[test]
    fn a_style_round_trips_with_only_what_differs() {
        let s = SeriesStyle {
            kind: SeriesKind::Stem,
            marker: Marker::Triangle,
            colour: Some(3),
            name: "load".into(),
            axis: YAxis::Right,
            x: "cos(t)".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"stem","marker":"triangle","colour":3,"name":"load","axis":"right","x":"cos(t)"}"#
        );
        assert_eq!(serde_json::from_str::<SeriesStyle>(&json).unwrap(), s);
        let only_kind = SeriesStyle {
            kind: SeriesKind::Bar,
            ..SeriesStyle::default()
        };
        assert_eq!(
            serde_json::to_string(&only_kind).unwrap(),
            r#"{"kind":"bar"}"#
        );
    }

    #[test]
    fn options_read_from_partial_or_empty_json() {
        let o: PlotOptions = serde_json::from_str("{}").unwrap();
        assert!(o.is_default() && o.grid);
        let o: PlotOptions =
            serde_json::from_str(r#"{"grid":false,"x_log":true,"y_max":{"source":"10 m"}}"#)
                .unwrap();
        assert!(!o.grid && o.x_log && !o.y_log);
        assert_eq!(o.y_max.source(), "10 m");
        assert_eq!(o.x_min.source(), "");
    }

    #[test]
    fn unknown_kinds_and_markers_are_refused_not_guessed() {
        assert!(serde_json::from_str::<SeriesStyle>(r#"{"kind":"hologram"}"#).is_err());
        assert!(serde_json::from_str::<SeriesStyle>(r#"{"marker":"star"}"#).is_err());
    }
}
