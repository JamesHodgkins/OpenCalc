//! Matrix operations beyond the basic arithmetic: identity, trace, rank, inverse,
//! integer powers, cross product, and joining matrices (roadmap Phase 1.2).
//!
//! Elimination uses partial pivoting and one tolerance for "is this pivot zero":
//! `largest element * max(rows, cols) * f64::EPSILON`. A matrix whose pivot falls
//! at or below it is treated as singular (inverse) or loses a rank (rank). The
//! tolerance is relative, so it does not depend on the unit the matrix is in.
//!
//! Trace, rank, inverse and powers need one dimension throughout and refuse a mixed
//! matrix (a unit mismatch). Cross product, `augment` and `stack` work with mixed
//! dimensions, element by element.

use super::{shape_error, Dims, Matrix, MAX_ELEMENTS};
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
        let dim = self.common_dim()?;
        let z = (0..self.rows).map(|i| self.at(i, i)).sum();
        Ok(Scalar::new(z, dim))
    }

    fn pivot_tolerance(&self) -> f64 {
        let peak = self.data.iter().map(|z| z.norm()).fold(0.0, f64::max);
        peak * self.rows.max(self.cols) as f64 * f64::EPSILON
    }

    /// Number of independent rows, by elimination with the tolerance above. The
    /// answer depends on how the elements are scaled, so the matrix must have one
    /// dimension throughout.
    pub fn rank(&self) -> Result<usize, ValueError> {
        self.common_dim()?;
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
        Ok(rank)
    }

    /// The inverse, by Gauss-Jordan elimination. Its dimension is the reciprocal.
    pub fn inverse(&self) -> Result<Matrix, ValueError> {
        if self.rows != self.cols {
            return Err(shape_error("the inverse needs a square matrix"));
        }
        let n = self.rows;
        let dim = self
            .common_dim()?
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
        let common = self.common_dim()?;
        let dim = if common.is_dimensionless() {
            Dimension::NONE
        } else {
            i8::try_from(n)
                .ok()
                .and_then(|n| common.checked_powi(n))
                .ok_or(UnitError::DimensionOverflow)?
        };
        let mut base = Matrix {
            dims: Dims::Uniform(Dimension::NONE),
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
        result.dims = Dims::Uniform(dim);
        Ok(result)
    }

    /// Cross product of two 3-element column vectors.
    pub fn cross(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if !self.is_vector() || !o.is_vector() || self.rows != 3 || o.rows != 3 {
            return Err(shape_error(
                "the cross product needs two vectors of 3 elements",
            ));
        }
        let (a, b) = (&self.data, &o.data);
        let data = vec![
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        // Component i is a[j] b[k] - a[k] b[j]: both products must have the same
        // dimension for the difference to mean anything.
        let product = |p: usize, q: usize| {
            self.dim_of(p)
                .checked_times(o.dim_of(q))
                .ok_or(UnitError::DimensionOverflow)
        };
        let mut dims = Vec::with_capacity(3);
        for (j, k) in [(1, 2), (2, 0), (0, 1)] {
            let (first, second) = (product(j, k)?, product(k, j)?);
            if first != second {
                return Err(UnitError::Mismatch(first, second).into());
            }
            dims.push(first);
        }
        Matrix::with_dims(3, 1, data, dims)
    }

    /// `self` with `o` placed to its right; both need the same number of rows.
    pub fn augment(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if self.rows != o.rows {
            return Err(shape_error("augment needs matrices with the same rows"));
        }
        let mut data = Vec::with_capacity(self.data.len() + o.data.len());
        let mut dims = Vec::with_capacity(self.data.len() + o.data.len());
        for r in 0..self.rows {
            data.extend_from_slice(&self.data[r * self.cols..(r + 1) * self.cols]);
            dims.extend((0..self.cols).map(|c| self.dim_at(r, c)));
            data.extend_from_slice(&o.data[r * o.cols..(r + 1) * o.cols]);
            dims.extend((0..o.cols).map(|c| o.dim_at(r, c)));
        }
        Matrix::with_dims(self.rows, self.cols + o.cols, data, dims)
    }

    /// `self` with `o` placed below it; both need the same number of columns.
    pub fn stack(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if self.cols != o.cols {
            return Err(shape_error("stack needs matrices with the same columns"));
        }
        let mut data = self.data.clone();
        data.extend_from_slice(&o.data);
        let dims: Vec<Dimension> = (0..self.data.len())
            .map(|i| self.dim_of(i))
            .chain((0..o.data.len()).map(|i| o.dim_of(i)))
            .collect();
        Matrix::with_dims(self.rows + o.rows, self.cols, data, dims)
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
            && a.dims == b.dims
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
        assert_eq!(
            inv.uniform_dim(),
            Some(Dimension::LENGTH.checked_powi(-1).unwrap())
        );
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
            a.powi(3).unwrap().uniform_dim().unwrap(),
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
        assert_eq!(m(2, 2, &[1.0, 2.0, 3.0, 4.0]).rank().unwrap(), 2);
        assert_eq!(m(2, 2, &[1.0, 2.0, 2.0, 4.0]).rank().unwrap(), 1);
        assert_eq!(m(2, 2, &[0.0, 0.0, 0.0, 0.0]).rank().unwrap(), 0);
        assert_eq!(m(2, 3, &[1.0, 0.0, 1.0, 0.0, 1.0, 1.0]).rank().unwrap(), 2);
        assert_eq!(m(3, 2, &[1.0, 2.0, 2.0, 4.0, 3.0, 6.0]).rank().unwrap(), 1);
        // Scaling the whole matrix does not change its rank (the tolerance is relative).
        assert_eq!(m(2, 2, &[1e-9, 2e-9, 2e-9, 4e-9]).rank().unwrap(), 1);
        assert_eq!(m(2, 2, &[1e9, 2e9, 3e9, 4e9]).rank().unwrap(), 2);
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
    }

    /// A column vector with one dimension per element.
    fn mixed(items: &[(f64, Dimension)]) -> Matrix {
        let s: Vec<Scalar> = items
            .iter()
            .map(|(x, d)| Scalar::new(Complex64::new(*x, 0.0), *d))
            .collect();
        Matrix::column(&s).unwrap()
    }

    const L: Dimension = Dimension::LENGTH;
    const T: Dimension = Dimension::TIME;

    #[test]
    fn a_vector_can_mix_dimensions_and_keeps_each() {
        let v = mixed(&[(2.0, L), (3.0, T)]);
        assert_eq!(v.uniform_dim(), None);
        assert_eq!(v.element(&[0]).unwrap().dim, L);
        assert_eq!(v.element(&[1]).unwrap().dim, T);
        assert!(
            v.common_dim().is_err(),
            "operations needing one dimension refuse it"
        );
    }

    #[test]
    fn equal_dimensions_are_stored_as_one_so_equal_matrices_are_equal() {
        let a = mixed(&[(1.0, L), (2.0, L)]);
        let b = Matrix::new(
            2,
            1,
            vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            L,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.uniform_dim(), Some(L));
    }

    #[test]
    fn mixed_vectors_add_only_to_the_same_mix() {
        let v = mixed(&[(1.0, L), (2.0, T)]);
        let w = mixed(&[(3.0, L), (4.0, T)]);
        let sum = v.sum(&w, false).unwrap();
        assert_eq!((sum.at(0, 0).re, sum.at(1, 0).re), (4.0, 6.0));
        assert_eq!(sum.dim_of(0), L);
        assert_eq!(sum.dim_of(1), T);
        let swapped = mixed(&[(3.0, T), (4.0, L)]);
        assert!(v.sum(&swapped, false).is_err());
    }

    #[test]
    fn scaling_a_mixed_vector_scales_each_dimension() {
        let v = mixed(&[(1.0, L), (2.0, T)]);
        let per_time = Scalar::new(Complex64::new(2.0, 0.0), T.checked_powi(-1).unwrap());
        let w = v.scaled(per_time).unwrap();
        assert_eq!(w.dim_of(0), L.checked_over(T).unwrap());
        assert_eq!(w.dim_of(1), Dimension::NONE);
        assert_eq!(w.at(1, 0).re, 4.0);
    }

    #[test]
    fn the_dot_product_of_a_mixed_vector_needs_matching_terms() {
        // (1 m, 2 s) . (3 1/m, 4 1/s) = 3 + 8: both terms are plain numbers.
        let inverse = |d: Dimension| d.checked_powi(-1).unwrap();
        let a = mixed(&[(1.0, L), (2.0, T)]);
        let b = mixed(&[(3.0, inverse(L)), (4.0, inverse(T))]);
        let d = a.dot(&b).unwrap();
        assert_eq!((d.z.re, d.dim), (11.0, Dimension::NONE));
        // (1 m, 2 s) . (3 1/s, 4 1/s): a length-per-time added to a plain number.
        let c = mixed(&[(3.0, inverse(T)), (4.0, inverse(T))]);
        assert!(a.dot(&c).is_err());
    }

    #[test]
    fn a_matrix_times_a_vector_checks_every_term_of_each_sum() {
        // [[1 m/s, 0], [0, 1]] * (2 s, 3): the first row gives 2 m and 0 * 3 -> a length
        // and a plain number would be added in row 1 if the zero had a dimension.
        let a = Matrix::with_dims(
            2,
            2,
            vec![Complex64::new(1.0, 0.0); 4],
            vec![
                L.checked_over(T).unwrap(),
                L.checked_over(T).unwrap(),
                Dimension::NONE,
                Dimension::NONE,
            ],
        )
        .unwrap();
        let v = mixed(&[(2.0, T), (3.0, T)]);
        let p = a.product(&v).unwrap();
        assert_eq!(p.dim_of(0), L);
        assert_eq!(p.dim_of(1), T);
        // A row whose terms have different dimensions cannot be summed.
        let bad = Matrix::with_dims(
            1,
            2,
            vec![Complex64::new(1.0, 0.0); 2],
            vec![Dimension::NONE, L],
        )
        .unwrap();
        assert!(bad.product(&v).is_err());
    }

    #[test]
    fn the_transpose_and_joins_carry_dimensions_along() {
        let v = mixed(&[(1.0, L), (2.0, T)]);
        let t = v.transposed();
        assert_eq!((t.rows(), t.cols()), (1, 2));
        assert_eq!((t.dim_of(0), t.dim_of(1)), (L, T));
        let stacked = v.stack(&mixed(&[(9.0, Dimension::MASS)])).unwrap();
        assert_eq!(stacked.dim_of(2), Dimension::MASS);
        let joined = v.augment(&mixed(&[(5.0, T), (6.0, L)])).unwrap();
        assert_eq!((joined.dim_at(0, 1), joined.dim_at(1, 1)), (T, L));
    }

    #[test]
    fn a_cross_product_needs_each_component_to_make_sense() {
        // Position (m) x force (N) = torque, every component N m.
        let r = mixed(&[(1.0, L), (0.0, L), (0.0, L)]);
        let f = mixed(&[
            (0.0, Dimension::FORCE),
            (1.0, Dimension::FORCE),
            (0.0, Dimension::FORCE),
        ]);
        let torque = r.cross(&f).unwrap();
        assert_eq!(
            torque.uniform_dim(),
            Some(L.checked_times(Dimension::FORCE).unwrap())
        );
        assert_eq!(torque.at(2, 0).re, 1.0);
        // Mixed components are fine when dim(a_i) - dim(b_i) is the same for every
        // i, so that a_j b_k and a_k b_j agree: here both vectors are (m, s, kg).
        let a = mixed(&[(1.0, L), (2.0, T), (3.0, Dimension::MASS)]);
        let b = mixed(&[(4.0, L), (5.0, T), (6.0, Dimension::MASS)]);
        assert!(a.cross(&b).is_ok(), "s kg - kg s");
        let c = mixed(&[(1.0, L), (1.0, L), (1.0, L)]);
        let d = mixed(&[(1.0, T), (1.0, Dimension::MASS), (1.0, T)]);
        assert!(c.cross(&d).is_err(), "m kg - m s");
    }

    #[test]
    fn operations_that_need_one_dimension_refuse_a_mixed_matrix() {
        let square =
            Matrix::with_dims(2, 2, vec![Complex64::new(1.0, 0.0); 4], vec![L, T, L, T]).unwrap();
        assert!(square.inverse().is_err());
        assert!(square.powi(2).is_err());
        assert!(square.trace().is_err());
        assert!(square.rank().is_err());
        assert!(square.determinant().is_err());
        assert!(square.norm().is_err());
    }
}
