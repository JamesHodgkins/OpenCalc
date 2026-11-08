//! Matrices and vectors: row-major flat storage. Each element has its own
//! dimension, so a vector can hold a position and a time, a force and a moment.
//!
//! Almost every matrix has one dimension throughout, and that is stored once. The
//! per-element list exists only while the dimensions actually differ and is never
//! built when they all agree, so equal matrices are equal as values. Operations
//! that only make sense with one dimension (determinant, inverse, powers, rank,
//! trace, statistics) call [`Matrix::common_dim`] and refuse a mixed matrix with a
//! unit-mismatch error.

use super::{Scalar, ValueError};
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use std::sync::Arc;

/// Largest matrix accepted, so no input can ask for unbounded memory or time.
pub const MAX_ELEMENTS: usize = 4096;

/// The dimensions of the elements.
#[derive(Clone, Debug, PartialEq)]
enum Dims {
    Uniform(Dimension),
    /// One per element, row-major. Only when they are not all equal.
    Each(Arc<Vec<Dimension>>),
}

impl Dims {
    /// The canonical form: `Uniform` whenever the dimensions agree.
    fn of(dims: Vec<Dimension>) -> Dims {
        match dims.first() {
            Some(first) if dims.iter().all(|d| d == first) => Dims::Uniform(*first),
            Some(_) => Dims::Each(Arc::new(dims)),
            None => Dims::Uniform(Dimension::NONE),
        }
    }

    fn get(&self, i: usize) -> Dimension {
        match self {
            Dims::Uniform(d) => *d,
            Dims::Each(v) => v[i],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<Complex64>,
    dims: Dims,
}

mod ops;

fn shape_error(msg: &str) -> ValueError {
    ValueError::Shape(msg.to_owned())
}

impl Matrix {
    fn build(
        rows: usize,
        cols: usize,
        data: Vec<Complex64>,
        dims: Dims,
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
            dims,
        })
    }

    /// A matrix whose elements all have dimension `dim`.
    pub fn new(
        rows: usize,
        cols: usize,
        data: Vec<Complex64>,
        dim: Dimension,
    ) -> Result<Matrix, ValueError> {
        Matrix::build(rows, cols, data, Dims::Uniform(dim))
    }

    /// A matrix with one dimension per element (row-major).
    pub fn with_dims(
        rows: usize,
        cols: usize,
        data: Vec<Complex64>,
        dims: Vec<Dimension>,
    ) -> Result<Matrix, ValueError> {
        if dims.len() != data.len() {
            return Err(shape_error("there must be one dimension per element"));
        }
        Matrix::build(rows, cols, data, Dims::of(dims))
    }

    /// A column vector from scalars, which may differ in dimension.
    pub fn column(items: &[Scalar]) -> Result<Matrix, ValueError> {
        Matrix::from_scalars(items.len(), 1, items)
    }

    /// Row-major elements, which may differ in dimension.
    pub fn from_scalars(rows: usize, cols: usize, items: &[Scalar]) -> Result<Matrix, ValueError> {
        Matrix::with_dims(
            rows,
            cols,
            items.iter().map(|s| s.z).collect(),
            items.iter().map(|s| s.dim).collect(),
        )
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

    /// The dimension of element `i` (row-major).
    pub fn dim_of(&self, i: usize) -> Dimension {
        self.dims.get(i)
    }

    pub fn dim_at(&self, r: usize, c: usize) -> Dimension {
        self.dims.get(r * self.cols + c)
    }

    /// The dimension all elements share, or `None` if they differ.
    pub fn uniform_dim(&self) -> Option<Dimension> {
        match self.dims {
            Dims::Uniform(d) => Some(d),
            Dims::Each(_) => None,
        }
    }

    /// The dimension all elements share. A mixed matrix is a unit mismatch: this is
    /// how the operations that need one dimension refuse it.
    pub fn common_dim(&self) -> Result<Dimension, ValueError> {
        match &self.dims {
            Dims::Uniform(d) => Ok(*d),
            Dims::Each(v) => {
                let first = v[0];
                let other = v.iter().find(|d| **d != first).copied().unwrap_or(first);
                Err(UnitError::Mismatch(first, other).into())
            }
        }
    }

    /// The elements with their dimensions.
    pub fn scalars(&self) -> impl Iterator<Item = Scalar> + '_ {
        self.data
            .iter()
            .enumerate()
            .map(|(i, z)| Scalar::new(*z, self.dims.get(i)))
    }

    /// Element at zero-based `indices`: one index for a vector, two for a matrix.
    pub fn element(&self, indices: &[usize]) -> Result<Scalar, ValueError> {
        let (r, c) = match *indices {
            [i] if self.cols == 1 => (i, 0),
            [i] if self.rows == 1 => (0, i),
            [_] => {
                return Err(shape_error(
                    "a matrix needs a row and a column index, like M[i, j]",
                ))
            }
            [r, c] => (r, c),
            _ => return Err(shape_error("an index has one or two numbers")),
        };
        if r >= self.rows || c >= self.cols {
            return Err(ValueError::Shape(format!(
                "index out of range for a {}x{} matrix",
                self.rows, self.cols
            )));
        }
        Ok(Scalar::new(self.at(r, c), self.dim_at(r, c)))
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
        if (self.rows, self.cols) != (o.rows, o.cols) {
            return Err(shape_error("matrices must have the same shape to add"));
        }
        // Element by element: a vector (1 m, 2 s) adds only to one with the same units.
        for i in 0..self.data.len() {
            let (a, b) = (self.dim_of(i), o.dim_of(i));
            if a != b {
                return Err(UnitError::Mismatch(a, b).into());
            }
        }
        let data = self
            .data
            .iter()
            .zip(&o.data)
            .map(|(a, b)| if subtract { a - b } else { a + b })
            .collect();
        Matrix::build(self.rows, self.cols, data, self.dims.clone())
    }

    pub fn scaled(&self, s: Scalar) -> Result<Matrix, ValueError> {
        let scale = |d: &Dimension| d.checked_times(s.dim).ok_or(UnitError::DimensionOverflow);
        let dims = match &self.dims {
            Dims::Uniform(d) => Dims::Uniform(scale(d)?),
            Dims::Each(v) => Dims::of(v.iter().map(scale).collect::<Result<Vec<_>, _>>()?),
        };
        Matrix::build(
            self.rows,
            self.cols,
            self.data.iter().map(|z| z * s.z).collect(),
            dims,
        )
    }

    pub fn product(&self, o: &Matrix) -> Result<Matrix, ValueError> {
        if self.cols != o.rows {
            return Err(ValueError::Shape(format!(
                "cannot multiply a {}x{} matrix by a {}x{} matrix",
                self.rows, self.cols, o.rows, o.cols
            )));
        }
        let mut data = Vec::with_capacity(self.rows * o.cols);
        let mut dims = Vec::with_capacity(self.rows * o.cols);
        for r in 0..self.rows {
            for c in 0..o.cols {
                // Every term of the sum must have the same dimension.
                let term = |k: usize| {
                    self.dim_at(r, k)
                        .checked_times(o.dim_at(k, c))
                        .ok_or(UnitError::DimensionOverflow)
                };
                let d = term(0)?;
                for k in 1..self.cols {
                    let other = term(k)?;
                    if other != d {
                        return Err(UnitError::Mismatch(d, other).into());
                    }
                }
                dims.push(d);
                data.push((0..self.cols).map(|k| self.at(r, k) * o.at(k, c)).sum());
            }
        }
        Matrix::build(self.rows, o.cols, data, Dims::of(dims))
    }

    pub fn transposed(&self) -> Matrix {
        let mut data = Vec::with_capacity(self.data.len());
        let mut dims = Vec::with_capacity(self.data.len());
        for c in 0..self.cols {
            for r in 0..self.rows {
                data.push(self.at(r, c));
                dims.push(self.dim_at(r, c));
            }
        }
        Matrix {
            rows: self.cols,
            cols: self.rows,
            data,
            dims: Dims::of(dims),
        }
    }

    /// Sum of products of two vectors of the same length.
    pub fn dot(&self, o: &Matrix) -> Result<Scalar, ValueError> {
        if !self.is_vector() || !o.is_vector() || self.rows != o.rows {
            return Err(shape_error("dot needs two vectors of the same length"));
        }
        let term = |i: usize| {
            self.dim_of(i)
                .checked_times(o.dim_of(i))
                .ok_or(UnitError::DimensionOverflow)
        };
        let dim = term(0)?;
        for i in 1..self.rows {
            let other = term(i)?;
            if other != dim {
                return Err(UnitError::Mismatch(dim, other).into());
            }
        }
        let z = self.data.iter().zip(&o.data).map(|(a, b)| a * b).sum();
        Ok(Scalar::new(z, dim))
    }

    /// Determinant by Gaussian elimination with partial pivoting.
    pub fn determinant(&self) -> Result<Scalar, ValueError> {
        if self.rows != self.cols {
            return Err(shape_error("the determinant needs a square matrix"));
        }
        let n = self.rows;
        let common = self.common_dim()?;
        let dim = i8::try_from(n)
            .ok()
            .and_then(|n| common.checked_powi(n))
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

    /// Euclidean norm of every element, in the dimension all elements share.
    pub fn norm(&self) -> Result<Scalar, ValueError> {
        let dim = self.common_dim()?;
        let parts = self.data.iter().flat_map(|z| [z.re, z.im]);
        Ok(Scalar::new(
            Complex64::new(numeric::length(parts), 0.0),
            dim,
        ))
    }
}
