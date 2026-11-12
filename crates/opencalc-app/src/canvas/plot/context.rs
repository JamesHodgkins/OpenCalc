//! What painting a plot needs from the rest of the program, and the small helpers
//! that name its parts.

use opencalc_core::document::{PlotSpec, SeriesStyle};
use opencalc_core::eval::Output;
use opencalc_core::registry::UnitSystem;

use crate::canvas::Env;
use crate::theme::Theme;

/// What painting a plot needs from the rest of the program: the palette of a page,
/// the evaluated plots and the unit system the axes are written in.
pub struct PlotCtx<'a> {
    pub paper: &'a Theme,
    pub output: &'a Output,
    pub units: UnitSystem,
}

impl<'a> From<&Env<'a>> for PlotCtx<'a> {
    fn from(env: &Env<'a>) -> PlotCtx<'a> {
        PlotCtx {
            paper: env.paper,
            output: env.output,
            units: env.units,
        }
    }
}

/// Below this on-screen text size, text is left out rather than drawn as noise.
pub const MIN_TEXT: f32 = 5.0;

/// The positions in `spec.series` of the series that have data. A blank series
/// is not evaluated, so the k-th series drawn is `used_series(spec)[k]` of the spec.
pub fn used_series(spec: &PlotSpec) -> Vec<usize> {
    (0..spec.series.len())
        .filter(|k| spec.has_data(*k))
        .collect()
}

/// `a [unit]`, or whichever of the two there is.
pub fn caption(user: &str, unit: &str) -> String {
    match (user.is_empty(), unit.is_empty()) {
        (true, true) => String::new(),
        (true, false) => unit.to_owned(),
        (false, true) => user.to_owned(),
        (false, false) => format!("{user} [{unit}]"),
    }
}

/// The name a series goes by: its own, or its expression.
pub fn series_name(spec: &PlotSpec, at: usize, style: &SeriesStyle) -> String {
    if style.name.trim().is_empty() {
        spec.series
            .get(at)
            .map_or_else(String::new, |s| s.source().to_owned())
    } else {
        style.name.clone()
    }
}
