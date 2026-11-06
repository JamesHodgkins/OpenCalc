//! Matrices and vectors: row-major flat storage, one dimension for all elements.

use super::{Scalar, ValueError};
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};

/// Largest matrix accepted, so no input can ask for unbounded memory or time.
pub const MAX_ELEMENTS: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<Complex64>,
    pub dim: Dimension,
}

fn shape_error(msg: &str) -> ValueError {
    ValueError::Shape(msg.to_owned())
}

impl Matrix {
    pub fn new(
        rows: usize,
        cols: usize,
        data: Vec<Complex64>,
        dim: Dimension,
    ) -> Result<Matrix, ValueError> {
        if rows == 0 || cols == 0 {
            return Err(shape_error("a matrix needs at least one row and column"));
        }
        if rows.checked_mul(cols).is_none_or(|n| n > MAX_ELEMENTS) {
            return Err(ValueError::Shape(format!(
                "a matrix may have at most {MAX_ELEMENTS} elements"
            )));
        }
        if data.len() != rows * cols {
            return Err(shape_error("the number of elements does not fit the shape"));
        }
        Ok(Matrix {
            rows,
            cols,
            data,
            dim,
        })
    }

    /// A column vector from scalars of one dimension.
    pub fn column(items: &[Scalar]) -> Result<Matrix, ValueError> {
        Matrix::from_scalars(items.len(), 1, items)
    }

    /// Row-major elements, all of the same dimension.
    pub fn from_scalars(rows: usize, cols: usize, items: &[Scalar]) -> Result<Matrix, ValueError> {
        let dim = items.first().map_or(Dimension::NONE, |s| s.dim);
        if let Some(bad) = items.iter().find(|s| s.dim != dim) {
            return Err(UnitError::Mismatch(dim, bad.dim).into());
        }
        Matrix::new(rows, cols, items.iter().map(|s| s.z).collect(), dim)
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn is_vector(&self) -> bool {
        self.cols == 1
    }

    pub fn at(&self, r: usize, c: usize) -> Complex64 {
        self.data[r * self.cols + c]
    }

    pub fn elements(&self) -> &[Complex64] {
        &self.data
    }

    pub fn is_finite(&self) -> bool {
        self.data.iter().all(|z| crate::numeric::is_finite(*z))
    }

    pub fn map(&self, f: impl Fn(Complex64) -> Complex64) -> Matrix {
        Matrix {
            data: self.data.iter().map(|z| f(*z)).collect(),
            ..self.clone()
        }
    }

    pub fn sum(&self, o: &Matrix, subtract: bool) -> Result<Matrix, ValueError> {
        if self.dim != o.dim {
            return Err(UnitError::Mismatch(self.dim, o.dim).into());
        }
        if (self.rows, self.cols) != (o.rows, o.cols) {
            return Err(shape_error("matrices must have the same shape to add"));
        }
        let data = self
            .data
            .iter()
            .zip(&o.data)
            .map(|(a, b)| if subtract { a - b } else { a + b })
            .collect();
        Matrix::new(self.rows, self.cols, data, self.dim)
    }

    pub fn scaled(&self, s: Scalar) -> Result<Matrix, ValueError> {
        let dim = self
            .dim
            .checked_times(s.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        Matrix::new(
            self.rows,
            self.cols,
            self.data.iter().map(|z| z * s.z).collect(),
            dim,
        )
    }

    pub fn product(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if self.cols != o.rows {
            return Err(ValueError::Shape(format!(
                "cannot multiply a {}x{} matrix by a {}x{} matrix",
                self.rows, self.cols, o.rows, o.cols
            )));
        }
        let dim = self
            .dim
            .checked_times(o.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        let mut data = Vec::with_capacity(self.rows * o.cols);
        for r in 0..self.rows {
            for c in 0..o.cols {
                data.push((0..self.cols).map(|k| self.at(r, k) * o.at(k, c)).sum());
            }
        }
        Matrix::new(self.rows, o.cols, data, dim)
    }

    pub fn transposed(&self) -> Matrix {
        let mut data = Vec::with_capacity(self.data.len());
        for c in 0..self.cols {
            for r in 0..self.rows {
                data.push(self.at(r, c));
            }
        }
        Matrix {
            rows: self.cols,
            cols: self.rows,
            data,
            dim: self.dim,
        }
    }

    /// Sum of products of two vectors of the same length.
    pub fn dot(&self, o: &Matrix) -> Result<Scalar, ValueError> {
        if !self.is_vector() || !o.is_vector() || self.rows != o.rows {
            return Err(shape_error("dot needs two vectors of the same length"));
        }
        let dim = self
            .dim
            .checked_times(o.dim)
            .ok_or(UnitError::DimensionOverflow)?;
        let z = self.data.iter().zip(&o.data).map(|(a, b)| a * b).sum();
        Ok(Scalar::new(z, dim))
    }

    /// Determinant by Gaussian elimination with partial pivoting.
    pub fn determinant(&self) -> Result<Scalar, ValueError> {
        if self.rows != self.cols {
            return Err(shape_error("the determinant needs a square matrix"));
        }
        let n = self.rows;
        let dim = i8::try_from(n)
            .ok()
            .and_then(|n| self.dim.checked_powi(n))
            .ok_or(UnitError::DimensionOverflow)?;
        let mut a = self.data.clone();
        let mut det = Complex64::new(1.0, 0.0);
        for col in 0..n {
            let pivot = (col..n)
                .max_by(|&x, &y| a[x * n + col].norm().total_cmp(&a[y * n + col].norm()))
                .unwrap_or(col);
            let p = a[pivot * n + col];
            if p.norm() == 0.0 {
                return Ok(Scalar::new(Complex64::new(0.0, 0.0), dim));
            }
            if pivot != col {
                for k in 0..n {
                    a.swap(pivot * n + k, col * n + k);
                }
                det = -det;
            }
            det *= p;
            for r in col + 1..n {
                let f = a[r * n + col] / p;
                for k in col..n {
                    let sub = f * a[col * n + k];
                    a[r * n + k] -= sub;
                }
            }
        }
        Ok(Scalar::new(det, dim))
    }

    /// Euclidean norm of every element, in this matrix's dimension.
    pub fn norm(&self) -> Scalar {
        let parts = self.data.iter().flat_map(|z| [z.re, z.im]);
        Scalar::new(Complex64::new(numeric::length(parts), 0.0), self.dim)
    }
}
