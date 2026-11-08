//! Functions that reduce a list, vector or matrix to one number: `sum`, `mean`,
//! `stdev`, `min`, `max`, `length`. Each takes one or more arguments and uses
//! every element of all of them, so `max(1, 5, 3)` and `max(v)` both work.

use super::Result;
use crate::eval::EvalError;
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use crate::value::{Scalar, Value, ValueError};

/// Every element of every argument, with the one dimension they share.
pub(super) fn gather(args: &[Value]) -> Result<(Vec<Complex64>, Dimension)> {
    let mut out = Vec::new();
    let mut dim = None;
    for a in args {
        let (items, d): (Vec<Complex64>, Dimension) = match a {
            Value::Scalar(s) => (vec![s.z], s.dim),
            Value::Matrix(m) => (m.elements().to_vec(), m.common_dim()?),
            Value::Series(r) => (r.elements().to_vec(), r.dim),
            Value::Function { .. } => {
                return Err(ValueError::Unsupported("a function as a value".into()).into())
            }
        };
        match dim {
            None => dim = Some(d),
            Some(first) if first != d => {
                return Err(ValueError::from(UnitError::Mismatch(first, d)).into())
            }
            Some(_) => {}
        }
        out.extend(items);
    }
    Ok((out, dim.unwrap_or(Dimension::NONE)))
}

/// The same, but every element must be real (for ordering and spread).
pub(super) fn gather_real(name: &str, args: &[Value]) -> Result<(Vec<f64>, Dimension)> {
    let (items, dim) = gather(args)?;
    if items.iter().any(|z| !numeric::is_real(*z)) {
        return Err(ValueError::Unsupported(format!("'{name}' of complex numbers")).into());
    }
    Ok((items.iter().map(|z| z.re).collect(), dim))
}

fn real_result(v: f64, dim: Dimension) -> Result<Value> {
    Ok(Value::Scalar(Scalar::new(numeric::real(v), dim)))
}

pub(super) fn sum(args: &[Value]) -> Result<Value> {
    let (items, dim) = gather(args)?;
    let total: Complex64 = items.iter().sum();
    Ok(Value::Scalar(Scalar::new(total, dim)))
}

pub(super) fn mean(args: &[Value]) -> Result<Value> {
    let (items, dim) = gather(args)?;
    let total: Complex64 = items.iter().sum();
    let n = numeric::real(items.len() as f64);
    Ok(Value::Scalar(Scalar::new(numeric::div(total, n), dim)))
}

/// Spread about the mean. `denominator` is `n` (population) or `n - 1` (sample).
fn spread(name: &str, args: &[Value], sample: bool) -> Result<Value> {
    let (items, dim) = gather_real(name, args)?;
    let n = items.len();
    if sample && n < 2 {
        return Err(EvalError::Invalid(format!(
            "'{name}' needs at least two values"
        )));
    }
    let mean = items.iter().sum::<f64>() / n as f64;
    let squares: f64 = items.iter().map(|x| (x - mean) * (x - mean)).sum();
    let divisor = if sample { n - 1 } else { n } as f64;
    real_result((squares / divisor).sqrt(), dim)
}

pub(super) fn stdev(args: &[Value]) -> Result<Value> {
    spread("stdev", args, false)
}

pub(super) fn stdev_sample(args: &[Value]) -> Result<Value> {
    spread("stdev_sample", args, true)
}

pub(super) fn min(args: &[Value]) -> Result<Value> {
    let (items, dim) = gather_real("min", args)?;
    real_result(items.iter().copied().fold(f64::INFINITY, f64::min), dim)
}

pub(super) fn max(args: &[Value]) -> Result<Value> {
    let (items, dim) = gather_real("max", args)?;
    real_result(items.iter().copied().fold(f64::NEG_INFINITY, f64::max), dim)
}

pub(super) fn length(args: &[Value]) -> Result<Value> {
    let (items, _) = gather(args)?;
    Ok(Value::dimensionless(items.len() as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64) -> Value {
        Value::dimensionless(x)
    }

    fn real(r: Result<Value>) -> f64 {
        r.unwrap().as_real().unwrap().si
    }

    #[test]
    fn statistics_of_several_scalars() {
        let args = [
            v(2.0),
            v(4.0),
            v(4.0),
            v(4.0),
            v(5.0),
            v(5.0),
            v(7.0),
            v(9.0),
        ];
        assert_eq!(real(sum(&args)), 40.0);
        assert_eq!(real(mean(&args)), 5.0);
        assert_eq!(real(stdev(&args)), 2.0);
        assert!((real(stdev_sample(&args)) - 2.138_089_935_299_395).abs() < 1e-12);
        assert_eq!(real(min(&args)), 2.0);
        assert_eq!(real(max(&args)), 9.0);
        assert_eq!(real(length(&args)), 8.0);
    }

    #[test]
    fn a_sample_needs_two_values() {
        assert!(stdev_sample(&[v(1.0)]).is_err());
        assert_eq!(real(stdev(&[v(1.0)])), 0.0);
    }

    #[test]
    fn mixed_dimensions_are_rejected() {
        let m = Value::from(crate::units::Quantity::from_unit(1.0, "m").unwrap());
        assert!(sum(&[v(1.0), m]).is_err());
    }

    #[test]
    fn ordering_needs_real_numbers() {
        assert!(max(&[Value::complex(0.0, 1.0), v(1.0)]).is_err());
        assert!(sum(&[Value::complex(0.0, 1.0), v(1.0)]).is_ok());
    }
}
