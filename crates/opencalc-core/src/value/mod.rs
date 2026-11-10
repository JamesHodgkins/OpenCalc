//! Values the evaluator produces: a scalar (complex number with a dimension), a
//! matrix of them, or a series (a list combined element by element, which is what
//! a range gives). A vector is a matrix with one column. Real numbers are scalars
//! with a zero imaginary part, so there is one arithmetic, not two.

mod matrix;
mod scalar;
mod series;

pub use matrix::{Matrix, MAX_ELEMENTS};
pub use scalar::Scalar;
pub use series::Series;

use crate::numeric::Complex64;
use crate::units::{Dimension, Quantity, UnitError};

/// Which comparison to make between two scalars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

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
    Series(Series),
    /// What a function definition evaluates to: the function itself has no number,
    /// only the names of its parameters. Never an operand.
    Function {
        params: Vec<String>,
    },
    /// Text: `"like this"`, or the message of an error caught by `on error`. It can
    /// be joined with `+` and compared with `==` and `!=`, and nothing else yet.
    Text(String),
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

/// What to do with two scalars, element by element.
type Pairwise<'a> = &'a dyn Fn(Scalar, Scalar) -> Result<Scalar, ValueError>;

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
            Value::Series(r) => r.is_finite(),
            Value::Function { .. } | Value::Text(_) => true,
        }
    }

    /// The dimension every element has, or `None` for a matrix whose elements
    /// differ (a vector of a length and a time, say).
    pub fn uniform_dim(&self) -> Option<Dimension> {
        match self {
            Value::Scalar(s) => Some(s.dim),
            Value::Matrix(m) => m.uniform_dim(),
            Value::Series(r) => Some(r.dim),
            Value::Function { .. } | Value::Text(_) => Some(Dimension::NONE),
        }
    }

    /// The dimension of each element, in order.
    pub fn dims(&self) -> Vec<Dimension> {
        match self {
            Value::Scalar(s) => vec![s.dim],
            Value::Matrix(m) => (0..m.elements().len()).map(|i| m.dim_of(i)).collect(),
            Value::Series(r) => vec![r.dim; r.len()],
            Value::Function { .. } | Value::Text(_) => Vec::new(),
        }
    }

    /// The real quantity this value is, if it is a real scalar.
    pub fn as_real(&self) -> Option<Quantity> {
        match self {
            Value::Scalar(s) => s.as_real(),
            Value::Matrix(_) | Value::Series(_) | Value::Function { .. } | Value::Text(_) => None,
        }
    }

    pub fn negated(&self) -> Value {
        match self {
            Value::Scalar(s) => Value::Scalar(s.negated()),
            Value::Matrix(m) => Value::Matrix(m.map(|z| -z)),
            Value::Series(r) => Value::Series(
                Series::new(r.elements().iter().map(|z| -*z).collect(), r.dim)
                    .expect("negation keeps the length"),
            ),
            Value::Function { .. } | Value::Text(_) => self.clone(),
        }
    }

    /// If either side is a series, combine element by element (a scalar is used
    /// against every element). `None` when neither is.
    fn pairwise(&self, o: &Value, f: Pairwise) -> Option<Result<Value, ValueError>> {
        let series = |r: Result<Series, ValueError>| r.map(Value::Series);
        match (self, o) {
            (Value::Series(a), Value::Series(b)) => Some(series(a.zip(b, f))),
            (Value::Series(a), Value::Scalar(b)) => Some(series(a.map(|x| f(x, *b)))),
            (Value::Scalar(a), Value::Series(b)) => Some(series(b.map(|x| f(*a, x)))),
            (Value::Series(_), Value::Matrix(_)) | (Value::Matrix(_), Value::Series(_)) => {
                Some(Err(ValueError::Shape(
                    "a list of values cannot be combined with a matrix".into(),
                )))
            }
            _ => None,
        }
    }

    pub fn add(&self, o: &Value) -> Result<Value, ValueError> {
        self.sum(o, false)
    }

    pub fn sub(&self, o: &Value) -> Result<Value, ValueError> {
        self.sum(o, true)
    }

    fn sum(&self, o: &Value, subtract: bool) -> Result<Value, ValueError> {
        if let Some(r) = self.pairwise(o, &|a, b| a.sum(b, subtract)) {
            return r;
        }
        match (self, o) {
            // `+` joins text; there is nothing to take away from it.
            (Value::Text(a), Value::Text(b)) if !subtract => Ok(Value::Text(format!("{a}{b}"))),
            (Value::Text(_), _) | (_, Value::Text(_)) => Err(ValueError::Unsupported(
                "arithmetic on text (only text + text joins it)".into(),
            )),
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.sum(*b, subtract)?)),
            (Value::Matrix(a), Value::Matrix(b)) => Ok(Value::Matrix(a.sum(b, subtract)?)),
            _ => Err(ValueError::Shape(
                "cannot add or subtract a scalar and a matrix".into(),
            )),
        }
    }

    pub fn mul(&self, o: &Value) -> Result<Value, ValueError> {
        if let Some(r) = self.pairwise(o, &|a, b| a.times(b)) {
            return r;
        }
        match (self, o) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.times(*b)?)),
            (Value::Scalar(s), Value::Matrix(m)) | (Value::Matrix(m), Value::Scalar(s)) => {
                Ok(Value::Matrix(m.scaled(*s)?))
            }
            (Value::Matrix(a), Value::Matrix(b)) => Ok(Value::Matrix(a.product(b)?)),
            _ => Err(ValueError::Unsupported("multiplying this value".into())),
        }
    }

    pub fn div(&self, o: &Value) -> Result<Value, ValueError> {
        if let Some(r) = self.pairwise(o, &|a, b| a.over(b)) {
            return r;
        }
        match (self, o) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.over(*b)?)),
            (Value::Matrix(m), Value::Scalar(s)) => Ok(Value::Matrix(m.scaled(s.reciprocal()?)?)),
            _ => Err(ValueError::Unsupported("division by a matrix".into())),
        }
    }

    /// 1 if the comparison holds, 0 if not, as a dimensionless number (one per
    /// element for lists).
    pub fn compare(&self, o: &Value, how: Comparison) -> Result<Value, ValueError> {
        let flag = |a: Scalar, b: Scalar| {
            Ok(Scalar::new(
                Complex64::new(f64::from(u8::from(a.compare(b, how)?)), 0.0),
                Dimension::NONE,
            ))
        };
        if let Some(r) = self.pairwise(o, &flag) {
            return r;
        }
        let yes = |same: bool| Value::dimensionless(f64::from(u8::from(same)));
        match (self, o) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(flag(*a, *b)?)),
            (Value::Text(a), Value::Text(b)) if how == Comparison::Eq => Ok(yes(a == b)),
            (Value::Text(a), Value::Text(b)) if how == Comparison::Ne => Ok(yes(a != b)),
            (Value::Text(_), _) | (_, Value::Text(_)) => Err(ValueError::Unsupported(
                "comparing text (only == and != work, and only between two texts)".into(),
            )),
            _ => Err(ValueError::Unsupported("comparing matrices".into())),
        }
    }

    /// Element `indices` (zero-based) of a matrix, vector or list. One index
    /// addresses a vector or list; two address a matrix.
    pub fn element(&self, indices: &[usize]) -> Result<Value, ValueError> {
        match (self, indices) {
            (Value::Matrix(m), _) => m.element(indices).map(Value::Scalar),
            (Value::Series(r), [i]) => r.at(*i).map(Value::Scalar).ok_or_else(|| {
                ValueError::Shape(format!("index out of range for {} values", r.len()))
            }),
            (Value::Series(_), _) => {
                Err(ValueError::Shape("a list of values takes one index".into()))
            }
            (Value::Scalar(_) | Value::Function { .. } | Value::Text(_), _) => Err(
                ValueError::Shape("only a matrix or list can be indexed".into()),
            ),
        }
    }

    pub fn pow(&self, e: &Value) -> Result<Value, ValueError> {
        if let Some(r) = self.pairwise(e, &|a, b| a.pow(b)) {
            return r;
        }
        match (self, e) {
            (Value::Scalar(a), Value::Scalar(b)) => Ok(Value::Scalar(a.pow(*b)?)),
            (Value::Matrix(m), Value::Scalar(e)) => {
                let n = e
                    .as_real()
                    .filter(|q| {
                        q.dim.is_dimensionless() && q.si.fract() == 0.0 && q.si.abs() < 1e18
                    })
                    .ok_or_else(|| {
                        ValueError::Unsupported("a matrix power that is not a whole number".into())
                    })?;
                Ok(Value::Matrix(m.powi(n.si as i64)?))
            }
            _ => Err(ValueError::Unsupported("a matrix power".into())),
        }
    }
}
