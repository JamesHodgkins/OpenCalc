//! Matrix constructors and operations: `identity`, `tr`, `rank`, `cross`,
//! `augment`, `stack` (roadmap Phase 1.2). The inverse and integer powers are
//! the `^` operator (`M ^ -1`, `M ^ 3`), as in Mathcad.

use super::Arity::Exactly;
use super::{count, FunctionDef, Result};
use crate::eval::EvalError;
use crate::value::{Matrix, Value, ValueError};

pub(super) const LINALG: &[FunctionDef] = &[
    function!(
        "identity",
        "identity(n)",
        "The n by n identity matrix",
        Exactly(1),
        |a| {
            let [n] = a else {
                return Err(EvalError::Arity("identity".into(), 1));
            };
            Ok(Value::Matrix(Matrix::identity(count(n)?)?))
        }
    ),
    function!(
        "tr",
        "tr(A)",
        "Trace: the sum of the diagonal of a square matrix",
        Exactly(1),
        |a| Ok(Value::Scalar(one("tr", a)?.trace()?))
    ),
    function!(
        "rank",
        "rank(A)",
        "Number of independent rows (relative tolerance: largest element x size x machine epsilon)",
        Exactly(1),
        |a| Ok(Value::dimensionless(one("rank", a)?.rank() as f64))
    ),
    function!(
        "cross",
        "cross(u, v)",
        "Cross product of two 3-element vectors",
        Exactly(2),
        |a| {
            let (u, v) = two("cross", a)?;
            Ok(Value::Matrix(u.cross(v)?))
        }
    ),
    function!(
        "augment",
        "augment(A, B)",
        "A with B placed to its right (same number of rows)",
        Exactly(2),
        |a| {
            let (u, v) = two("augment", a)?;
            Ok(Value::Matrix(u.augment(v)?))
        }
    ),
    function!(
        "stack",
        "stack(A, B)",
        "A with B placed below it (same number of columns)",
        Exactly(2),
        |a| {
            let (u, v) = two("stack", a)?;
            Ok(Value::Matrix(u.stack(v)?))
        }
    ),
];

fn one<'a>(name: &str, args: &'a [Value]) -> Result<&'a Matrix> {
    match args {
        [Value::Matrix(m)] => Ok(m),
        [_] => Err(ValueError::Shape(format!("'{name}' needs a matrix")).into()),
        _ => Err(EvalError::Arity(name.to_owned(), 1)),
    }
}

fn two<'a>(name: &str, args: &'a [Value]) -> Result<(&'a Matrix, &'a Matrix)> {
    match args {
        [Value::Matrix(a), Value::Matrix(b)] => Ok((a, b)),
        [_, _] => Err(ValueError::Shape(format!("'{name}' needs two matrices")).into()),
        _ => Err(EvalError::Arity(name.to_owned(), 2)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::call;
    use super::*;
    use crate::numeric;
    use crate::units::Dimension;
    use crate::value::Scalar;

    fn mat(rows: usize, cols: usize, items: &[f64]) -> Value {
        let s: Vec<Scalar> = items
            .iter()
            .map(|x| Scalar::new(numeric::real(*x), Dimension::NONE))
            .collect();
        Value::Matrix(Matrix::from_scalars(rows, cols, &s).unwrap())
    }

    fn num(v: f64) -> Value {
        Value::dimensionless(v)
    }

    #[test]
    fn identity_trace_and_rank_through_the_function_table() {
        let i3 = call("identity", &[num(3.0)]).unwrap();
        assert_eq!(call("tr", std::slice::from_ref(&i3)).unwrap(), num(3.0));
        assert_eq!(call("rank", &[i3]).unwrap(), num(3.0));
        assert!(call("identity", &[num(0.0)]).is_err());
        assert!(
            call("identity", &[num(4096.0)]).is_err(),
            "over the element limit"
        );
    }

    #[test]
    fn cross_augment_and_stack_through_the_function_table() {
        let x = mat(3, 1, &[1.0, 0.0, 0.0]);
        let y = mat(3, 1, &[0.0, 1.0, 0.0]);
        assert_eq!(
            call("cross", &[x.clone(), y.clone()]).unwrap(),
            mat(3, 1, &[0.0, 0.0, 1.0])
        );
        assert_eq!(
            call("augment", &[x.clone(), y.clone()]).unwrap(),
            mat(3, 2, &[1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
        );
        assert_eq!(
            call("stack", &[x, y]).unwrap(),
            mat(6, 1, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
        );
        assert!(call("rank", &[num(1.0)]).is_err());
        assert!(call("cross", &[num(1.0), num(2.0)]).is_err());
    }

    #[test]
    fn the_power_operator_gives_the_inverse_and_integer_powers() {
        let a = mat(2, 2, &[4.0, 7.0, 2.0, 6.0]);
        let inv = a.pow(&num(-1.0)).unwrap();
        let Value::Matrix(inv) = inv else { panic!() };
        assert!((inv.at(0, 0).re - 0.6).abs() < 1e-12);
        assert_eq!(a.pow(&num(1.0)).unwrap(), a);
        let sq = a.pow(&num(2.0)).unwrap();
        assert_eq!(sq, a.mul(&a).unwrap());
        assert!(
            a.pow(&num(0.5)).is_err(),
            "a fractional matrix power is refused"
        );
        assert!(a.pow(&Value::complex(1.0, 1.0)).is_err());
        assert!(a.pow(&num(1e30)).is_err());
        assert!(mat(2, 1, &[1.0, 2.0]).pow(&num(2.0)).is_err(), "not square");
    }
}
