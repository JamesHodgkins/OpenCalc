//! Evaluating plot regions: turn their x and series expressions into points.

use super::node::eval_region;
use super::{Cx, EvalError};
use crate::document::PlotInput;
use crate::numeric;
use crate::units::{Dimension, UnitError};
use crate::value::{Value, ValueError};

/// One line on a plot, in SI values.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotSeries {
    pub y_dim: Dimension,
    pub points: Vec<(f64, f64)>,
    /// Measured on the right-hand axis.
    pub right: bool,
}

/// What a plot region draws.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotData {
    pub x_dim: Dimension,
    pub series: Vec<PlotSeries>,
    /// The dimension of the right-hand axis, when some series use it.
    pub y2_dim: Option<Dimension>,
    /// Logarithmic axes (every value on them is positive).
    pub x_log: bool,
    pub y_log: bool,
    pub y2_log: bool,
    /// Axis limits in SI, in the order x min, x max, y min, y max, right min,
    /// right max; `None` is automatic. Their units were checked against the axes.
    pub limits: [Option<f64>; 6],
}

impl PlotData {
    /// A plot with nothing to draw yet.
    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }

    /// The dimension of the left-hand axis: that of the first series on it.
    pub fn left_dim(&self) -> Dimension {
        self.series
            .iter()
            .find(|s| !s.right)
            .map_or(Dimension::NONE, |s| s.y_dim)
    }
}

/// The real numbers in a list or a vector (a single row or column), and their
/// dimension.
fn numbers(value: &Value) -> Result<(Vec<f64>, Dimension), EvalError> {
    let (items, dim) = match value {
        Value::Series(r) => (r.elements().to_vec(), r.dim),
        Value::Matrix(m) if m.cols() == 1 || m.rows() == 1 => {
            (m.elements().to_vec(), m.common_dim()?)
        }
        _ => {
            return Err(ValueError::Shape(
                "a plot needs a list or vector of values, like a range".into(),
            )
            .into())
        }
    };
    if items.iter().any(|z| !numeric::is_real(*z)) {
        return Err(ValueError::Unsupported("plotting complex numbers".into()).into());
    }
    Ok((items.iter().map(|z| z.re).collect(), dim))
}

fn in_plot(part: &str, error: EvalError) -> EvalError {
    EvalError::InPlot {
        part: part.to_owned(),
        error: Box::new(error),
    }
}

fn positive_only() -> EvalError {
    EvalError::Invalid("a logarithmic axis needs every value to be positive".into())
}

pub(super) fn evaluate_plot(input: &PlotInput, cx: &Cx) -> Result<PlotData, EvalError> {
    let mut data = PlotData {
        x_dim: Dimension::NONE,
        series: Vec::new(),
        y2_dim: None,
        x_log: input.x_log,
        y_log: input.y_log,
        y2_log: input.y2_log,
        limits: [None; 6],
    };
    // Every series needs the shared x values unless it has x values of its own.
    let needs_shared = input.series.iter().any(|s| s.x.is_none());
    if input.x_blank && needs_shared {
        if input.series.is_empty() {
            return Ok(data);
        }
        return Err(EvalError::Invalid(
            "enter the x values to plot against".into(),
        ));
    }
    let value = |e| eval_region(e, cx).map_err(|l| l.error);
    let shared = if input.x_blank {
        None
    } else {
        let (xs, dim) = value(&input.x)
            .and_then(|v| numbers(&v))
            .map_err(|e| in_plot("x", e))?;
        if input.x_log && xs.iter().any(|x| *x <= 0.0) {
            return Err(in_plot("x", positive_only()));
        }
        data.x_dim = dim;
        Some((xs, dim))
    };
    // A series asking for the right axis only gets one if another is on the left.
    let any_left = input.series.iter().any(|s| !s.right);
    let (mut left_dim, mut right_dim): (Option<Dimension>, Option<Dimension>) = (None, None);
    let mut x_dim_seen = shared.as_ref().map(|(_, d)| *d);
    for (k, series) in input.series.iter().enumerate() {
        let part = format!("series {}", k + 1);
        let right = series.right && any_left;
        let (xs, x_dim) = match (&series.x, &shared) {
            (Some(e), _) => {
                let own = format!("{part} x");
                let (xs, dim) = value(e)
                    .and_then(|v| numbers(&v))
                    .map_err(|e| in_plot(&own, e))?;
                if input.x_log && xs.iter().any(|x| *x <= 0.0) {
                    return Err(in_plot(&own, positive_only()));
                }
                (xs, dim)
            }
            (None, Some((xs, dim))) => (xs.clone(), *dim),
            // Cannot happen: a series without x values of its own was checked above to
            // have shared ones to use.
            (None, None) => {
                return Err(EvalError::Internal("a plot series has no x values".into()))
            }
        };
        // One x axis: every series' x values measure the same thing.
        match x_dim_seen {
            None => {
                x_dim_seen = Some(x_dim);
                data.x_dim = x_dim;
            }
            Some(first) if first != x_dim => {
                let mismatch = UnitError::Mismatch(first, x_dim);
                return Err(in_plot(&format!("{part} x"), mismatch.into()));
            }
            Some(_) => {}
        }
        let (ys, y_dim) = value(&series.y)
            .and_then(|v| numbers(&v))
            .map_err(|e| in_plot(&part, e))?;
        if ys.len() != xs.len() {
            let message = format!("has {} values but x has {}", ys.len(), xs.len());
            return Err(in_plot(&part, ValueError::Shape(message).into()));
        }
        // The series on one axis share it, so they must measure the same thing.
        let group = if right { &mut right_dim } else { &mut left_dim };
        match *group {
            Some(first) if first != y_dim => {
                let mismatch = UnitError::Mismatch(first, y_dim);
                return Err(in_plot(&part, mismatch.into()));
            }
            _ => *group = Some(y_dim),
        }
        let log = if right { input.y2_log } else { input.y_log };
        if log && ys.iter().any(|y| *y <= 0.0) {
            return Err(in_plot(&part, positive_only()));
        }
        data.series.push(PlotSeries {
            y_dim,
            points: xs.iter().copied().zip(ys).collect(),
            right,
        });
    }
    data.y2_dim = right_dim;
    resolve_limits(input, &mut data, cx)?;
    Ok(data)
}

/// Evaluate the axis limits, check their units against the axes and order, and
/// check that a log axis has positive limits. Limits of the right axis are ignored
/// while no series is on it.
fn resolve_limits(input: &PlotInput, data: &mut PlotData, cx: &Cx) -> Result<(), EvalError> {
    const NAMES: [&str; 6] = [
        "x min",
        "x max",
        "y min",
        "y max",
        "right axis min",
        "right axis max",
    ];
    let left = data.left_dim();
    for (k, limit) in input.limits.iter().enumerate() {
        let Some(expr) = limit else { continue };
        let (axis, log) = match k {
            0 | 1 => (data.x_dim, data.x_log),
            2 | 3 => (left, data.y_log),
            _ => match data.y2_dim {
                Some(d) => (d, data.y2_log),
                None => continue,
            },
        };
        let value = eval_region(expr, cx)
            .map_err(|l| in_plot(NAMES[k], l.error))
            .and_then(|v| match v.as_real() {
                Some(q) => Ok(q),
                None => Err(in_plot(
                    NAMES[k],
                    EvalError::Invalid("an axis limit must be a real number".into()),
                )),
            })?;
        if value.dim != axis {
            let mismatch = UnitError::Mismatch(axis, value.dim);
            return Err(in_plot(NAMES[k], mismatch.into()));
        }
        if log && value.si <= 0.0 {
            return Err(in_plot(NAMES[k], positive_only()));
        }
        data.limits[k] = Some(value.si);
    }
    for (lo, hi, name) in [(0, 1, "x"), (2, 3, "y"), (4, 5, "right axis")] {
        if let (Some(a), Some(b)) = (data.limits[lo], data.limits[hi]) {
            if a >= b {
                let message = format!("the {name} minimum must be less than the {name} maximum");
                return Err(in_plot(
                    &format!("{name} limits"),
                    EvalError::Invalid(message),
                ));
            }
        }
    }
    Ok(())
}
