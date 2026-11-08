//! Built-in functions. Arguments arrive already evaluated.

mod stats;

use crate::eval::EvalError;
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use crate::value::{Matrix, Scalar, Series, Value, ValueError};

type Result<T> = std::result::Result<T, EvalError>;

/// How many arguments a function takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arity {
    Exactly(usize),
    AtLeast(usize),
}

/// One built-in function: its name, how it is written, what it does, and its
/// implementation. The single table, so the evaluator, the reserved-name check
/// for custom units, the help text and (later) autocomplete cannot disagree.
pub struct FunctionDef {
    pub name: &'static str,
    pub signature: &'static str,
    pub summary: &'static str,
    pub arity: Arity,
    run: fn(&[Value]) -> Result<Value>,
}

macro_rules! function {
    ($name:literal, $sig:literal, $summary:literal, $arity:expr, $run:expr) => {
        FunctionDef {
            name: $name,
            signature: $sig,
            summary: $summary,
            arity: $arity,
            run: $run,
        }
    };
}

use Arity::{AtLeast, Exactly};

mod calculus;
mod discrete;
mod elementary;
mod linalg;
mod lists;
mod polynomial;

pub const FUNCTIONS: &[FunctionDef] = &[
    function!(
        "sqrt",
        "sqrt(x)",
        "Principal square root; negative reals give imaginary results",
        Exactly(1),
        |a| {
            scalar_arg("sqrt", a)?
                .sqrt()
                .map(Value::Scalar)
                .map_err(Into::into)
        }
    ),
    function!(
        "abs",
        "abs(z)",
        "Absolute value, or the modulus of a complex number",
        Exactly(1),
        |a| abs("abs", a)
    ),
    function!("re", "re(z)", "Real part", Exactly(1), |a| part(
        "re",
        a,
        |z| z.re
    )),
    function!("im", "im(z)", "Imaginary part", Exactly(1), |a| part(
        "im",
        a,
        |z| z.im
    )),
    function!("conj", "conj(z)", "Complex conjugate", Exactly(1), |a| {
        let s = scalar_arg("conj", a)?;
        Ok(Value::Scalar(Scalar::new(s.z.conj(), s.dim)))
    }),
    function!(
        "arg",
        "arg(z)",
        "Angle of a complex number, in radians",
        Exactly(1),
        |a| { Ok(Value::dimensionless(scalar_arg("arg", a)?.z.arg())) }
    ),
    function!(
        "sin",
        "sin(x)",
        "Sine of a dimensionless number in radians",
        Exactly(1),
        |a| transcendental("sin", a, numeric::sin)
    ),
    function!(
        "cos",
        "cos(x)",
        "Cosine of a dimensionless number in radians",
        Exactly(1),
        |a| transcendental("cos", a, numeric::cos)
    ),
    function!(
        "tan",
        "tan(x)",
        "Tangent of a dimensionless number in radians",
        Exactly(1),
        |a| transcendental("tan", a, numeric::tan)
    ),
    function!("ln", "ln(x)", "Natural logarithm", Exactly(1), |a| {
        transcendental("ln", a, numeric::ln)
    }),
    function!(
        "exp",
        "exp(x)",
        "e raised to a dimensionless power",
        Exactly(1),
        |a| transcendental("exp", a, numeric::exp)
    ),
    function!(
        "vec",
        "vec(a, b, ...)",
        "Column vector; all elements share one dimension",
        AtLeast(1),
        vector
    ),
    function!(
        "mat",
        "mat(rows, cols, a11, a12, ...)",
        "Matrix from row-major elements",
        AtLeast(3),
        matrix
    ),
    function!(
        "sum",
        "sum(x, ...)",
        "Total of every element",
        AtLeast(1),
        stats::sum
    ),
    function!(
        "mean",
        "mean(x, ...)",
        "Average of every element",
        AtLeast(1),
        stats::mean
    ),
    function!(
        "stdev",
        "stdev(x, ...)",
        "Population standard deviation",
        AtLeast(1),
        stats::stdev
    ),
    function!(
        "stdev_sample",
        "stdev_sample(x, ...)",
        "Sample standard deviation (divides by n - 1)",
        AtLeast(1),
        stats::stdev_sample
    ),
    function!(
        "min",
        "min(x, ...)",
        "Smallest element",
        AtLeast(1),
        stats::min
    ),
    function!(
        "max",
        "max(x, ...)",
        "Largest element",
        AtLeast(1),
        stats::max
    ),
    function!(
        "length",
        "length(x, ...)",
        "Number of elements",
        AtLeast(1),
        stats::length
    ),
    function!(
        "transpose",
        "transpose(A)",
        "Transposed matrix",
        Exactly(1),
        |a| { Ok(Value::Matrix(matrix_arg("transpose", a)?.transposed())) }
    ),
    function!(
        "det",
        "det(A)",
        "Determinant of a square matrix",
        Exactly(1),
        |a| { Ok(Value::Scalar(matrix_arg("det", a)?.determinant()?)) }
    ),
    function!(
        "norm",
        "norm(A)",
        "Euclidean norm of all elements",
        Exactly(1),
        |a| { Ok(Value::Scalar(matrix_arg("norm", a)?.norm()?)) }
    ),
    function!(
        "dot",
        "dot(u, v)",
        "Dot product of two vectors",
        Exactly(2),
        |a| {
            let [Value::Matrix(u), Value::Matrix(v)] = a else {
                return Err(ValueError::Shape("dot needs two vectors".into()).into());
            };
            Ok(Value::Scalar(u.dot(v)?))
        }
    ),
    function!("rows", "rows(A)", "Number of rows", Exactly(1), |a| {
        Ok(Value::dimensionless(matrix_arg("rows", a)?.rows() as f64))
    }),
    function!("cols", "cols(A)", "Number of columns", Exactly(1), |a| {
        Ok(Value::dimensionless(matrix_arg("cols", a)?.cols() as f64))
    }),
];

/// The group a function is listed under (the Functions window and its search).
pub fn category(name: &str) -> &'static str {
    match name {
        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2" | "sinh" | "cosh" | "tanh"
        | "asinh" | "acosh" | "atanh" => "Trigonometry",
        "ln" | "exp" | "sqrt" | "log" => "Powers and logarithms",
        "floor" | "ceil" | "trunc" | "round" | "sign" | "mod" | "gcd" | "lcm" => {
            "Rounding and integers"
        }
        "abs" | "re" | "im" | "conj" | "arg" => "Complex numbers",
        "vec" | "mat" | "transpose" | "det" | "norm" | "dot" | "rows" | "cols" | "identity"
        | "tr" | "rank" | "cross" | "augment" | "stack" => "Vectors and matrices",
        "sum" | "mean" | "stdev" | "stdev_sample" | "min" | "max" | "length" | "median"
        | "mode" | "var" | "var_sample" | "sort" | "reverse" | "cumsum" | "cvar" | "corr" => {
            "Statistics"
        }
        "factorial" | "combin" | "permut" | "isprime" => "Number theory",
        "sum_over" | "product_over" | "integrate" | "derivative" | "limit" => "Calculus",
        "root" | "polyroots" | "find" | "minerr" | "minimize" | "maximize" => "Solving",
        _ => "Other",
    }
}

/// Every built-in function: the core table, then the tables of each module.
pub fn all() -> impl Iterator<Item = &'static FunctionDef> + Clone {
    FUNCTIONS
        .iter()
        .chain(elementary::ELEMENTARY)
        .chain(linalg::LINALG)
        .chain(lists::LISTS)
        .chain(discrete::DISCRETE)
        .chain(calculus::CALCULUS)
        .chain(polynomial::POLYNOMIAL)
}

pub fn lookup(name: &str) -> Option<&'static FunctionDef> {
    all().find(|f| f.name == name)
}

pub fn call(name: &str, args: &[Value]) -> Result<Value> {
    let f = lookup(name).ok_or_else(|| EvalError::UnknownFunction(name.to_owned()))?;
    match f.arity {
        Exactly(n) if args.len() != n => return Err(EvalError::Arity(name.to_owned(), n)),
        AtLeast(n) if args.len() < n => {
            return Err(EvalError::Invalid(format!(
                "{name} needs at least {n} argument(s)"
            )))
        }
        _ => {}
    }
    if let [Value::Series(list)] = args {
        if MAPS_OVER_LISTS.contains(&name) {
            return over_list(f, list);
        }
    }
    // `abs` of a matrix is not element by element, so it is not in this list.
    if let [Value::Matrix(m)] = args {
        if name != "abs" && MAPS_OVER_LISTS.contains(&name) {
            return lists::over_matrix(f, m);
        }
    }
    (f.run)(args)
}

/// Functions of one number that apply to each element of a list.
const MAPS_OVER_LISTS: &[&str] = &[
    "sqrt", "abs", "re", "im", "conj", "arg", "sin", "cos", "tan", "ln", "exp", "asin", "acos",
    "atan", "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "floor", "ceil", "trunc", "sign",
];

fn over_list(f: &FunctionDef, list: &Series) -> Result<Value> {
    let mut out = Vec::with_capacity(list.len());
    for element in list.scalars() {
        match (f.run)(&[Value::Scalar(element)])? {
            Value::Scalar(s) => out.push(s),
            _ => unreachable!("these functions return a number"),
        }
    }
    Ok(Value::Series(Series::from_scalars(&out)?))
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
            Value::Matrix(_) | Value::Series(_) | Value::Function { .. } => {
                Err(ValueError::Shape("matrix elements must be scalars".into()).into())
            }
        })
        .collect()
}

/// `vec(a, b, c)`: a column vector.
fn vector(args: &[Value]) -> Result<Value> {
    // A list of values becomes a column vector.
    if let [Value::Series(list)] = args {
        let items: Vec<Scalar> = list.scalars().collect();
        return Ok(Value::Matrix(Matrix::column(&items)?));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_function_has_a_real_category() {
        for f in all() {
            assert_ne!(category(f.name), "Other", "{} has no category", f.name);
        }
    }

    #[test]
    fn names_are_unique_and_every_entry_is_documented() {
        let every: Vec<&FunctionDef> = all().collect();
        for (k, f) in every.iter().enumerate() {
            assert!(every[..k].iter().all(|g| g.name != f.name), "{}", f.name);
            assert!(f.signature.starts_with(f.name), "{}", f.name);
            assert!(!f.summary.is_empty(), "{}", f.name);
        }
    }

    #[test]
    fn the_arity_is_checked_before_the_function_runs() {
        let one = Value::dimensionless(1.0);
        assert_eq!(call("sin", &[]), Err(EvalError::Arity("sin".into(), 1)));
        assert_eq!(
            call("dot", std::slice::from_ref(&one)),
            Err(EvalError::Arity("dot".into(), 2))
        );
        assert!(matches!(call("vec", &[]), Err(EvalError::Invalid(_))));
        assert!(matches!(
            call("mat", &[one.clone(), one]),
            Err(EvalError::Invalid(_))
        ));
        assert_eq!(
            call("nope", &[]),
            Err(EvalError::UnknownFunction("nope".into()))
        );
    }
}
