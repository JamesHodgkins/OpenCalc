//! A complex number with a dimension, in SI.

use super::{Comparison, ValueError};
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, Quantity, UnitError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scalar {
    pub z: Complex64,
    pub dim: Dimension,
}

impl From<Quantity> for Scalar {
    fn from(q: Quantity) -> Scalar {
        Scalar::new(numeric::real(q.si), q.dim)
    }
}

impl Scalar {
    pub fn new(z: Complex64, dim: Dimension) -> Scalar {
        Scalar { z, dim }
    }

    pub fn is_finite(&self) -> bool {
        numeric::is_finite(self.z)
    }

    pub fn is_real(&self) -> bool {
        numeric::is_real(self.z)
    }

    pub fn as_real(&self) -> Option<Quantity> {
        self.is_real().then_some(Quantity {
            si: self.z.re,
            dim: self.dim,
        })
    }

    pub fn negated(&self) -> Scalar {
        Scalar::new(-self.z, self.dim)
    }

    fn same_dim(&self, o: &Scalar) -> Result<Dimension, UnitError> {
        if self.dim == o.dim {
            Ok(self.dim)
        } else {
            Err(UnitError::Mismatch(self.dim, o.dim))
        }
    }

    pub fn sum(self, o: Scalar, subtract: bool) -> Result<Scalar, ValueError> {
        let dim = self.same_dim(&o)?;
        let z = if subtract { self.z - o.z } else { self.z + o.z };
        Ok(Scalar::new(z, dim))
    }

    pub fn times(self, o: Scalar) -> Result<Scalar, ValueError> {
        let dim = self
            .dim
            .checked_times(o.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        Ok(Scalar::new(self.z * o.z, dim))
    }

    pub fn over(self, o: Scalar) -> Result<Scalar, ValueError> {
        let dim = self
            .dim
            .checked_over(o.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        Ok(Scalar::new(numeric::div(self.z, o.z), dim))
    }

    pub fn reciprocal(self) -> Result<Scalar, ValueError> {
        Scalar::new(numeric::real(1.0), Dimension::NONE).over(self)
    }

    /// An integer exponent keeps dimensions; any other exponent needs a
    /// dimensionless base. The exponent itself must be dimensionless.
    pub fn pow(self, e: Scalar) -> Result<Scalar, ValueError> {
        if !e.dim.is_dimensionless() {
            return Err(UnitError::NotDimensionless("^".into()).into());
        }
        let integer = e.is_real() && e.z.re.fract() == 0.0 && e.z.re.abs() <= f64::from(i8::MAX);
        if integer {
            let dim = self
                .dim
                .checked_powi(e.z.re as i8)
                .ok_or(UnitError::DimensionOverflow)?;
            return Ok(Scalar::new(numeric::pow(self.z, e.z), dim));
        }
        if !self.dim.is_dimensionless() {
            return Err(UnitError::NotDimensionless("^".into()).into());
        }
        Ok(Scalar::new(numeric::pow(self.z, e.z), Dimension::NONE))
    }

    /// Compare two scalars. Both must have the same dimension. Equality is exact
    /// (without `TOL`); ordering needs real numbers.
    pub fn compare(self, o: Scalar, how: Comparison) -> Result<bool, ValueError> {
        self.same_dim(&o)?;
        match how {
            Comparison::Eq => Ok(self.z == o.z),
            Comparison::Ne => Ok(self.z != o.z),
            ordering => {
                if !self.is_real() || !o.is_real() {
                    return Err(ValueError::Unsupported("ordering complex numbers".into()));
                }
                let (a, b) = (self.z.re, o.z.re);
                Ok(match ordering {
                    Comparison::Lt => a < b,
                    Comparison::Gt => a > b,
                    Comparison::Le => a <= b,
                    _ => a >= b,
                })
            }
        }
    }

    /// Principal square root; the dimension must have even exponents.
    pub fn sqrt(self) -> Result<Scalar, ValueError> {
        let dim = self
            .dim
            .halved()
            .ok_or_else(|| UnitError::NotDimensionless("sqrt".into()))?;
        Ok(Scalar::new(numeric::sqrt(self.z), dim))
    }
}
