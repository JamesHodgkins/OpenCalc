//! Matrix operations beyond the basic arithmetic: identity, trace, rank, inverse,
//! integer powers, cross product, and joining matrices (roadmap Phase 1.2).
//!
//! Elimination uses partial pivoting and one tolerance for "is this pivot zero":
//! `largest element * max(rows, cols) * f64::EPSILON`. A matrix whose pivot falls
//! at or below it is treated as singular (inverse) or loses a rank (rank). The
//! tolerance is relative, so it does not depend on the unit the matrix is in.

use super::{shape_error, Matrix, MAX_ELEMENTS};
use crate::numeric::Complex64;
use crate::units::{Dimension, UnitError};
use crate::value::{Scalar, ValueError};

/// Largest integer power of a matrix (by repeated squaring, so about 20 products).
pub const MAX_POWER: i64 = 1 << 20;

fn zero() -> Complex64 {
    Complex64::new(0.0, 0.0)
}

impl Matrix {
    /// The `n` by `n` identity matrix (dimensionless).
    pub fn identity(n: usize) -> Result<Matrix, ValueError> {
        if n.checked_mul(n).is_none_or(|e| e > MAX_ELEMENTS) {
            return Err(ValueError::Shape(format!(
                "a matrix may have at most {MAX_ELEMENTS} elements"
            )));
        }
        let mut data = vec![zero(); n * n];
        for i in 0..n {
            data[i * n + i] = Complex64::new(1.0, 0.0);
        }
        Matrix::new(n, n, data, Dimension::NONE)
    }

    /// Sum of the diagonal of a square matrix.
    pub fn trace(&self) -> Result<Scalar, ValueError> {
        if self.rows != self.cols {
            return Err(shape_error("the trace needs a square matrix"));
        }
        let z = (0..self.rows).map(|i| self.at(i, i)).sum();
        Ok(Scalar::new(z, self.dim))
    }

    fn pivot_tolerance(&self) -> f64 {
        let peak = self.data.iter().map(|z| z.norm()).fold(0.0, f64::max);
        peak * self.rows.max(self.cols) as f64 * f64::EPSILON
    }

    /// Number of independent rows, by elimination with the tolerance above.
    pub fn rank(&self) -> usize {
        let (m, n) = (self.rows, self.cols);
        let tol = self.pivot_tolerance();
        let mut a = self.data.clone();
        let mut rank = 0;
        for col in 0..n {
            if rank == m {
                break;
            }
            let pivot = (rank..m)
                .max_by(|&x, &y| a[x * n + col].norm().total_cmp(&a[y * n + col].norm()))
                .unwrap_or(rank);
            if a[pivot * n + col].norm() <= tol {
                continue;
            }
            for k in 0..n {
                a.swap(pivot * n + k, rank * n + k);
            }
            let p = a[rank * n + col];
            for r in rank + 1..m {
                let f = a[r * n + col] / p;
                for k in col..n {
                    let sub = f * a[rank * n + k];
                    a[r * n + k] -= sub;
                }
            }
            rank += 1;
        }
        rank
    }

    /// The inverse, by Gauss-Jordan elimination. Its dimension is the reciprocal.
    pub fn inverse(&self) -> Result<Matrix, ValueError> {
        if self.rows != self.cols {
            return Err(shape_error("the inverse needs a square matrix"));
        }
        let n = self.rows;
        let dim = self
            .dim
            .checked_powi(-1)
            .ok_or(UnitError::DimensionOverflow)?;
        let tol = self.pivot_tolerance();
        let mut a = self.data.clone();
        let mut inv = Matrix::identity(n)?.data;
        for col in 0..n {
            let pivot = (col..n)
                .max_by(|&x, &y| a[x * n + col].norm().total_cmp(&a[y * n + col].norm()))
                .unwrap_or(col);
            if a[pivot * n + col].norm() <= tol {
                return Err(shape_error("the matrix is singular and has no inverse"));
            }
            for k in 0..n {
                a.swap(pivot * n + k, col * n + k);
                inv.swap(pivot * n + k, col * n + k);
            }
            let p = a[col * n + col];
            for k in 0..n {
                a[col * n + k] /= p;
                inv[col * n + k] /= p;
            }
            for r in (0..n).filter(|&r| r != col) {
                let f = a[r * n + col];
                if f == zero() {
                    continue;
                }
                for k in 0..n {
                    let (s1, s2) = (f * a[col * n + k], f * inv[col * n + k]);
                    a[r * n + k] -= s1;
                    inv[r * n + k] -= s2;
                }
            }
        }
        Matrix::new(n, n, inv, dim)
    }

    /// The matrix to an integer power: `0` gives the identity, a negative power
    /// the power of the inverse. The dimension is the dimension to that power.
    pub fn powi(&self, n: i64) -> Result<Matrix, ValueError> {
        if self.rows != self.cols {
            return Err(shape_error("a matrix power needs a square matrix"));
        }
        if n.unsigned_abs() > MAX_POWER as u64 {
            return Err(ValueError::Shape(format!(
                "a matrix power must be between -{MAX_POWER} and {MAX_POWER}"
            )));
        }
        let dim = if self.dim.is_dimensionless() {
            Dimension::NONE
        } else {
            i8::try_from(n)
                .ok()
                .and_then(|n| self.dim.checked_powi(n))
                .ok_or(UnitError::DimensionOverflow)?
        };
        let mut base = Matrix {
            dim: Dimension::NONE,
            ..self.clone()
        };
        if n < 0 {
            base = base.inverse()?;
        }
        let mut result = Matrix::identity(self.rows)?;
        let mut e = n.unsigned_abs();
        while e > 0 {
            if e & 1 == 1 {
                result = result.product(&base)?;
            }
            e >>= 1;
            if e > 0 {
                base = base.product(&base)?;
            }
        }
        result.dim = dim;
        Ok(result)
    }

    /// Cross product of two 3-element column vectors.
    pub fn cross(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if !self.is_vector() || !o.is_vector() || self.rows != 3 || o.rows != 3 {
            return Err(shape_error(
                "the cross product needs two vectors of 3 elements",
            ));
        }
        let dim = self
            .dim
            .checked_times(o.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        let (a, b) = (&self.data, &o.data);
        let data = vec![
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        Matrix::new(3, 1, data, dim)
    }

    /// `self` with `o` placed to its right; both need the same number of rows.
    pub fn augment(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if self.dim != o.dim {
            return Err(UnitError::Mismatch(self.dim, o.dim).into());
        }
        if self.rows != o.rows {
            return Err(shape_error("augment needs matrices with the same rows"));
        }
        let mut data = Vec::with_capacity(self.data.len() + o.data.len());
        for r in 0..self.rows {
            data.extend_from_slice(&self.data[r * self.cols..(r + 1) * self.cols]);
            data.extend_from_slice(&o.data[r * o.cols..(r + 1) * o.cols]);
        }
        Matrix::new(self.rows, self.cols + o.cols, data, self.dim)
    }

    /// `self` with `o` placed below it; both need the same number of columns.
    pub fn stack(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if self.dim != o.dim {
            return Err(UnitError::Mismatch(self.dim, o.dim).into());
        }
        if self.cols != o.cols {
            return Err(shape_error("stack needs matrices with the same columns"));
        }
        let mut data = self.data.clone();
        data.extend_from_slice(&o.data);
        Matrix::new(self.rows + o.rows, self.cols, data, self.dim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(rows: usize, cols: usize, items: &[f64]) -> Matrix {
        let s: Vec<Scalar> = items
            .iter()
            .map(|x| Scalar::new(Complex64::new(*x, 0.0), Dimension::NONE))
            .collect();
        Matrix::from_scalars(rows, cols, &s).unwrap()
    }

    fn close(a: &Matrix, b: &Matrix) -> bool {
        a.rows == b.rows
            && a.cols == b.cols
            && a.dim == b.dim
            && a.data
                .iter()
                .zip(&b.data)
                .all(|(x, y)| (x - y).norm() < 1e-12)
    }

    #[test]
    fn the_inverse_of_a_known_matrix() {
        // [[4, 7], [2, 6]] has determinant 10, so the inverse is [[0.6, -0.7], [-0.2, 0.4]].
        let inv = m(2, 2, &[4.0, 7.0, 2.0, 6.0]).inverse().unwrap();
        assert!(close(&inv, &m(2, 2, &[0.6, -0.7, -0.2, 0.4])));
    }

    #[test]
    fn a_matrix_times_its_inverse_is_the_identity() {
        let a = m(3, 3, &[2.0, -1.0, 0.0, -1.0, 2.0, -1.0, 0.0, -1.0, 2.0]);
        let product = a.product(&a.inverse().unwrap()).unwrap();
        assert!(close(&product, &Matrix::identity(3).unwrap()));
    }

    #[test]
    fn a_singular_matrix_has_no_inverse() {
        assert!(m(2, 2, &[1.0, 2.0, 2.0, 4.0]).inverse().is_err());
        assert!(m(2, 2, &[0.0, 0.0, 0.0, 0.0]).inverse().is_err());
        assert!(m(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).inverse().is_err());
    }

    #[test]
    fn the_inverse_has_the_reciprocal_dimension() {
        let a = Matrix::new(1, 1, vec![Complex64::new(4.0, 0.0)], Dimension::LENGTH).unwrap();
        let inv = a.inverse().unwrap();
        assert_eq!(inv.dim, Dimension::LENGTH.checked_powi(-1).unwrap());
        assert!((inv.data[0].re - 0.25).abs() < 1e-15);
    }

    #[test]
    fn powers_by_squaring_agree_with_repeated_products() {
        let a = m(2, 2, &[1.0, 1.0, 1.0, 0.0]);
        // The Fibonacci matrix: a^10 = [[89, 55], [55, 34]].
        assert!(close(
            &a.powi(10).unwrap(),
            &m(2, 2, &[89.0, 55.0, 55.0, 34.0])
        ));
        assert!(close(&a.powi(0).unwrap(), &Matrix::identity(2).unwrap()));
        assert!(close(&a.powi(1).unwrap(), &a));
        let back = a.powi(-3).unwrap().product(&a.powi(3).unwrap()).unwrap();
        assert!(close(&back, &Matrix::identity(2).unwrap()));
    }

    #[test]
    fn a_power_of_a_dimensioned_matrix_has_the_dimension_to_that_power() {
        let a = Matrix::new(
            2,
            2,
            vec![
                Complex64::new(1.0, 0.0),
                zero(),
                zero(),
                Complex64::new(2.0, 0.0),
            ],
            Dimension::LENGTH,
        )
        .unwrap();
        assert_eq!(
            a.powi(3).unwrap().dim,
            Dimension::LENGTH.checked_powi(3).unwrap()
        );
        assert!(a.powi(MAX_POWER).is_err(), "dimension exponent overflows");
    }

    #[test]
    fn powers_are_bounded() {
        let a = m(1, 1, &[1.0]);
        assert!(a.powi(MAX_POWER).is_ok());
        assert!(a.powi(MAX_POWER + 1).is_err());
        assert!(a.powi(i64::MIN).is_err());
        assert!(m(2, 1, &[1.0, 2.0]).powi(2).is_err());
    }

    #[test]
    fn rank_counts_independent_rows() {
        assert_eq!(m(2, 2, &[1.0, 2.0, 3.0, 4.0]).rank(), 2);
        assert_eq!(m(2, 2, &[1.0, 2.0, 2.0, 4.0]).rank(), 1);
        assert_eq!(m(2, 2, &[0.0, 0.0, 0.0, 0.0]).rank(), 0);
        assert_eq!(m(2, 3, &[1.0, 0.0, 1.0, 0.0, 1.0, 1.0]).rank(), 2);
        assert_eq!(m(3, 2, &[1.0, 2.0, 2.0, 4.0, 3.0, 6.0]).rank(), 1);
        // Scaling the whole matrix does not change its rank (the tolerance is relative).
        assert_eq!(m(2, 2, &[1e-9, 2e-9, 2e-9, 4e-9]).rank(), 1);
        assert_eq!(m(2, 2, &[1e9, 2e9, 3e9, 4e9]).rank(), 2);
    }

    #[test]
    fn trace_and_identity() {
        assert_eq!(m(2, 2, &[1.0, 2.0, 3.0, 4.0]).trace().unwrap().z.re, 5.0);
        assert!(m(2, 3, &[1.0; 6]).trace().is_err());
        assert_eq!(Matrix::identity(3).unwrap().trace().unwrap().z.re, 3.0);
        assert!(
            Matrix::identity(65).is_err(),
            "65 x 65 is over the element limit"
        );
        assert!(Matrix::identity(usize::MAX).is_err());
    }

    #[test]
    fn cross_product_follows_the_right_hand_rule() {
        let x = m(3, 1, &[1.0, 0.0, 0.0]);
        let y = m(3, 1, &[0.0, 1.0, 0.0]);
        assert!(close(&x.cross(&y).unwrap(), &m(3, 1, &[0.0, 0.0, 1.0])));
        assert!(close(&y.cross(&x).unwrap(), &m(3, 1, &[0.0, 0.0, -1.0])));
        assert!(x.cross(&m(2, 1, &[1.0, 2.0])).is_err());
    }

    #[test]
    fn augment_and_stack_join_matrices() {
        let a = m(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let b = m(2, 1, &[5.0, 6.0]);
        assert!(close(
            &a.augment(&b).unwrap(),
            &m(2, 3, &[1.0, 2.0, 5.0, 3.0, 4.0, 6.0])
        ));
        let c = m(1, 2, &[7.0, 8.0]);
        assert!(close(
            &a.stack(&c).unwrap(),
            &m(3, 2, &[1.0, 2.0, 3.0, 4.0, 7.0, 8.0])
        ));
        assert!(a.augment(&c).is_err());
        assert!(a.stack(&b).is_err());
        let length = Matrix::new(2, 1, vec![zero(), zero()], Dimension::LENGTH).unwrap();
        assert!(
            a.augment(&length).is_err(),
            "different dimensions do not join"
        );
    }
}
