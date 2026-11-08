//! Whole-number functions: `factorial`, `combin`, `permut`, `isprime`
//! (roadmap Phase 1.2). Every argument must be a dimensionless whole number
//! from 0 to 2^53, and every loop is bounded so no argument can hang the
//! evaluator.

use super::Arity::Exactly;
use super::{FunctionDef, Result};
use crate::eval::EvalError;
use crate::numeric;
use crate::units::{Dimension, UnitError};
use crate::value::{Value, ValueError};

/// Every integer up to 2^53 is exact in an `f64`.
const MAX_WHOLE: f64 = 9_007_199_254_740_992.0;
/// 170! is the largest factorial an `f64` holds.
const MAX_FACTORIAL: u64 = 170;
/// C(n, k) with k this large (and n >= 2k) already exceeds the largest `f64`.
const MAX_COMBIN_K: u64 = 600;

pub(super) const DISCRETE: &[FunctionDef] = &[
    function!(
        "factorial",
        "factorial(n)",
        "n! for a whole number n from 0 to 170",
        Exactly(1),
        |a| {
            let [n] = a else {
                return Err(EvalError::Arity("factorial".into(), 1));
            };
            let n = whole("factorial", n)?;
            if n > MAX_FACTORIAL {
                return Err(too_large("factorial"));
            }
            Ok(Value::dimensionless((2..=n).map(|i| i as f64).product()))
        }
    ),
    function!(
        "combin",
        "combin(n, k)",
        "Ways to choose k of n things, order ignored",
        Exactly(2),
        combin
    ),
    function!(
        "permut",
        "permut(n, k)",
        "Ways to choose k of n things, order counted",
        Exactly(2),
        permut
    ),
    function!(
        "isprime",
        "isprime(n)",
        "1 if the whole number n is prime, otherwise 0",
        Exactly(1),
        |a| {
            let [n] = a else {
                return Err(EvalError::Arity("isprime".into(), 1));
            };
            Ok(Value::dimensionless(f64::from(u8::from(is_prime(whole(
                "isprime", n,
            )?)))))
        }
    ),
];

fn too_large(name: &str) -> EvalError {
    EvalError::Invalid(format!("{name}: the result is too large"))
}

fn whole(name: &str, v: &Value) -> Result<u64> {
    let Value::Scalar(s) = v else {
        return Err(ValueError::Shape(format!("'{name}' needs a single number")).into());
    };
    if s.dim != Dimension::NONE {
        return Err(UnitError::NotDimensionless(name.to_owned()).into());
    }
    if !numeric::is_real(s.z) || s.z.re.fract() != 0.0 || !(0.0..=MAX_WHOLE).contains(&s.z.re) {
        return Err(EvalError::Invalid(format!(
            "{name} needs a whole number from 0 to 2^53"
        )));
    }
    Ok(s.z.re as u64)
}

fn pair(name: &str, a: &[Value]) -> Result<(u64, u64)> {
    let [n, k] = a else {
        return Err(EvalError::Arity(name.to_owned(), 2));
    };
    let (n, k) = (whole(name, n)?, whole(name, k)?);
    if k > n {
        return Err(EvalError::Invalid(format!("{name}: k cannot exceed n")));
    }
    Ok((n, k))
}

fn combin(a: &[Value]) -> Result<Value> {
    let (n, k) = pair("combin", a)?;
    let k = k.min(n - k);
    if k > MAX_COMBIN_K {
        return Err(too_large("combin"));
    }
    // Exact while the running value fits 128 bits: C(m + 1, i) = C(m, i - 1) * (m + 1) / i
    // divides evenly at every step.
    let mut exact: Option<u128> = Some(1);
    for i in 1..=k {
        exact = exact
            .and_then(|acc| acc.checked_mul(u128::from(n - k + i)))
            .map(|p| p / u128::from(i));
        if exact.is_none() {
            break;
        }
    }
    let value = match exact {
        Some(v) => v as f64,
        None => {
            let mut acc = 1.0f64;
            for i in 1..=k {
                acc = acc * (n - k + i) as f64 / i as f64;
            }
            acc
        }
    };
    if !value.is_finite() {
        return Err(too_large("combin"));
    }
    Ok(Value::dimensionless(value))
}

fn permut(a: &[Value]) -> Result<Value> {
    let (n, k) = pair("permut", a)?;
    let mut acc = 1.0f64;
    for i in 0..k {
        acc *= (n - i) as f64;
        // Each factor is at least 1, and all but possibly the last exceed 1, so the
        // product overflows within about 1024 steps: the loop is bounded.
        if !acc.is_finite() {
            return Err(too_large("permut"));
        }
    }
    Ok(Value::dimensionless(acc))
}

fn mul_mod(a: u64, b: u64, m: u64) -> u64 {
    (u128::from(a) * u128::from(b) % u128::from(m)) as u64
}

fn pow_mod(mut base: u64, mut exp: u64, m: u64) -> u64 {
    let mut result = 1 % m;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base, m);
        }
        base = mul_mod(base, base, m);
        exp >>= 1;
    }
    result
}

/// Miller-Rabin with the first twelve primes as witnesses, which is exact for
/// every `u64`.
fn is_prime(n: u64) -> bool {
    const WITNESSES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    if n < 2 {
        return false;
    }
    for p in WITNESSES {
        if n.is_multiple_of(p) {
            return n == p;
        }
    }
    let s = (n - 1).trailing_zeros();
    let d = (n - 1) >> s;
    'witness: for a in WITNESSES {
        let mut x = pow_mod(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul_mod(x, x, n);
            if x == n - 1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::call;
    use super::*;

    fn num(v: f64) -> Value {
        Value::dimensionless(v)
    }

    fn run(name: &str, args: &[f64]) -> f64 {
        let args: Vec<Value> = args.iter().map(|x| num(*x)).collect();
        call(name, &args).unwrap().as_real().unwrap().si
    }

    #[test]
    fn factorials() {
        assert_eq!(run("factorial", &[0.0]), 1.0);
        assert_eq!(run("factorial", &[1.0]), 1.0);
        assert_eq!(run("factorial", &[5.0]), 120.0);
        assert_eq!(run("factorial", &[20.0]), 2_432_902_008_176_640_000.0);
        let big = run("factorial", &[170.0]);
        assert!((big / 7.257_415_615_307_994e306 - 1.0).abs() < 1e-13);
        assert!(call("factorial", &[num(171.0)]).is_err());
        assert!(call("factorial", &[num(-1.0)]).is_err());
        assert!(call("factorial", &[num(2.5)]).is_err());
    }

    #[test]
    fn combinations_and_permutations() {
        assert_eq!(run("combin", &[5.0, 2.0]), 10.0);
        assert_eq!(run("combin", &[5.0, 0.0]), 1.0);
        assert_eq!(run("combin", &[5.0, 5.0]), 1.0);
        assert_eq!(run("combin", &[52.0, 5.0]), 2_598_960.0);
        assert_eq!(run("combin", &[60.0, 30.0]), 118_264_581_564_861_424.0);
        assert_eq!(run("permut", &[5.0, 2.0]), 20.0);
        assert_eq!(run("permut", &[5.0, 0.0]), 1.0);
        assert_eq!(run("permut", &[5.0, 5.0]), 120.0);
        assert!(call("combin", &[num(3.0), num(5.0)]).is_err());
        assert!(call("permut", &[num(3.0), num(5.0)]).is_err());
    }

    #[test]
    fn symmetric_combinations_agree() {
        for (n, k) in [(30.0, 7.0), (100.0, 3.0), (200.0, 195.0)] {
            assert_eq!(run("combin", &[n, k]), run("combin", &[n, n - k]));
        }
    }

    #[test]
    fn huge_arguments_stop_quickly_with_an_error() {
        let big = 9_007_199_254_740_992.0;
        assert!(call("combin", &[num(big), num(big / 2.0)]).is_err());
        assert!(call("permut", &[num(big), num(big / 2.0)]).is_err());
        assert!(call("factorial", &[num(big)]).is_err());
        // Choosing few from a huge set is fine and cheap.
        assert_eq!(run("combin", &[big, 1.0]), big);
        assert_eq!(run("combin", &[big, 0.0]), 1.0);
    }

    #[test]
    fn primes_small_and_large() {
        let primes = [2.0, 3.0, 5.0, 7.0, 97.0, 7919.0, 2_147_483_647.0];
        for p in primes {
            assert_eq!(run("isprime", &[p]), 1.0, "{p}");
        }
        for c in [0.0, 1.0, 4.0, 9.0, 91.0, 561.0, 7917.0, 2_147_483_649.0] {
            assert_eq!(run("isprime", &[c]), 0.0, "{c}");
        }
        // The largest prime below 2^53, and a Carmichael number that fools weak tests.
        assert_eq!(run("isprime", &[9_007_199_254_740_881.0]), 1.0);
        assert_eq!(run("isprime", &[9_007_199_254_740_991.0]), 0.0);
        assert_eq!(
            run("isprime", &[3_215_031_751.0]),
            0.0,
            "a strong pseudoprime to 2, 3, 5, 7"
        );
    }

    #[test]
    fn primes_agree_with_trial_division_below_a_few_thousand() {
        for n in 0..3000u64 {
            let slow = n >= 2 && (2..).take_while(|d| d * d <= n).all(|d| n % d != 0);
            assert_eq!(is_prime(n), slow, "{n}");
        }
    }

    #[test]
    fn arguments_must_be_plain_whole_numbers() {
        let m = Value::from(crate::units::Quantity::from_unit(5.0, "m").unwrap());
        assert!(call("factorial", &[m]).is_err());
        assert!(call("isprime", &[Value::complex(2.0, 1.0)]).is_err());
        assert!(call("isprime", &[num(1e300)]).is_err());
    }
}
