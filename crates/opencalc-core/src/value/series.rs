//! A list of numbers that share a dimension and are combined element by element:
//! what a range such as `0..10` produces, and what any expression that uses one
//! produces in turn. Unlike a matrix, `*` and `^` act on each element.

use super::matrix::MAX_ELEMENTS;
use super::{Scalar, ValueError};
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};

#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    data: Vec<Complex64>,
    pub dim: Dimension,
}

fn shape(msg: &str) -> ValueError {
    ValueError::Shape(msg.to_owned())
}

impl Series {
    pub fn new(data: Vec<Complex64>, dim: Dimension) -> Result<Series, ValueError> {
        if data.is_empty() {
            return Err(shape("a list of values needs at least one value"));
        }
        if data.len() > MAX_ELEMENTS {
            return Err(ValueError::Shape(format!(
                "a list of values may have at most {MAX_ELEMENTS} values"
            )));
        }
        Ok(Series { data, dim })
    }

    /// From scalars, which must all have the same dimension.
    pub fn from_scalars(items: &[Scalar]) -> Result<Series, ValueError> {
        let dim = items.first().map_or(Dimension::NONE, |s| s.dim);
        if let Some(bad) = items.iter().find(|s| s.dim != dim) {
            return Err(UnitError::Mismatch(dim, bad.dim).into());
        }
        Series::new(items.iter().map(|s| s.z).collect(), dim)
    }

    /// `start..end`, or `start, next..end` where the step is `next - start`. All
    /// must be real and share a dimension. The default step is one SI unit,
    /// towards `end`.
    pub fn range(start: Scalar, next: Option<Scalar>, end: Scalar) -> Result<Series, ValueError> {
        let parts = [Some(start), next, Some(end)];
        for s in parts.iter().flatten() {
            if !s.is_real() {
                return Err(ValueError::Unsupported("a range of complex numbers".into()));
            }
            if s.dim != start.dim {
                return Err(UnitError::Mismatch(start.dim, s.dim).into());
            }
        }
        let (a, b) = (start.z.re, end.z.re);
        let step = next.map_or(if b >= a { 1.0 } else { -1.0 }, |s| s.z.re - a);
        if step == 0.0 {
            return Err(shape("a range cannot have a step of zero"));
        }
        let steps = (b - a) / step;
        if steps < -1e-9 {
            return Err(shape("the step of a range points away from its end"));
        }
        // Rounding slack, so 0..0.3 in steps of 0.1 includes 0.3.
        let count = (steps + 1e-9).floor() + 1.0;
        if !count.is_finite() || count > MAX_ELEMENTS as f64 {
            return Err(ValueError::Shape(format!(
                "a range may have at most {MAX_ELEMENTS} values"
            )));
        }
        let data = (0..count as usize)
            .map(|k| numeric::real(a + step * k as f64))
            .collect();
        Series::new(data, start.dim)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn elements(&self) -> &[Complex64] {
        &self.data
    }

    pub fn at(&self, i: usize) -> Option<Scalar> {
        self.data.get(i).map(|z| Scalar::new(*z, self.dim))
    }

    pub fn is_finite(&self) -> bool {
        self.data.iter().all(|z| numeric::is_finite(*z))
    }

    pub fn scalars(&self) -> impl Iterator<Item = Scalar> + '_ {
        self.data.iter().map(|z| Scalar::new(*z, self.dim))
    }

    /// Every element negated. The length and dimension are unchanged, so this cannot fail.
    pub fn negated(&self) -> Series {
        Series {
            data: self.data.iter().map(|z| -*z).collect(),
            dim: self.dim,
        }
    }

    /// Apply `f` to every element.
    pub fn map(
        &self,
        f: impl Fn(Scalar) -> Result<Scalar, ValueError>,
    ) -> Result<Series, ValueError> {
        let out = self.scalars().map(f).collect::<Result<Vec<_>, _>>()?;
        Series::from_scalars(&out)
    }

    /// Combine with another series of the same length, element by element.
    pub fn zip(
        &self,
        o: &Series,
        f: impl Fn(Scalar, Scalar) -> Result<Scalar, ValueError>,
    ) -> Result<Series, ValueError> {
        if self.len() != o.len() {
            return Err(ValueError::Shape(format!(
                "lists of {} and {} values cannot be combined element by element",
                self.len(),
                o.len()
            )));
        }
        let out = self
            .scalars()
            .zip(o.scalars())
            .map(|(a, b)| f(a, b))
            .collect::<Result<Vec<_>, _>>()?;
        Series::from_scalars(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: f64) -> Scalar {
        Scalar::new(numeric::real(v), Dimension::NONE)
    }

    fn values(r: &Series) -> Vec<f64> {
        r.elements().iter().map(|z| z.re).collect()
    }

    #[test]
    fn a_range_counts_up_by_one_by_default_and_includes_its_end() {
        assert_eq!(
            values(&Series::range(s(0.0), None, s(4.0)).unwrap()),
            [0.0, 1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(
            values(&Series::range(s(3.0), None, s(1.0)).unwrap()),
            [3.0, 2.0, 1.0]
        );
        assert_eq!(values(&Series::range(s(2.0), None, s(2.0)).unwrap()), [2.0]);
    }

    #[test]
    fn the_second_value_sets_the_step() {
        let r = Series::range(s(0.0), Some(s(2.0)), s(7.0)).unwrap();
        assert_eq!(values(&r), [0.0, 2.0, 4.0, 6.0]);
        let down = Series::range(s(10.0), Some(s(7.5)), s(0.0)).unwrap();
        assert_eq!(values(&down), [10.0, 7.5, 5.0, 2.5, 0.0]);
    }

    #[test]
    fn rounding_does_not_lose_the_last_value() {
        let r = Series::range(s(0.0), Some(s(0.1)), s(0.3)).unwrap();
        assert_eq!(r.len(), 4);
        // Values come from start + k * step, not from repeated addition.
        assert_eq!(r.elements()[3].re, 0.0 + 0.1 * 3.0);
    }

    #[test]
    fn bad_ranges_are_errors() {
        assert!(Series::range(s(0.0), Some(s(0.0)), s(5.0)).is_err());
        assert!(Series::range(s(0.0), Some(s(-1.0)), s(5.0)).is_err());
        assert!(Series::range(s(0.0), None, s(1e9)).is_err());
        assert!(Series::range(s(0.0), Some(s(1e-12)), s(1.0)).is_err());
        let m = Scalar::new(numeric::real(1.0), Dimension::LENGTH);
        assert!(Series::range(s(0.0), None, m).is_err());
        let z = Scalar::new(Complex64::new(0.0, 1.0), Dimension::NONE);
        assert!(Series::range(z, None, s(3.0)).is_err());
    }

    #[test]
    fn element_wise_combination_needs_equal_lengths() {
        let a = Series::range(s(0.0), None, s(2.0)).unwrap();
        let b = Series::range(s(0.0), None, s(3.0)).unwrap();
        assert!(a.zip(&b, |x, y| x.sum(y, false)).is_err());
        let sum = a.zip(&a, |x, y| x.sum(y, false)).unwrap();
        assert_eq!(values(&sum), [0.0, 2.0, 4.0]);
    }
}
