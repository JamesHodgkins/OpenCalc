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
        data.series.push(PlotSeries {
            y_dim,
            points: xs.iter().copied().zip(ys).collect(),
        });
    }
    Ok(data)
}
