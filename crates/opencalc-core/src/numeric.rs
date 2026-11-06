//! The numeric boundary: every floating-point and complex operation the
//! evaluator performs goes through here, so the rest of the engine never depends
//! on a particular number library, and real inputs keep exact real behaviour
//! (`2 ^ 0.5` is real, `sqrt(-4)` is `2i`).

pub use num_complex::Complex64;

use std::f64::consts::PI;

pub fn real(re: f64) -> Complex64 {
    Complex64::new(re, 0.0)
}

pub fn is_real(z: Complex64) -> bool {
    z.im == 0.0
}

pub fn is_finite(z: Complex64) -> bool {
    z.re.is_finite() && z.im.is_finite()
}

/// Quotient. Real operands divide as reals, and complex operands use Smith's
/// algorithm: the textbook formula divides by `|b|^2`, which overflows for
/// `1 / 1e155` and silently turns a tiny answer into zero.
pub fn div(a: Complex64, b: Complex64) -> Complex64 {
    if is_real(a) && is_real(b) {
        return real(a.re / b.re);
    }
    if b.re.abs() >= b.im.abs() {
        let r = b.im / b.re;
        let den = b.re + b.im * r;
        Complex64::new((a.re + a.im * r) / den, (a.im - a.re * r) / den)
    } else {
        let r = b.re / b.im;
        let den = b.re * r + b.im;
        Complex64::new((a.re * r + a.im) / den, (a.im * r - a.re) / den)
    }
}

/// Euclidean length of a list of components without intermediate overflow.
pub fn length(parts: impl Iterator<Item = f64> + Clone) -> f64 {
    let scale = parts.clone().fold(0.0_f64, |m, x| m.max(x.abs()));
    if scale == 0.0 || !scale.is_finite() {
        return scale;
    }
    scale * parts.map(|x| (x / scale) * (x / scale)).sum::<f64>().sqrt()
}

/// Principal square root. A negative real gives an exact imaginary result.
pub fn sqrt(z: Complex64) -> Complex64 {
    if is_real(z) {
        if z.re >= 0.0 {
            real(z.re.sqrt())
        } else {
            Complex64::new(0.0, (-z.re).sqrt())
        }
    } else {
        z.sqrt()
    }
}

/// Natural logarithm, principal branch.
pub fn ln(z: Complex64) -> Complex64 {
    if is_real(z) {
        if z.re >= 0.0 {
            real(z.re.ln())
        } else {
            Complex64::new((-z.re).ln(), PI)
        }
    } else {
        z.ln()
    }
}

pub fn exp(z: Complex64) -> Complex64 {
    if is_real(z) {
        real(z.re.exp())
    } else {
        z.exp()
    }
}

pub fn sin(z: Complex64) -> Complex64 {
    if is_real(z) {
        real(z.re.sin())
    } else {
        z.sin()
    }
}

pub fn cos(z: Complex64) -> Complex64 {
    if is_real(z) {
        real(z.re.cos())
    } else {
        z.cos()
    }
}

pub fn tan(z: Complex64) -> Complex64 {
    if is_real(z) {
        real(z.re.tan())
    } else {
        z.tan()
    }
}

/// `base ^ exp`. Real results stay exactly real; a negative real base with a
/// fractional exponent moves into the complex plane (principal value).
pub fn pow(base: Complex64, exp: Complex64) -> Complex64 {
    if is_real(base) && is_real(exp) {
        if base.re >= 0.0 || exp.re.fract() == 0.0 {
            return real(base.re.powf(exp.re));
        }
        return base.powc(exp);
    }
    if is_real(exp) && exp.re.fract() == 0.0 && exp.re.abs() <= 1024.0 {
        return base.powi(exp.re as i32);
    }
    base.powc(exp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_root_of_a_negative_real_is_exactly_imaginary() {
        assert_eq!(sqrt(real(-4.0)), Complex64::new(0.0, 2.0));
        // A negated zero imaginary part must not flip the branch.
        assert_eq!(sqrt(Complex64::new(-4.0, -0.0)), Complex64::new(0.0, 2.0));
    }

    #[test]
    fn division_does_not_underflow_through_the_squared_modulus() {
        assert_eq!(div(real(1.0), real(1e155)), real(1e-155));
        let z = div(Complex64::new(1.0, 0.0), Complex64::new(1e155, 1e155));
        assert!((z.re - 5e-156).abs() < 1e-170 && (z.im + 5e-156).abs() < 1e-170);
    }

    #[test]
    fn complex_division_matches_the_definition() {
        let z = div(Complex64::new(3.0, 4.0), Complex64::new(1.0, -2.0));
        assert!((z.re + 1.0).abs() < 1e-15 && (z.im - 2.0).abs() < 1e-15);
    }

    #[test]
    fn length_of_huge_components_does_not_overflow() {
        assert!((length([3e200, 4e200].into_iter()) / 5e200 - 1.0).abs() < 1e-15);
        assert_eq!(length([0.0, 0.0].into_iter()), 0.0);
    }

    #[test]
    fn real_powers_stay_real() {
        assert_eq!(pow(real(2.0), real(0.5)), real(2f64.sqrt()));
        assert_eq!(pow(real(-2.0), real(3.0)), real(-8.0));
    }

    #[test]
    fn negative_base_fractional_exponent_is_complex() {
        let z = pow(real(-8.0), real(1.0 / 3.0));
        assert!((z.re - 1.0).abs() < 1e-12 && (z.im - 3f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn logarithm_of_a_negative_real_has_imaginary_part_pi() {
        assert_eq!(ln(real(-1.0)), Complex64::new(0.0, PI));
    }

    #[test]
    fn zero_and_overflow_stay_non_finite_for_the_caller_to_reject() {
        assert!(!is_finite(ln(real(0.0))));
        assert!(!is_finite(pow(real(10.0), real(400.0))));
    }
}
