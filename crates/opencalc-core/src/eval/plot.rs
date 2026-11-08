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
}

/// What a plot region draws.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotData {
    pub x_dim: Dimension,
    pub series: Vec<PlotSeries>,
    /// Logarithmic axes (every value on them is positive).
    pub x_log: bool,
    pub y_log: bool,
    /// Axis limits in SI, in the order x min, x max, y min, y max; `None` is
    /// automatic. Their units were checked against the axes.
    pub limits: [Option<f64>; 4],
}

impl PlotData {
    /// A plot with nothing to draw yet.
    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
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

pub(super) fn evaluate_plot(input: &PlotInput, cx: &Cx) -> Result<PlotData, EvalError> {
    let mut data = PlotData {
        x_dim: Dimension::NONE,
        series: Vec::new(),
        x_log: input.x_log,
        y_log: input.y_log,
        limits: [None; 4],
    };
    if input.x_blank {
        if input.series.is_empty() {
            return Ok(data);
        }
        return Err(EvalError::Invalid(
            "enter the x values to plot against".into(),
        ));
    }
    let value = |e| eval_region(e, cx).map_err(|l| l.error);
    let (xs, x_dim) = value(&input.x)
        .and_then(|v| numbers(&v))
        .map_err(|e| in_plot("x", e))?;
    data.x_dim = x_dim;
    if input.x_log && xs.iter().any(|x| *x <= 0.0) {
        return Err(in_plot("x", positive_only()));
    }
    for (k, series) in input.series.iter().enumerate() {
        let part = format!("series {}", k + 1);
        let (ys, y_dim) = value(series)
            .and_then(|v| numbers(&v))
            .map_err(|e| in_plot(&part, e))?;
        if ys.len() != xs.len() {
            let message = format!("has {} values but x has {}", ys.len(), xs.len());
            return Err(in_plot(&part, ValueError::Shape(message).into()));
        }
        // All series share one y axis, so they must measure the same thing.
        if let Some(first) = data.series.first() {
            if first.y_dim != y_dim {
                let mismatch = UnitError::Mismatch(first.y_dim, y_dim);
                return Err(in_plot(&part, mismatch.into()));
            }
        }
        if input.y_log && ys.iter().any(|y| *y <= 0.0) {
            return Err(in_plot(&part, positive_only()));
        }
        data.series.push(PlotSeries {
            y_dim,
            points: xs.iter().copied().zip(ys).collect(),
        });
    }
    resolve_limits(input, &mut data, cx)?;
    Ok(data)
}

fn positive_only() -> EvalError {
    EvalError::Invalid("a logarithmic axis needs every value to be positive".into())
}

/// Evaluate the axis limits, check their units against the axes and order, and
/// check that a log axis has positive limits.
fn resolve_limits(input: &PlotInput, data: &mut PlotData, cx: &Cx) -> Result<(), EvalError> {
    const NAMES: [&str; 4] = ["x min", "x max", "y min", "y max"];
    let y_dim = data.series.first().map_or(Dimension::NONE, |s| s.y_dim);
    for (k, limit) in input.limits.iter().enumerate() {
        let Some(expr) = limit else { continue };
        let axis = if k < 2 { data.x_dim } else { y_dim };
        let log = if k < 2 { data.x_log } else { data.y_log };
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
    for (lo, hi, name) in [(0, 1, "x"), (2, 3, "y")] {
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
