//! `polyroots`: all roots, real and complex, of a polynomial.
//!
//! The coefficients are a vector, lowest power first: `polyroots(vec(2, -3, 1))`
//! is the roots of `2 - 3x + x^2`. Roots at zero are taken out exactly; the rest
//! come from Aberth-Ehrlich iteration, which refines all of them together and
//! finds complex roots from a real start. A root counted twice or more is located
//! only to about the square root of machine precision (1e-8), as usual for
//! repeated roots; the residual check below still accepts it, since the
//! polynomial's value there is as small as it can be.

use super::Arity::Exactly;
use super::{FunctionDef, Result};
use crate::eval::EvalError;
use crate::numeric::{self, Complex64};
use crate::units::Dimension;
use crate::value::{Matrix, Scalar, Value, ValueError};

/// Highest degree accepted (the work grows with its square per iteration).
const MAX_DEGREE: usize = 64;
const MAX_ITERATIONS: usize = 500;
/// Step size, relative, below which the iteration has converged.
const CONVERGED: f64 = 1e-15;
/// Largest accepted |p(root)| relative to the sum of the terms' sizes at the root.
const RESIDUAL: f64 = 1e-8;
/// An imaginary part this small (relative) on a root of a real polynomial is rounding.
const SNAP: f64 = 1e-12;

pub(super) const POLYNOMIAL: &[FunctionDef] = &[function!(
    "polyroots",
    "polyroots(v)",
    "Roots of v[0] + v[1] x + v[2] x^2 + ... as a vector (coefficients lowest power first)",
    Exactly(1),
    polyroots
)];

fn zero() -> Complex64 {
    Complex64::new(0.0, 0.0)
}

fn polyroots(args: &[Value]) -> Result<Value> {
    let [arg] = args else {
        return Err(EvalError::Arity("polyroots".into(), 1));
    };
    let mut c: Vec<Complex64> = match arg {
        Value::Matrix(m) if m.is_vector() => m.elements().to_vec(),
        Value::Series(s) => s.elements().to_vec(),
        _ => {
            return Err(ValueError::Shape("polyroots needs a vector of coefficients".into()).into())
        }
    };
    let real_input = c.iter().all(|z| numeric::is_real(*z));
    while c.last() == Some(&zero()) {
        c.pop();
    }
    if c.len() < 2 {
        return Err(EvalError::Invalid(
            "polyroots needs a polynomial of degree 1 or more".into(),
        ));
    }
    if c.len() - 1 > MAX_DEGREE {
        return Err(EvalError::Invalid(format!(
            "polyroots handles degrees up to {MAX_DEGREE}"
        )));
    }
    // Roots at zero: the lowest coefficients are zero.
    let at_zero = c.iter().take_while(|z| **z == zero()).count();
    c.drain(..at_zero);
    let mut roots = if c.len() >= 2 {
        aberth(&c)?
    } else {
        Vec::new()
    };
    roots.extend(std::iter::repeat_n(zero(), at_zero));
    if real_input {
        for r in &mut roots {
            if r.im.abs() <= SNAP * r.re.abs().max(1.0) {
                r.im = 0.0;
            }
        }
    }
    roots.sort_by(|a, b| a.re.total_cmp(&b.re).then(a.im.total_cmp(&b.im)));
    let items: Vec<Scalar> = roots
        .into_iter()
        .map(|z| Scalar::new(z, Dimension::NONE))
        .collect();
    Ok(Value::Matrix(Matrix::column(&items)?))
}

/// `p(z)` and `p'(z)` by Horner's rule; coefficients lowest power first.
fn horner(c: &[Complex64], z: Complex64) -> (Complex64, Complex64) {
    let (mut p, mut dp) = (zero(), zero());
    for coefficient in c.iter().rev() {
        dp = dp * z + p;
        p = p * z + coefficient;
    }
    (p, dp)
}

fn aberth(c: &[Complex64]) -> Result<Vec<Complex64>> {
    let n = c.len() - 1;
    let radius = (c[0].norm() / c[n].norm()).powf(1.0 / n as f64);
    // Start on a circle, turned off the axes so conjugate roots do not stay tied.
    let mut z: Vec<Complex64> = (0..n)
        .map(|k| Complex64::from_polar(radius, std::f64::consts::TAU * k as f64 / n as f64 + 0.4))
        .collect();
    for _ in 0..MAX_ITERATIONS {
        let mut worst = 0f64;
        for k in 0..n {
            let (p, dp) = horner(c, z[k]);
            if p == zero() {
                continue;
            }
            if dp == zero() {
                // A flat spot: nudge off it.
                z[k] += Complex64::new(1e-3 * radius.max(1e-3), 1e-3 * radius.max(1e-3));
                worst = worst.max(1.0);
                continue;
            }
            let ratio = p / dp;
            let repulsion: Complex64 = (0..n)
                .filter(|&j| j != k)
                .map(|j| z[k] - z[j])
                .filter(|d| *d != zero())
                .map(|d| d.inv())
                .sum();
            let step = ratio / (Complex64::new(1.0, 0.0) - ratio * repulsion);
            z[k] -= step;
            worst = worst.max(step.norm() / z[k].norm().max(1.0));
        }
        if !z.iter().all(|r| numeric::is_finite(*r)) {
            break;
        }
        if worst <= CONVERGED {
            break;
        }
    }
    for r in &z {
        let (p, _) = horner(c, *r);
        let scale: f64 = c
            .iter()
            .enumerate()
            .map(|(i, a)| a.norm() * r.norm().powi(i as i32))
            .sum();
        if !numeric::is_finite(*r) || p.norm() > RESIDUAL * scale {
            return Err(EvalError::Invalid(
                "polyroots did not converge for this polynomial".into(),
            ));
        }
    }
    Ok(z)
}

#[cfg(test)]
mod tests {
    use super::super::call;
    use super::*;

    fn coeffs(items: &[f64]) -> Value {
        let s: Vec<Scalar> = items
            .iter()
            .map(|x| Scalar::new(numeric::real(*x), Dimension::NONE))
            .collect();
        Value::Matrix(Matrix::column(&s).unwrap())
    }

    fn roots(items: &[f64]) -> Vec<Complex64> {
        match call("polyroots", &[coeffs(items)]).unwrap() {
            Value::Matrix(m) => m.elements().to_vec(),
            other => panic!("{other:?}"),
        }
    }

    fn near(z: Complex64, re: f64, im: f64, tol: f64) -> bool {
        (z - Complex64::new(re, im)).norm() <= tol
    }

    #[test]
    fn a_quadratic_with_real_roots_gives_exact_reals_in_order() {
        let r = roots(&[2.0, -3.0, 1.0]);
        assert_eq!(r.len(), 2);
        assert!(near(r[0], 1.0, 0.0, 1e-12) && r[0].im == 0.0);
        assert!(near(r[1], 2.0, 0.0, 1e-12) && r[1].im == 0.0);
    }

    #[test]
    fn complex_roots_come_in_conjugate_pairs() {
        let r = roots(&[1.0, 0.0, 1.0]);
        assert!(
            near(r[0], 0.0, -1.0, 1e-12) && near(r[1], 0.0, 1.0, 1e-12),
            "{r:?}"
        );
        // x^2 - 2x + 5 = 0 has roots 1 +- 2i.
        let r = roots(&[5.0, -2.0, 1.0]);
        assert!(
            near(r[0], 1.0, -2.0, 1e-12) && near(r[1], 1.0, 2.0, 1e-12),
            "{r:?}"
        );
    }

    #[test]
    fn zero_roots_and_trailing_zero_coefficients() {
        // x^3 - x = x (x - 1)(x + 1)
        let r = roots(&[0.0, -1.0, 0.0, 1.0]);
        assert_eq!(r.len(), 3);
        assert!(near(r[0], -1.0, 0.0, 1e-12) && r[1] == zero() && near(r[2], 1.0, 0.0, 1e-12));
        // A zero highest coefficient is ignored.
        assert_eq!(roots(&[2.0, -3.0, 1.0, 0.0]).len(), 2);
        // x^3 has three roots at 0.
        assert!(roots(&[0.0, 0.0, 0.0, 1.0]).iter().all(|z| *z == zero()));
    }

    #[test]
    fn repeated_roots_are_located_to_about_a_hundred_millionth() {
        // (x - 1)^3 = -1 + 3x - 3x^2 + x^3
        for z in roots(&[-1.0, 3.0, -3.0, 1.0]) {
            assert!(near(z, 1.0, 0.0, 1e-4), "{z}");
        }
    }

    #[test]
    fn ten_known_roots() {
        // (x-1)(x-2)...(x-10): a classic hard case, accurate to about 1e-5.
        let mut c = vec![1.0];
        for r in 1..=10 {
            let mut next = vec![0.0; c.len() + 1];
            for (i, a) in c.iter().enumerate() {
                next[i + 1] += a;
                next[i] -= a * f64::from(r);
            }
            c = next;
        }
        let found = roots(&c);
        for (k, z) in found.iter().enumerate() {
            assert!(near(*z, (k + 1) as f64, 0.0, 1e-4), "root {k}: {z}");
        }
    }

    #[test]
    fn complex_coefficients_are_allowed() {
        // x - (1 + 2i)
        let c = Value::Matrix(
            Matrix::column(&[
                Scalar::new(Complex64::new(-1.0, -2.0), Dimension::NONE),
                Scalar::new(numeric::real(1.0), Dimension::NONE),
            ])
            .unwrap(),
        );
        let Value::Matrix(m) = call("polyroots", &[c]).unwrap() else {
            panic!()
        };
        assert!(near(m.elements()[0], 1.0, 2.0, 1e-12));
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert!(call("polyroots", &[coeffs(&[5.0])]).is_err(), "a constant");
        assert!(
            call("polyroots", &[coeffs(&[0.0, 0.0])]).is_err(),
            "all zero"
        );
        assert!(call("polyroots", &[Value::dimensionless(2.0)]).is_err());
        let too_long = vec![1.0; MAX_DEGREE + 2];
        assert!(call("polyroots", &[coeffs(&too_long)]).is_err());
        let wide = Value::Matrix(
            Matrix::from_scalars(1, 2, &[Scalar::new(numeric::real(1.0), Dimension::NONE); 2])
                .unwrap(),
        );
        assert!(
            call("polyroots", &[wide]).is_err(),
            "a row is not a column vector"
        );
    }

    #[test]
    fn the_roots_satisfy_the_polynomial() {
        // A degree-12 polynomial with an awkward mix of coefficients.
        let c: Vec<f64> = (0..13).map(|i| ((i * 7 % 11) as f64) - 4.0).collect();
        let c: Vec<f64> = c.iter().map(|x| if *x == 0.0 { 1.0 } else { *x }).collect();
        let coeffs_c: Vec<Complex64> = c.iter().map(|x| numeric::real(*x)).collect();
        for z in roots(&c) {
            let (p, _) = horner(&coeffs_c, z);
            let scale: f64 = c
                .iter()
                .enumerate()
                .map(|(i, a)| a.abs() * z.norm().powi(i as i32))
                .sum();
            assert!(p.norm() <= 1e-9 * scale, "{z}: {p}");
        }
    }
}
