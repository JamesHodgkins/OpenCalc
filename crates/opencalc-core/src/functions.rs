//! Built-in functions. Arguments arrive already evaluated.

use crate::eval::EvalError;
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use crate::value::{Matrix, Scalar, Value, ValueError};

type Result<T> = std::result::Result<T, EvalError>;

pub fn call(name: &str, args: &[Value]) -> Result<Value> {
    match name {
        "sqrt" => scalar_arg(name, args)?
            .sqrt()
            .map(Value::Scalar)
            .map_err(Into::into),
        "abs" => abs(name, args),
        "re" => part(name, args, |z| z.re),
        "im" => part(name, args, |z| z.im),
        "conj" => {
            let s = scalar_arg(name, args)?;
            Ok(Value::Scalar(Scalar::new(s.z.conj(), s.dim)))
        }
        "arg" => {
            let s = scalar_arg(name, args)?;
            Ok(Value::dimensionless(s.z.arg()))
        }
        "sin" => transcendental(name, args, numeric::sin),
        "cos" => transcendental(name, args, numeric::cos),
        "tan" => transcendental(name, args, numeric::tan),
        "ln" => transcendental(name, args, numeric::ln),
        "exp" => transcendental(name, args, numeric::exp),
        "vec" => vector(args),
        "mat" => matrix(args),
        "transpose" => Ok(Value::Matrix(matrix_arg(name, args)?.transposed())),
        "det" => Ok(Value::Scalar(matrix_arg(name, args)?.determinant()?)),
        "norm" => Ok(Value::Scalar(matrix_arg(name, args)?.norm())),
        "dot" => {
            let [Value::Matrix(a), Value::Matrix(b)] = args else {
                return Err(EvalError::Arity(name.to_owned(), 2));
            };
            Ok(Value::Scalar(a.dot(b)?))
        }
        "rows" => Ok(Value::dimensionless(matrix_arg(name, args)?.rows() as f64)),
        "cols" => Ok(Value::dimensionless(matrix_arg(name, args)?.cols() as f64)),
        _ => Err(EvalError::UnknownFunction(name.to_owned())),
    }
}

fn scalar_arg(name: &str, args: &[Value]) -> Result<Scalar> {
    match args {
        [Value::Scalar(s)] => Ok(*s),
        [Value::Matrix(_)] => Err(ValueError::Unsupported(format!("'{name}'")).into()),
        _ => Err(EvalError::Arity(name.to_owned(), 1)),
    }
}

fn matrix_arg<'a>(name: &str, args: &'a [Value]) -> Result<&'a Matrix> {
    match args {
        [Value::Matrix(m)] => Ok(m),
        [Value::Scalar(_)] => Err(ValueError::Shape(format!("'{name}' needs a matrix")).into()),
        _ => Err(EvalError::Arity(name.to_owned(), 1)),
    }
}

/// Modulus: keeps the dimension. Of a matrix it is not defined (use `norm`).
fn abs(name: &str, args: &[Value]) -> Result<Value> {
    let s = scalar_arg(name, args)?;
    Ok(Value::Scalar(Scalar::new(
        numeric::real(numeric::length([s.z.re, s.z.im].into_iter())),
        s.dim,
    )))
}

fn part(name: &str, args: &[Value], f: fn(Complex64) -> f64) -> Result<Value> {
    let s = scalar_arg(name, args)?;
    Ok(Value::Scalar(Scalar::new(numeric::real(f(s.z)), s.dim)))
}

fn transcendental(name: &str, args: &[Value], f: fn(Complex64) -> Complex64) -> Result<Value> {
    let s = scalar_arg(name, args)?;
    if s.dim != Dimension::NONE {
        return Err(UnitError::NotDimensionless(name.to_owned()).into());
    }
    Ok(Value::Scalar(Scalar::new(f(s.z), Dimension::NONE)))
}

fn scalars(args: &[Value]) -> Result<Vec<Scalar>> {
    args.iter()
        .map(|a| match a {
            Value::Scalar(s) => Ok(*s),
            Value::Matrix(_) => {
                Err(ValueError::Shape("matrix elements must be scalars".into()).into())
            }
        })
        .collect()
}

/// `vec(a, b, c)`: a column vector.
fn vector(args: &[Value]) -> Result<Value> {
    Ok(Value::Matrix(Matrix::column(&scalars(args)?)?))
}

/// `mat(rows, cols, a11, a12, ...)`: row-major elements.
fn matrix(args: &[Value]) -> Result<Value> {
    let [rows, cols, items @ ..] = args else {
        return Err(EvalError::Invalid(
            "mat(rows, cols, elements...) needs a shape".into(),
        ));
    };
    let (rows, cols) = (count(rows)?, count(cols)?);
    Ok(Value::Matrix(Matrix::from_scalars(
        rows,
        cols,
        &scalars(items)?,
    )?))
}

fn count(v: &Value) -> Result<usize> {
    match v.as_real() {
        Some(q)
            if q.dim.is_dimensionless()
                && q.si.fract() == 0.0
                && (1.0..=4096.0).contains(&q.si) =>
        {
            Ok(q.si as usize)
        }
        _ => Err(EvalError::Invalid(
            "a matrix size must be a whole number of at least 1".into(),
        )),
    }
}
