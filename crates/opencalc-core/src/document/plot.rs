//! What an X-Y plot region holds: where its data comes from, as math sources.

use serde::{Deserialize, Serialize};

use super::plot_style::{PlotOptions, SeriesStyle};
use super::region::MathRegion;
use crate::expr::Expr;

/// Most data series one plot draws.
pub const MAX_SERIES: usize = 8;

/// An X-Y plot. The x values and each series are expressions (saved as source
/// text, like a Math region) that must evaluate to a list or vector; the
/// evaluator turns them into points. A blank series is ignored, so a new plot is
/// valid and empty until something is typed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlotSpec {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub x_label: String,
    #[serde(default)]
    pub y_label: String,
    pub x: MathRegion,
    pub series: Vec<MathRegion>,
    /// Axis limits, grid, legend and log axes. Saved only when changed. Boxed
    /// because it is big and rarely changed, and every region (and every edit)
    /// would otherwise be as large as the biggest plot.
    #[serde(default, skip_serializing_if = "PlotOptions::is_default")]
    pub options: Box<PlotOptions>,
    /// How each series is drawn, by position. Shorter than `series` (or empty)
    /// means the rest are plain lines; trailing defaults are not kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<SeriesStyle>,
}

impl Default for PlotSpec {
    fn default() -> PlotSpec {
        PlotSpec {
            title: String::new(),
            x_label: String::new(),
            y_label: String::new(),
            x: MathRegion::new(""),
            series: vec![MathRegion::new("")],
            options: Box::default(),
            styles: Vec::new(),
        }
    }
}

impl PlotSpec {
    /// Whether nothing has been entered to plot.
    pub fn is_blank(&self) -> bool {
        self.x.source().trim().is_empty()
            && self.series.iter().all(|s| s.source().trim().is_empty())
    }

    /// A copy with other x values.
    pub fn with_x(mut self, source: &str) -> PlotSpec {
        self.x = MathRegion::new(source);
        self
    }

    /// A copy with series `index` replaced; an index past the end adds one (up to
    /// [`MAX_SERIES`]).
    pub fn with_series(mut self, index: usize, source: &str) -> PlotSpec {
        if let Some(s) = self.series.get_mut(index) {
            *s = MathRegion::new(source);
        } else if self.series.len() < MAX_SERIES {
            self.series.push(MathRegion::new(source));
        }
        self
    }

    /// A copy without series `index`. The last series is never removed (it is
    /// blanked instead), so a plot always has somewhere to type.
    pub fn without_series(mut self, index: usize) -> PlotSpec {
        if self.series.len() > 1 {
            if index < self.series.len() {
                self.series.remove(index);
                if index < self.styles.len() {
                    self.styles.remove(index);
                }
            }
        } else if let Some(s) = self.series.get_mut(index) {
            *s = MathRegion::new("");
            self.styles.clear();
        }
        self.trimmed()
    }

    /// The style of series `index`: its own, or the default.
    pub fn style(&self, index: usize) -> SeriesStyle {
        self.styles.get(index).cloned().unwrap_or_default()
    }

    /// A copy with series `index` drawn in `style`. An index past the last series
    /// is ignored.
    pub fn with_style(mut self, index: usize, style: SeriesStyle) -> PlotSpec {
        if index >= self.series.len() {
            return self;
        }
        if self.styles.len() <= index {
            self.styles.resize(index + 1, SeriesStyle::default());
        }
        self.styles[index] = style;
        self.trimmed()
    }

    /// A copy with other plot-wide options.
    pub fn with_options(mut self, options: PlotOptions) -> PlotSpec {
        self.options = Box::new(options);
        self
    }

    /// Styles are kept sparse: no more than there are series, and no trailing
    /// defaults, so plots that look the same compare equal.
    pub fn trimmed(mut self) -> PlotSpec {
        self.styles.truncate(self.series.len());
        while self.styles.last().is_some_and(SeriesStyle::is_default) {
            self.styles.pop();
        }
        self
    }

    pub fn with_title(mut self, title: &str) -> PlotSpec {
        self.title = title.to_owned();
        self
    }

    pub fn with_labels(mut self, x_label: &str, y_label: &str) -> PlotSpec {
        self.x_label = x_label.to_owned();
        self.y_label = y_label.to_owned();
        self
    }
}

/// A plot as the evaluator takes it: the expressions to evaluate.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotInput {
    pub x: Expr,
    /// One per non-blank series.
    pub series: Vec<Expr>,
    /// The x source is empty (whatever `x` then holds).
    pub x_blank: bool,
    /// Logarithmic axes.
    pub x_log: bool,
    pub y_log: bool,
    /// Axis limits in the order x min, x max, y min, y max; `None` is automatic.
    pub limits: [Option<Expr>; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_plot_is_blank_with_one_empty_series() {
        let p = PlotSpec::default();
        assert!(p.is_blank());
        assert_eq!(p.series.len(), 1);
    }

    #[test]
    fn series_can_be_added_replaced_and_removed_but_never_all() {
        let p = PlotSpec::default().with_x("x").with_series(0, "x ^ 2");
        assert!(!p.is_blank());
        let p = p.with_series(1, "x ^ 3");
        assert_eq!(p.series.len(), 2);
        assert_eq!(p.series[1].source(), "x ^ 3");
        let p = p.without_series(0);
        assert_eq!(p.series.len(), 1);
        assert_eq!(p.series[0].source(), "x ^ 3");
        // The last one is blanked, not removed.
        let p = p.without_series(0);
        assert_eq!(p.series.len(), 1);
        assert_eq!(p.series[0].source(), "");
    }

    #[test]
    fn a_plot_has_at_most_the_maximum_number_of_series() {
        let mut p = PlotSpec::default();
        for i in 0..MAX_SERIES + 3 {
            p = p.with_series(i, "x");
        }
        assert_eq!(p.series.len(), MAX_SERIES);
    }

    #[test]
    fn a_plot_saves_its_sources_and_reads_old_files_without_labels() {
        let p = PlotSpec::default()
            .with_x("x")
            .with_series(0, "x ^ 2")
            .with_title("Parabola");
        let json = serde_json::to_string(&p).unwrap();
        assert!(
            !json.contains("Binary"),
            "the expression tree must not be saved: {json}"
        );
        let back: PlotSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        let minimal = r#"{"x":{"source":"x"},"series":[{"source":"x"}]}"#;
        let old: PlotSpec = serde_json::from_str(minimal).unwrap();
        assert_eq!(old.title, "");
        assert_eq!(old.series[0].source(), "x");
    }
}
