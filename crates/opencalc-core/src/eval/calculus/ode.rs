//! `odesolve(f, t, y, t0, y0, t1)` and `odesolve(f, t, y, t0, y0, t1, n)`.
//!
//! Solves `dy/dt = f(t, y)` from `t0` to `t1`. `t` and `y` are the names the
//! expression `f` uses for the time and the state; they exist only inside the call.
//! The state is a number or a column vector, and the elements of a vector may have
//! different units (a position and a speed). `f` must give a value of the same shape
//! whose elements have the units of `y` per unit of `t`.
//!
//! Without `n` the result is the state at `t1`. With `n` it is a table with `n + 1`
//! rows, one for each of `n + 1` equally spaced times from `t0` to `t1`, and the
//! columns `t`, `y1`, `y2`, ...; take columns out with `col`.

use super::super::node::{eval, Evaluated, Located};
use super::super::{Cx, EvalError};
use super::{invalid, real_arg};
use crate::expr::Expr;
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use crate::value::{Matrix, Scalar, Value, MAX_ELEMENTS};
use std::cell::RefCell;

mod rk45;

/// The shape of the state: one number, or a column vector.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Number,
    Vector,
}

fn value_of(shape: Shape, y: &[f64], dims: &[Dimension]) -> Result<Value, Located> {
    let items: Vec<Scalar> = y
        .iter()
        .zip(dims)
        .map(|(v, d)| Scalar::new(numeric::real(*v), *d))
        .collect();
    match shape {
        Shape::Number => Ok(Value::Scalar(items[0])),
        Shape::Vector => Ok(Value::Matrix(
            Matrix::column(&items).map_err(Located::here)?,
        )),
    }
}

pub(super) fn odesolve(args: &[Expr], cx: &Cx) -> Evaluated {
    if !(6..=7).contains(&args.len()) {
        return Err(invalid(
            "odesolve takes 6 or 7 arguments: odesolve(f, t, y, t0, y0, t1) or ... , n)",
        ));
    }
    let (Expr::Name(time), Expr::Name(state)) = (&args[1], &args[2]) else {
        return Err(invalid(
            "the second and third arguments of odesolve must be the names used for the time and the state",
        ));
    };
    if time == state {
        return Err(
            invalid("odesolve needs two different names for the time and the state").under(2),
        );
    }
    let ((t0, dt), (t1, dt1)) = (real_arg(args, 3, cx)?, real_arg(args, 5, cx)?);
    if dt != dt1 {
        return Err(Located::here(UnitError::Mismatch(dt, dt1)).under(5));
    }
    let segments = segments(args, cx)?;
    let (shape, y0, dims) = initial(&args[4], cx)?;
    let columns = 1 + y0.len();
    if (segments + 1)
        .checked_mul(columns)
        .is_none_or(|n| n > MAX_ELEMENTS)
    {
        return Err(invalid(format!(
            "a table of {} rows and {columns} columns is larger than the {MAX_ELEMENTS} elements a matrix may have",
            segments + 1
        ))
        .under(6));
    }
    // Expected units of f: the state's per unit of time.
    let rates: Vec<Dimension> = dims
        .iter()
        .map(|d| d.checked_over(dt).ok_or(UnitError::DimensionOverflow))
        .collect::<Result<_, _>>()
        .map_err(Located::here)?;

    let mut locals = cx.locals.borrow().clone();
    let base = locals.len();
    locals.push((time.clone(), Value::dimensionless(0.0)));
    locals.push((state.clone(), Value::dimensionless(0.0)));
    let mut inner = Cx {
        env: cx.env,
        fns: cx.fns,
        units: cx.units,
        locals: RefCell::new(locals),
        depth: cx.depth,
        meter: cx.meter,
    };
    let body = &args[0];
    let mut rhs = |t: f64, y: &[f64]| -> Result<Vec<f64>, Located> {
        inner.meter.charge().map_err(Located::here)?;
        inner.locals.get_mut()[base].1 = Value::Scalar(Scalar::new(numeric::real(t), dt));
        inner.locals.get_mut()[base + 1].1 = value_of(shape, y, &dims)?;
        let v = eval(body, &inner).map_err(|l| l.under(0))?;
        let items: Vec<Scalar> = match (shape, v) {
            (Shape::Number, Value::Scalar(s)) => vec![s],
            (Shape::Vector, Value::Matrix(m)) if m.is_vector() && m.elements().len() == y.len() => {
                m.scalars().collect()
            }
            _ => {
                return Err(
                    invalid("f must have the same shape as the state y (a number, or a vector of the same length)")
                        .under(0),
                )
            }
        };
        let mut out = Vec::with_capacity(items.len());
        for (s, want) in items.iter().zip(&rates) {
            if !numeric::is_real(s.z) {
                return Err(invalid("odesolve works with real numbers only").under(0));
            }
            if s.dim != *want {
                return Err(Located::here(UnitError::Mismatch(s.dim, *want)).under(0));
            }
            out.push(s.z.re);
        }
        Ok(out)
    };
    let states = rk45::solve(&mut rhs, t0, &y0, t1, segments)?;

    let Some(last) = states.last() else {
        return Err(Located::here(EvalError::Internal(
            "odesolve produced nothing".into(),
        )));
    };
    if args.len() == 6 {
        return value_of(shape, last, &dims);
    }
    // The table: time first, then the state, a row per time.
    let mut data: Vec<Complex64> = Vec::with_capacity((segments + 1) * columns);
    let mut table_dims: Vec<Dimension> = Vec::with_capacity((segments + 1) * columns);
    for (k, state) in states.iter().enumerate() {
        let t = if k == segments {
            t1
        } else {
            t0 + (t1 - t0) * k as f64 / segments as f64
        };
        data.push(numeric::real(t));
        table_dims.push(dt);
        data.extend(state.iter().map(|v| numeric::real(*v)));
        table_dims.extend(dims.iter().copied());
    }
    Ok(Value::Matrix(
        Matrix::with_dims(segments + 1, columns, data, table_dims).map_err(Located::here)?,
    ))
}

/// The number of output intervals: 1 (just the end) unless the seventh argument asks.
fn segments(args: &[Expr], cx: &Cx) -> Result<usize, Located> {
    if args.len() < 7 {
        return Ok(1);
    }
    let (n, dim) = real_arg(args, 6, cx)?;
    if dim != Dimension::NONE || n.fract() != 0.0 || !(1.0..=4096.0).contains(&n) {
        return Err(invalid(
            "the number of output intervals must be a whole number from 1 to 4096",
        )
        .under(6));
    }
    Ok(n as usize)
}

/// The initial state: its shape, values and the dimension of each element.
fn initial(arg: &Expr, cx: &Cx) -> Result<(Shape, Vec<f64>, Vec<Dimension>), Located> {
    match eval(arg, cx).map_err(|l| l.under(4))? {
        Value::Scalar(s) if numeric::is_real(s.z) => Ok((Shape::Number, vec![s.z.re], vec![s.dim])),
        Value::Matrix(m) if m.is_vector() && m.elements().iter().all(|z| numeric::is_real(*z)) => {
            let dims = (0..m.elements().len()).map(|i| m.dim_of(i)).collect();
            Ok((
                Shape::Vector,
                m.elements().iter().map(|z| z.re).collect(),
                dims,
            ))
        }
        _ => Err(invalid(
            "the initial state must be a real number or a column vector of real numbers",
        )
        .under(4)),
    }
}
