//! Elementary functions of one or two numbers: inverse and hyperbolic
//! trigonometry, logarithms to any base, rounding, remainders and whole-number
//! functions (roadmap Phase 1.2).
//!
//! Rules shared by all of them:
//! * trigonometric, hyperbolic and logarithm arguments must be dimensionless;
//! * rounding (`floor`, `ceil`, `round`, `trunc`) also needs a dimensionless
//!   argument, because rounding a length would depend on the unit it is shown in;
//! * `mod` and `sign` accept any dimension (they do not depend on the unit);
//! * a real argument inside the real domain gives a real result, and one outside
//!   it (`asin(2)`, `acosh(0.5)`) moves into the complex plane (principal value).

use super::Arity::{AtLeast, Exactly};
use super::{scalar_arg, transcendental, FunctionDef, Result};
use crate::eval::EvalError;
use crate::numeric::{self, Complex64};
use crate::units::{Dimension, UnitError};
use crate::value::{Scalar, Value, ValueError};

/// The real function inside its domain, the complex one outside it.
fn real_or(
    z: Complex64,
    inside: fn(f64) -> bool,
    real: fn(f64) -> f64,
    complex: fn(Complex64) -> Complex64,
) -> Complex64 {
    if numeric::is_real(z) && inside(z.re) {
        numeric::real(real(z.re))
    } else {
        complex(z)
    }
}

macro_rules! inverse {
    ($name:ident, $inside:expr, $real:expr, $complex:expr) => {
        fn $name(z: Complex64) -> Complex64 {
            real_or(z, $inside, $real, $complex)
        }
    };
}

inverse!(asin, |x| x.abs() <= 1.0, f64::asin, |z| z.asin());
inverse!(acos, |x| x.abs() <= 1.0, f64::acos, |z| z.acos());
inverse!(atan, |_| true, f64::atan, |z| z.atan());
inverse!(sinh, |_| true, f64::sinh, |z| z.sinh());
inverse!(cosh, |_| true, f64::cosh, |z| z.cosh());
inverse!(tanh, |_| true, f64::tanh, |z| z.tanh());
inverse!(asinh, |_| true, f64::asinh, |z| z.asinh());
inverse!(acosh, |x| x >= 1.0, f64::acosh, |z| z.acosh());
inverse!(atanh, |x| x.abs() < 1.0, f64::atanh, |z| z.atanh());

pub(super) const ELEMENTARY: &[FunctionDef] = &[
    function!(
        "asin",
        "asin(x)",
        "Inverse sine, in radians",
        Exactly(1),
        |a| transcendental("asin", a, asin)
    ),
    function!(
        "acos",
        "acos(x)",
        "Inverse cosine, in radians",
        Exactly(1),
        |a| transcendental("acos", a, acos)
    ),
    function!(
        "atan",
        "atan(x)",
        "Inverse tangent, in radians",
        Exactly(1),
        |a| transcendental("atan", a, atan)
    ),
    function!(
        "atan2",
        "atan2(x, y)",
        "Angle in radians from the positive x axis to the point (x, y); x and y share a dimension",
        Exactly(2),
        atan2
    ),
    function!("sinh", "sinh(x)", "Hyperbolic sine", Exactly(1), |a| {
        transcendental("sinh", a, sinh)
    }),
    function!("cosh", "cosh(x)", "Hyperbolic cosine", Exactly(1), |a| {
        transcendental("cosh", a, cosh)
    }),
    function!("tanh", "tanh(x)", "Hyperbolic tangent", Exactly(1), |a| {
        transcendental("tanh", a, tanh)
    }),
    function!(
        "asinh",
        "asinh(x)",
        "Inverse hyperbolic sine",
        Exactly(1),
        |a| transcendental("asinh", a, asinh)
    ),
    function!(
        "acosh",
        "acosh(x)",
        "Inverse hyperbolic cosine",
        Exactly(1),
        |a| transcendental("acosh", a, acosh)
    ),
    function!(
        "atanh",
        "atanh(x)",
        "Inverse hyperbolic tangent",
        Exactly(1),
        |a| transcendental("atanh", a, atanh)
    ),
    function!(
        "log",
        "log(x) or log(x, b)",
        "Logarithm to base 10, or to base b",
        AtLeast(1),
        log
    ),
    function!(
        "floor",
        "floor(x)",
        "Largest whole number not above x",
        Exactly(1),
        |a| round_with("floor", a, f64::floor)
    ),
    function!(
        "ceil",
        "ceil(x)",
        "Smallest whole number not below x",
        Exactly(1),
        |a| round_with("ceil", a, f64::ceil)
    ),
    function!(
        "trunc",
        "trunc(x)",
        "x with its fractional part removed",
        Exactly(1),
        |a| round_with("trunc", a, f64::trunc)
    ),
    function!(
        "round",
        "round(x) or round(x, n)",
        "x to the nearest whole number, or to n decimal places (halves round away from zero)",
        AtLeast(1),
        round
    ),
    function!(
        "sign",
        "sign(x)",
        "-1, 0 or 1 by the sign of a real x, in any dimension",
        Exactly(1),
        sign
    ),
    function!(
        "mod",
        "mod(x, y)",
        "Remainder of x divided by y, with the sign of y; x and y share a dimension",
        Exactly(2),
        modulo
    ),
    function!(
        "gcd",
        "gcd(a, b, ...)",
        "Greatest common divisor of whole numbers",
        AtLeast(2),
        |a| whole_numbers("gcd", a, gcd)
    ),
    function!(
        "lcm",
        "lcm(a, b, ...)",
        "Least common multiple of whole numbers",
        AtLeast(2),
        |a| whole_numbers("lcm", a, lcm)
    ),
];

/// The real value and dimension of one argument.
fn real_of(name: &str, v: &Value) -> Result<(f64, Dimension)> {
    match v {
        Value::Scalar(s) if numeric::is_real(s.z) => Ok((s.z.re, s.dim)),
        Value::Scalar(_) => {
            Err(ValueError::Unsupported(format!("'{name}' of a complex number")).into())
        }
        _ => Err(ValueError::Shape(format!("'{name}' needs a single number")).into()),
    }
}

/// Two real arguments that must share a dimension.
fn same_dimension(name: &str, a: &[Value]) -> Result<(f64, f64, Dimension)> {
    let [x, y] = a else {
        return Err(EvalError::Arity(name.to_owned(), 2));
    };
    let ((x, dx), (y, dy)) = (real_of(name, x)?, real_of(name, y)?);
    if dx != dy {
        return Err(UnitError::Mismatch(dx, dy).into());
    }
    Ok((x, y, dx))
}

fn dimensionless_real(name: &str, v: &Value) -> Result<f64> {
    let (x, d) = real_of(name, v)?;
    if d != Dimension::NONE {
        return Err(UnitError::NotDimensionless(name.to_owned()).into());
    }
    Ok(x)
}

fn atan2(a: &[Value]) -> Result<Value> {
    let (x, y, _) = same_dimension("atan2", a)?;
    if x == 0.0 && y == 0.0 {
        return Err(EvalError::Invalid("atan2(0, 0) has no angle".into()));
    }
    Ok(Value::dimensionless(y.atan2(x)))
}

fn log(a: &[Value]) -> Result<Value> {
    let (x, base) = match a {
        [x] => (x, None),
        [x, b] => (x, Some(b)),
        _ => return Err(EvalError::Arity("log".into(), 2)),
    };
    let x = scalar_arg("log", std::slice::from_ref(x))?;
    if x.dim != Dimension::NONE {
        return Err(UnitError::NotDimensionless("log".into()).into());
    }
    let b = match base {
        Some(b) => dimensionless_real("log", b)?,
        None => 10.0,
    };
    if b <= 0.0 || b == 1.0 {
        return Err(EvalError::Invalid(
            "a logarithm base must be positive and not 1".into(),
        ));
    }
    if x.z == Complex64::new(0.0, 0.0) {
        return Err(EvalError::NotFinite);
    }
    // Exact for the common bases, so log(1000) is 3 and not 2.9999999999999996.
    let result = if numeric::is_real(x.z) && x.z.re > 0.0 {
        numeric::real(if b == 10.0 {
            x.z.re.log10()
        } else if b == 2.0 {
            x.z.re.log2()
        } else {
            x.z.re.ln() / b.ln()
        })
    } else {
        numeric::div(numeric::ln(x.z), numeric::real(b.ln()))
    };
    Ok(Value::Scalar(Scalar::new(result, Dimension::NONE)))
}

fn round_with(name: &str, a: &[Value], f: fn(f64) -> f64) -> Result<Value> {
    match a {
        [v] => Ok(Value::dimensionless(f(dimensionless_real(name, v)?))),
        _ => Err(EvalError::Arity(name.to_owned(), 1)),
    }
}

fn round(a: &[Value]) -> Result<Value> {
    match a {
        [_] => round_with("round", a, f64::round),
        [v, n] => {
            let x = dimensionless_real("round", v)?;
            let n = dimensionless_real("round", n)?;
            if n.fract() != 0.0 || n.abs() > 15.0 {
                return Err(EvalError::Invalid(
                    "round needs a whole number of decimal places from -15 to 15".into(),
                ));
            }
            let scale = 10f64.powi(n as i32);
            Ok(Value::dimensionless((x * scale).round() / scale))
        }
        _ => Err(EvalError::Arity("round".into(), 2)),
    }
}

fn sign(a: &[Value]) -> Result<Value> {
    let [v] = a else {
        return Err(EvalError::Arity("sign".into(), 1));
    };
    let (x, _) = real_of("sign", v)?;
    Ok(Value::dimensionless(if x == 0.0 {
        0.0
    } else {
        x.signum()
    }))
}

fn modulo(a: &[Value]) -> Result<Value> {
    let (x, y, dim) = same_dimension("mod", a)?;
    if y == 0.0 {
        return Err(EvalError::NotFinite);
    }
    let r = x - y * (x / y).floor();
    Ok(Value::Scalar(Scalar::new(numeric::real(r), dim)))
}

/// Largest whole number the whole-number functions accept: every integer up to
/// 2^53 is exact in an `f64`.
const MAX_WHOLE: f64 = 9_007_199_254_740_992.0;

fn whole_numbers(name: &str, a: &[Value], f: fn(u64, u64) -> Option<u64>) -> Result<Value> {
    let mut acc: Option<u64> = None;
    for v in a {
        let x = dimensionless_real(name, v)?;
        if x.fract() != 0.0 || x.abs() > MAX_WHOLE {
            return Err(EvalError::Invalid(format!(
                "{name} needs whole numbers no larger than 2^53"
            )));
        }
        let x = x.abs() as u64;
        acc =
            match acc {
                None => Some(x),
                Some(prev) => Some(f(prev, x).ok_or_else(|| {
                    EvalError::Invalid(format!("{name}: the result is too large"))
                })?),
            };
    }
    Ok(Value::dimensionless(acc.unwrap_or(0) as f64))
}

fn gcd(mut a: u64, mut b: u64) -> Option<u64> {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    Some(a)
}

fn lcm(a: u64, b: u64) -> Option<u64> {
    if a == 0 || b == 0 {
        return Some(0);
    }
    let r = (a / gcd(a, b)?).checked_mul(b)?;
    (r as f64 <= MAX_WHOLE).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::super::call;
    use super::*;

    fn num(v: f64) -> Value {
        Value::dimensionless(v)
    }

    fn real(name: &str, args: &[f64]) -> f64 {
        let args: Vec<Value> = args.iter().map(|x| num(*x)).collect();
        match call(name, &args).unwrap() {
            Value::Scalar(s) => {
                assert!(numeric::is_real(s.z), "{name} gave a complex result");
                s.z.re
            }
            other => panic!("{other:?}"),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1.0)
    }

    #[test]
    fn inverse_functions_undo_their_partners() {
        for x in [-0.9, -0.3, 0.0, 0.4, 0.99] {
            assert!(close(real("sin", &[real("asin", &[x])]), x));
            assert!(close(real("cos", &[real("acos", &[x])]), x));
            assert!(close(real("tan", &[real("atan", &[x])]), x));
            assert!(close(real("tanh", &[real("atanh", &[x])]), x));
        }
        for x in [-3.0, 0.0, 2.5] {
            assert!(close(real("sinh", &[real("asinh", &[x])]), x));
        }
        assert!(close(real("cosh", &[real("acosh", &[2.5])]), 2.5));
    }

    #[test]
    fn known_values() {
        assert!(close(real("asin", &[1.0]), std::f64::consts::FRAC_PI_2));
        assert!(close(real("atan", &[1.0]), std::f64::consts::FRAC_PI_4));
        assert!(close(real("cosh", &[0.0]), 1.0));
        assert!(close(real("sinh", &[1.0]), 1.175_201_193_643_801_4));
    }

    #[test]
    fn atan2_takes_x_then_y_and_finds_the_quadrant() {
        let half_pi = std::f64::consts::FRAC_PI_2;
        assert!(close(real("atan2", &[0.0, 1.0]), half_pi));
        assert!(close(real("atan2", &[-1.0, 0.0]), std::f64::consts::PI));
        assert!(close(
            real("atan2", &[1.0, -1.0]),
            -std::f64::consts::FRAC_PI_4
        ));
        assert!(call("atan2", &[num(0.0), num(0.0)]).is_err());
    }

    #[test]
    fn atan2_accepts_matching_dimensions_and_refuses_mixed_ones() {
        let m = |v| Value::Scalar(Scalar::new(numeric::real(v), Dimension::LENGTH));
        let s = Value::Scalar(Scalar::new(numeric::real(1.0), Dimension::TIME));
        assert!(call("atan2", &[m(1.0), m(1.0)]).is_ok());
        assert!(call("atan2", &[m(1.0), s]).is_err());
    }

    #[test]
    fn outside_the_real_domain_the_result_is_complex_not_nan() {
        let Value::Scalar(z) = call("asin", &[num(2.0)]).unwrap() else {
            panic!()
        };
        assert!(close(z.z.re, std::f64::consts::FRAC_PI_2));
        assert!(close(z.z.im.abs(), 1.316_957_896_924_816_6));
        let Value::Scalar(z) = call("acosh", &[num(0.5)]).unwrap() else {
            panic!()
        };
        assert!(z.z.im != 0.0);
    }

    #[test]
    fn trigonometry_needs_a_plain_number() {
        let m = Value::Scalar(Scalar::new(numeric::real(1.0), Dimension::LENGTH));
        assert!(call("asin", std::slice::from_ref(&m)).is_err());
        assert!(call("floor", std::slice::from_ref(&m)).is_err());
        assert!(call("sign", &[m]).is_ok());
    }

    #[test]
    fn log_is_base_ten_by_default_and_exact_for_powers() {
        assert_eq!(real("log", &[1000.0]), 3.0);
        assert_eq!(real("log", &[8.0, 2.0]), 3.0);
        assert!(close(real("log", &[81.0, 3.0]), 4.0));
        assert_eq!(real("log", &[1.0]), 0.0);
        assert!(call("log", &[num(10.0), num(1.0)]).is_err());
        assert!(call("log", &[num(10.0), num(-2.0)]).is_err());
        assert!(call("log", &[num(10.0), num(2.0), num(2.0)]).is_err());
        assert!(call("log", &[num(0.0)]).is_err());
    }

    #[test]
    fn a_negative_number_has_a_complex_logarithm() {
        let Value::Scalar(z) = call("log", &[num(-10.0)]).unwrap() else {
            panic!()
        };
        assert!(close(z.z.re, 1.0));
        assert!(close(
            z.z.im,
            std::f64::consts::PI / std::f64::consts::LN_10
        ));
    }

    #[test]
    fn rounding_goes_the_documented_way() {
        assert_eq!(real("floor", &[-1.5]), -2.0);
        assert_eq!(real("ceil", &[-1.5]), -1.0);
        assert_eq!(real("trunc", &[-1.9]), -1.0);
        assert_eq!(real("round", &[2.5]), 3.0);
        assert_eq!(real("round", &[-2.5]), -3.0);
        assert!(close(real("round", &[7.12345, 2.0]), 7.12));
        assert_eq!(real("round", &[1250.0, -2.0]), 1300.0);
        assert!(call("round", &[num(1.0), num(0.5)]).is_err());
        assert!(call("round", &[num(1.0), num(99.0)]).is_err());
    }

    #[test]
    fn mod_takes_the_sign_of_the_divisor() {
        assert_eq!(real("mod", &[7.0, 3.0]), 1.0);
        assert_eq!(real("mod", &[-7.0, 3.0]), 2.0);
        assert_eq!(real("mod", &[7.0, -3.0]), -2.0);
        assert_eq!(
            call("mod", &[num(1.0), num(0.0)]),
            Err(EvalError::NotFinite)
        );
    }

    #[test]
    fn mod_keeps_the_dimension() {
        let m = |v| Value::Scalar(Scalar::new(numeric::real(v), Dimension::LENGTH));
        let Value::Scalar(r) = call("mod", &[m(7.0), m(3.0)]).unwrap() else {
            panic!()
        };
        assert_eq!((r.z.re, r.dim), (1.0, Dimension::LENGTH));
        assert!(call("mod", &[m(7.0), num(3.0)]).is_err());
    }

    #[test]
    fn sign_is_minus_one_zero_or_one() {
        assert_eq!(real("sign", &[-4.0]), -1.0);
        assert_eq!(real("sign", &[0.0]), 0.0);
        assert_eq!(real("sign", &[-0.0]), 0.0);
        assert_eq!(real("sign", &[9.0]), 1.0);
    }

    #[test]
    fn gcd_and_lcm_over_many_arguments() {
        assert_eq!(real("gcd", &[12.0, 18.0]), 6.0);
        assert_eq!(real("gcd", &[12.0, 18.0, 8.0]), 2.0);
        assert_eq!(real("gcd", &[0.0, 5.0]), 5.0);
        assert_eq!(real("lcm", &[4.0, 6.0]), 12.0);
        assert_eq!(real("lcm", &[4.0, 6.0, 10.0]), 60.0);
        assert_eq!(real("lcm", &[0.0, 6.0]), 0.0);
        assert!(call("gcd", &[num(1.5), num(3.0)]).is_err());
        assert!(call("lcm", &[num(1e15), num(999_999_999_999_989.0)]).is_err());
        assert!(call("gcd", &[num(1e300), num(2.0)]).is_err());
    }
}
