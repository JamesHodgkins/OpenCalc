//! Functions of lists and vectors: `median`, `mode`, `var`, `var_sample`,
//! `sort`, `reverse`, `cumsum`, `cvar`, `corr` (roadmap Phase 1.2), and the
//! element-by-element application of a one-number function to a matrix.
//!
//! Like `stdev`, `var` and `cvar` divide by n (the population form, as Mathcad
//! does); `var_sample` divides by n - 1. All of them need real numbers and, where
//! several values are combined, one shared dimension.

use super::stats::{gather, gather_real};
use super::Arity::{AtLeast, Exactly};
use super::{FunctionDef, Result};
use crate::eval::EvalError;
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use crate::value::{Matrix, Scalar, Series, Value, ValueError};

pub(super) const LISTS: &[FunctionDef] = &[
    function!(
        "median",
        "median(x, ...)",
        "Middle value of every element (the mean of the two middle ones when even)",
        AtLeast(1),
        median
    ),
    function!(
        "mode",
        "mode(x, ...)",
        "Most frequent value; the smallest of any that tie",
        AtLeast(1),
        mode
    ),
    function!(
        "var",
        "var(x, ...)",
        "Population variance (divides by n)",
        AtLeast(1),
        |a| variance("var", a, false)
    ),
    function!(
        "var_sample",
        "var_sample(x, ...)",
        "Sample variance (divides by n - 1)",
        AtLeast(2),
        |a| variance("var_sample", a, true)
    ),
    function!(
        "sort",
        "sort(v)",
        "A vector or list in ascending order",
        Exactly(1),
        |a| reshape("sort", a, |items| {
            items.sort_by(|x, y| x.re.total_cmp(&y.re));
        })
    ),
    function!(
        "reverse",
        "reverse(v)",
        "A vector or list in the opposite order",
        Exactly(1),
        |a| reshape("reverse", a, |items| items.reverse())
    ),
    function!(
        "cumsum",
        "cumsum(v)",
        "Running total of a vector or list",
        Exactly(1),
        |a| reshape("cumsum", a, |items| {
            let mut total = Complex64::new(0.0, 0.0);
            for z in items.iter_mut() {
                total += *z;
                *z = total;
            }
        })
    ),
    function!(
        "cvar",
        "cvar(u, v)",
        "Population covariance of two vectors or lists of the same length",
        Exactly(2),
        |a| covariance("cvar", a, false)
    ),
    function!(
        "corr",
        "corr(u, v)",
        "Pearson correlation of two vectors or lists of the same length",
        Exactly(2),
        |a| covariance("corr", a, true)
    ),
];

fn median(args: &[Value]) -> Result<Value> {
    let (mut items, dim) = gather_real("median", args)?;
    items.sort_by(f64::total_cmp);
    let mid = items.len() / 2;
    let m = if items.len() % 2 == 1 {
        items[mid]
    } else {
        (items[mid - 1] + items[mid]) / 2.0
    };
    Ok(Value::Scalar(Scalar::new(numeric::real(m), dim)))
}

fn mode(args: &[Value]) -> Result<Value> {
    let (mut items, dim) = gather_real("mode", args)?;
    items.sort_by(f64::total_cmp);
    let (mut best, mut best_count) = (items[0], 0usize);
    let mut run_start = 0;
    for i in 1..=items.len() {
        if i == items.len() || items[i] != items[run_start] {
            if i - run_start > best_count {
                (best, best_count) = (items[run_start], i - run_start);
            }
            run_start = i;
        }
    }
    Ok(Value::Scalar(Scalar::new(numeric::real(best), dim)))
}

fn variance(name: &str, args: &[Value], sample: bool) -> Result<Value> {
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
    let dim = dim.checked_powi(2).ok_or(UnitError::DimensionOverflow)?;
    Ok(Value::Scalar(Scalar::new(
        numeric::real(squares / divisor),
        dim,
    )))
}

/// Apply `f` to the elements of a column vector or list and rebuild the same
/// kind of value. `sort` also needs every element real (checked before `f` runs).
fn reshape(name: &str, args: &[Value], f: impl FnOnce(&mut Vec<Complex64>)) -> Result<Value> {
    let [arg] = args else {
        return Err(EvalError::Arity(name.to_owned(), 1));
    };
    let (mut items, dim) = gather(args)?;
    let needs_real = name == "sort";
    if needs_real && items.iter().any(|z| !numeric::is_real(*z)) {
        return Err(ValueError::Unsupported(format!("'{name}' of complex numbers")).into());
    }
    f(&mut items);
    match arg {
        Value::Series(_) => Ok(Value::Series(Series::new(items, dim)?)),
        Value::Matrix(m) if m.is_vector() => {
            Ok(Value::Matrix(Matrix::new(m.rows(), 1, items, dim)?))
        }
        _ => Err(ValueError::Shape(format!("'{name}' needs a column vector or a list")).into()),
    }
}

/// Covariance, or with `correlation` the Pearson coefficient.
fn covariance(name: &str, args: &[Value], correlation: bool) -> Result<Value> {
    let [u, v] = args else {
        return Err(EvalError::Arity(name.to_owned(), 2));
    };
    let ((x, dx), (y, dy)) = (
        gather_real(name, std::slice::from_ref(u))?,
        gather_real(name, std::slice::from_ref(v))?,
    );
    if x.len() != y.len() {
        return Err(ValueError::Shape(format!(
            "'{name}' needs two lists of the same length ({} and {})",
            x.len(),
            y.len()
        ))
        .into());
    }
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let sxy: f64 = x.iter().zip(&y).map(|(a, b)| (a - mx) * (b - my)).sum();
    if !correlation {
        let dim = dx.checked_times(dy).ok_or(UnitError::DimensionOverflow)?;
        return Ok(Value::Scalar(Scalar::new(numeric::real(sxy / n), dim)));
    }
    let sxx: f64 = x.iter().map(|a| (a - mx) * (a - mx)).sum();
    let syy: f64 = y.iter().map(|b| (b - my) * (b - my)).sum();
    if sxx == 0.0 || syy == 0.0 {
        // One side never varies, so there is no correlation to speak of.
        return Err(EvalError::NotFinite);
    }
    let r = (sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0);
    Ok(Value::Scalar(Scalar::new(
        numeric::real(r),
        Dimension::NONE,
    )))
}

/// A one-number function applied to every element of a matrix.
pub(super) fn over_matrix(f: &FunctionDef, m: &Matrix) -> Result<Value> {
    let mut out = Vec::with_capacity(m.elements().len());
    for z in m.elements() {
        match (f.run)(&[Value::Scalar(Scalar::new(*z, m.dim))])? {
            Value::Scalar(s) => out.push(s),
            _ => return Err(EvalError::Internal(format!("'{}' left a number", f.name))),
        }
    }
    Ok(Value::Matrix(Matrix::from_scalars(
        m.rows(),
        m.cols(),
        &out,
    )?))
}

#[cfg(test)]
mod tests {
    use super::super::call;
    use super::*;

    fn num(v: f64) -> Value {
        Value::dimensionless(v)
    }

    fn nums(items: &[f64]) -> Vec<Value> {
        items.iter().map(|x| num(*x)).collect()
    }

    fn real(name: &str, items: &[f64]) -> f64 {
        call(name, &nums(items)).unwrap().as_real().unwrap().si
    }

    fn column(items: &[f64]) -> Value {
        let s: Vec<Scalar> = items
            .iter()
            .map(|x| Scalar::new(numeric::real(*x), Dimension::NONE))
            .collect();
        Value::Matrix(Matrix::column(&s).unwrap())
    }

    fn values(v: &Value) -> Vec<f64> {
        match v {
            Value::Matrix(m) => m.elements().iter().map(|z| z.re).collect(),
            Value::Series(s) => s.elements().iter().map(|z| z.re).collect(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn median_of_odd_and_even_counts() {
        assert_eq!(real("median", &[5.0, 1.0, 3.0]), 3.0);
        assert_eq!(real("median", &[4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(real("median", &[7.0]), 7.0);
    }

    #[test]
    fn mode_is_the_most_frequent_and_the_smallest_on_a_tie() {
        assert_eq!(real("mode", &[1.0, 2.0, 2.0, 3.0]), 2.0);
        assert_eq!(real("mode", &[3.0, 1.0, 3.0, 1.0, 2.0]), 1.0);
        assert_eq!(real("mode", &[4.0]), 4.0);
        assert_eq!(real("mode", &[-1.0, 5.0, 5.0, -1.0, 5.0]), 5.0);
    }

    #[test]
    fn variance_matches_stdev_squared_and_has_the_squared_dimension() {
        let data = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert_eq!(real("var", &data), 4.0);
        assert_eq!(real("var", &data), real("stdev", &data).powi(2));
        assert!((real("var_sample", &data) - 32.0 / 7.0).abs() < 1e-12);
        assert!(call("var_sample", &nums(&[1.0])).is_err());
        let lengths: Vec<Value> = [1.0, 3.0]
            .iter()
            .map(|x| crate::units::Quantity::from_unit(*x, "m").unwrap().into())
            .collect();
        let Value::Scalar(v) = call("var", &lengths).unwrap() else {
            panic!()
        };
        assert_eq!((v.z.re, v.dim), (1.0, Dimension::AREA));
    }

    #[test]
    fn sort_reverse_and_cumsum_keep_the_kind_of_value() {
        let v = column(&[3.0, 1.0, 2.0]);
        assert_eq!(
            values(&call("sort", std::slice::from_ref(&v)).unwrap()),
            [1.0, 2.0, 3.0]
        );
        assert_eq!(
            values(&call("reverse", std::slice::from_ref(&v)).unwrap()),
            [2.0, 1.0, 3.0]
        );
        assert_eq!(
            values(&call("cumsum", std::slice::from_ref(&v)).unwrap()),
            [3.0, 4.0, 6.0]
        );
        assert!(matches!(call("sort", &[v]).unwrap(), Value::Matrix(_)));
        let list = Value::Series(
            Series::range(
                Scalar::new(numeric::real(3.0), Dimension::NONE),
                Some(Scalar::new(numeric::real(2.0), Dimension::NONE)),
                Scalar::new(numeric::real(1.0), Dimension::NONE),
            )
            .unwrap(),
        );
        assert_eq!(values(&call("sort", &[list]).unwrap()), [1.0, 2.0, 3.0]);
        assert!(
            call("sort", &[num(1.0)]).is_err(),
            "a lone number is not a vector"
        );
        let wide = Value::Matrix(
            Matrix::from_scalars(1, 2, &[Scalar::new(numeric::real(1.0), Dimension::NONE); 2])
                .unwrap(),
        );
        assert!(
            call("sort", &[wide]).is_err(),
            "a row is not a column vector"
        );
        assert!(call("sort", &[Value::complex(1.0, 1.0)]).is_err());
    }

    #[test]
    fn covariance_and_correlation() {
        let (u, v) = (column(&[1.0, 2.0, 3.0, 4.0]), column(&[2.0, 4.0, 6.0, 8.0]));
        // cov = mean((x - 2.5)(y - 5)) = (1.5*3 + 0.5*1 + 0.5*1 + 1.5*3) / 4 = 2.5
        let cov = call("cvar", &[u.clone(), v.clone()])
            .unwrap()
            .as_real()
            .unwrap()
            .si;
        assert!((cov - 2.5).abs() < 1e-12);
        let r = call("corr", &[u.clone(), v]).unwrap().as_real().unwrap().si;
        assert!((r - 1.0).abs() < 1e-12);
        let down = column(&[8.0, 6.0, 4.0, 2.0]);
        let r = call("corr", &[u.clone(), down])
            .unwrap()
            .as_real()
            .unwrap()
            .si;
        assert!((r + 1.0).abs() < 1e-12);
        assert!(call("corr", &[u.clone(), column(&[1.0, 1.0, 1.0, 1.0])]).is_err());
        assert!(
            call("cvar", &[u, column(&[1.0, 2.0])]).is_err(),
            "different lengths"
        );
    }

    #[test]
    fn functions_apply_to_each_matrix_element() {
        let m = Value::Matrix(
            Matrix::from_scalars(
                2,
                2,
                &[0.0, 1.0, 4.0, 9.0].map(|x| Scalar::new(numeric::real(x), Dimension::NONE)),
            )
            .unwrap(),
        );
        let root = call("sqrt", std::slice::from_ref(&m)).unwrap();
        assert_eq!(values(&root), [0.0, 1.0, 2.0, 3.0]);
        let Value::Matrix(r) = root else { panic!() };
        assert_eq!((r.rows(), r.cols()), (2, 2));
        let area =
            Value::Matrix(Matrix::new(1, 1, vec![numeric::real(9.0)], Dimension::AREA).unwrap());
        let Value::Matrix(side) = call("sqrt", &[area]).unwrap() else {
            panic!()
        };
        assert_eq!(side.dim, Dimension::LENGTH);
        assert!(call("sin", &[m]).is_ok());
    }
}
