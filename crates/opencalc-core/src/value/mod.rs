//! Values the evaluator produces: a scalar (complex number with a dimension) or
//! a matrix of them. A vector is a matrix with one column. Real numbers are
//! scalars with a zero imaginary part, so there is one arithmetic, not two.

mod matrix;
mod scalar;

pub use matrix::{Matrix, MAX_ELEMENTS};
pub use scalar::Scalar;

use crate::numeric::Complex64;
use crate::units::{Dimension, Quantity, UnitError};

/// Why a value operation failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ValueError {
    #[error(transparent)]
    Unit(#[from] UnitError),
    #[error("{0}")]
    Shape(String),
    #[error("{0} is not supported for these values")]
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Scalar(Scalar),
    Matrix(Matrix),
}

impl From<Quantity> for Value {
    fn from(q: Quantity) -> Value {
        Value::Scalar(q.into())
    }
}

impl From<Scalar> for Value {
    fn from(s: Scalar) -> Value {
        Value::Scalar(s)
    }
}

impl Value {
    pub fn dimensionless(v: f64) -> Value {
        Quantity::dimensionless(v).into()
    }

    /// A dimensionless complex number `re + im i`.
    pub fn complex(re: f64, im: f64) -> Value {
        Value::Scalar(Scalar::new(Complex64::new(re, im), Dimension::NONE))
    }

    pub fn is_finite(&self) -> bool {
        match self {
            Value::Scalar(s) => s.is_finite(),
            Value::Matrix(m) => m.is_finite(),
        }
    }

    pub fn dim(&self) -> Dimension {
        match self {
            Value::Scalar(s) => s.dim,
            Value::Matrix(m) => m.dim,
        }
    }

    /// The real quantity this value is, if it is a real scalar.
    pub fn as_real(&self) -> Option<Quantity> {
        match self {
            Value::Scalar(s) => s.as_real(),
            Value::Matrix(_) => None,
        }
    }

    pub fn negated(&self) -> Value {
        match self {
            Value::Scalar(s) => Value::Scalar(s.negated()),
            Value::Matrix(m) => Value::Matrix(m.map(|z| -z)),
        }
    }

    pub fn add(&self, o: &Value) -> Result<Value, ValueError> {
        self.sum(o, false)
    }

    pub fn sub(&self, o: &Value) -> Result<Value, ValueError> {
        self.sum(o, true)
    }

    fn sum(&self, o: &Value, subtract: bool) -> Result<Value, ValueError> {
        match (self, o) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.sum(*b, subtract)?)),
            (Value::Matrix(a), Value::Matrix(b)) => Ok(Value::Matrix(a.sum(b, subtract)?)),
            _ => Err(ValueError::Shape(
                "cannot add or subtract a scalar and a matrix".into(),
            )),
        }
    }

    pub fn mul(&self, o: &Value) -> Result<Value, ValueError> {
        match (self, o) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.times(*b)?)),
            (Value::Scalar(s), Value::Matrix(m)) | (Value::Matrix(m), Value::Scalar(s)) => {
                Ok(Value::Matrix(m.scaled(*s)?))
            }
            (Value::Matrix(a), Value::Matrix(b)) => Ok(Value::Matrix(a.product(b)?)),
        }
    }

    pub fn div(&self, o: &Value) -> Result<Value, ValueError> {
        match (self, o) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.over(*b)?)),
            (Value::Matrix(m), Value::Scalar(s)) => Ok(Value::Matrix(m.scaled(s.reciprocal()?)?)),
            _ => Err(ValueError::Unsupported("division by a matrix".into())),
        }
    }

    pub fn pow(&self, e: &Value) -> Result<Value, ValueError> {
        match (self, e) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.pow(*b)?)),
            _ => Err(ValueError::Unsupported("a matrix power".into())),
        }
    }
}
