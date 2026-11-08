//! Derivatives (Ridders' extrapolated central differences) and two-sided limits
//! (Richardson extrapolation of samples on both sides).
//!
//! Both report the best extrapolation together with an error estimate, and refuse
//! to answer when the estimate is not small: a pole, a jump, a kink or noise gives
//! an error, not a number. Tolerances are relative to the result and to the
//! largest value the function took while sampling.

use super::super::node::Located;
use super::invalid;
use crate::numeric::Complex64;

pub type Function<'a> = dyn FnMut(f64) -> Result<Complex64, Located> + 'a;

/// Shrink factor of the step at each level, and its square.
const CON: f64 = 1.4;
const CON2: f64 = CON * CON;
/// Levels of the derivative tableau.
const NTAB: usize = 12;
/// Stop when the error grows to this many times the best so far.
const SAFE: f64 = 2.0;
/// Largest accepted error estimate of a derivative, relative.
const DERIVATIVE_TOL: f64 = 1e-6;
/// Levels of the limit sequence on each side.
const LEVELS: usize = 9;
/// Largest accepted error estimate of a one-sided limit, and disagreement of the
/// two sides, relative.
const LIMIT_TOL: f64 = 1e-7;

/// A function and the largest |f| it has returned.
struct Probe<'f, 'a> {
    f: &'f mut Function<'a>,
    peak: f64,
}

impl Probe<'_, '_> {
    fn at(&mut self, x: f64) -> Result<Complex64, Located> {
        let v = (self.f)(x)?;
        self.peak = self.peak.max(v.norm());
        Ok(v)
    }
}

/// f'(x0), by extrapolating central differences to zero step.
pub fn derivative(f: &mut Function, x0: f64) -> Result<Complex64, Located> {
    let mut p = Probe { f, peak: 0.0 };
    let scale = x0.abs().max(1.0);
    let h0 = 0.05 * scale;
    let mut h = h0;
    let mut a = [[Complex64::new(0.0, 0.0); NTAB]; NTAB];
    let central = |p: &mut Probe, h: f64| -> Result<Complex64, Located> {
        Ok((p.at(x0 + h)? - p.at(x0 - h)?) / (2.0 * h))
    };
    a[0][0] = central(&mut p, h)?;
    let (mut best, mut err) = (a[0][0], f64::MAX);
    for i in 1..NTAB {
        h /= CON;
        a[0][i] = central(&mut p, h)?;
        let mut fac = CON2;
        for j in 1..=i {
            a[j][i] = (a[j - 1][i] * fac - a[j - 1][i - 1]) / (fac - 1.0);
            fac *= CON2;
            let e = (a[j][i] - a[j - 1][i])
                .norm()
                .max((a[j][i] - a[j - 1][i - 1]).norm());
            if e <= err {
                err = e;
                best = a[j][i];
            }
        }
        if (a[i][i] - a[i - 1][i - 1]).norm() >= SAFE * err {
            break;
        }
    }
    let typical = p.peak / h0;
    if err > DERIVATIVE_TOL * best.norm().max(typical) {
        return Err(invalid(format!(
            "the derivative did not converge (estimated error {err:.1e})"
        )));
    }
    // Central differences cannot see a kink such as |x| at 0 (both sides cancel).
    // Compare the one-sided slopes at two step sizes: across a kink their
    // difference stays the same as the step shrinks; for a smooth function it halves.
    let gap = |p: &mut Probe, h: f64| -> Result<f64, Located> {
        let f0 = p.at(x0)?;
        let right = (p.at(x0 + h)? - f0) / h;
        let left = (f0 - p.at(x0 - h)?) / h;
        Ok((right - left).norm())
    };
    let h1 = 1e-6 * scale;
    let (d1, d2) = (gap(&mut p, h1)?, gap(&mut p, h1 / 2.0)?);
    if d1 > 1e-7 * typical && d2 > 0.8 * d1 {
        return Err(invalid(
            "the function has a corner at this point, so it has no derivative there",
        ));
    }
    Ok(best)
}

/// One side of a limit: the extrapolated value and its error estimate.
fn one_side(p: &mut Probe, x0: f64, h0: f64, sign: f64) -> Result<(Complex64, f64), Located> {
    let mut t = [[Complex64::new(0.0, 0.0); LEVELS]; LEVELS];
    let mut h = h0;
    let (mut best, mut err) = (Complex64::new(0.0, 0.0), f64::MAX);
    for k in 0..LEVELS {
        t[k][0] = p.at(x0 + sign * h)?;
        if k == 0 {
            best = t[0][0];
        }
        let mut pow = 1.0;
        for j in 1..=k {
            pow *= 2.0;
            t[k][j] = t[k][j - 1] + (t[k][j - 1] - t[k - 1][j - 1]) / (pow - 1.0);
            let e = (t[k][j] - t[k][j - 1])
                .norm()
                .max((t[k][j] - t[k - 1][j - 1]).norm());
            if e <= err {
                err = e;
                best = t[k][j];
            }
        }
        h /= 2.0;
    }
    Ok((best, err))
}

/// The two-sided limit of f as x approaches x0. f is never evaluated at x0.
pub fn limit(f: &mut Function, x0: f64) -> Result<Complex64, Located> {
    let mut p = Probe { f, peak: 0.0 };
    let h0 = 0.25 * x0.abs().max(1.0);
    let (up, up_err) = one_side(&mut p, x0, h0, 1.0)?;
    let (down, down_err) = one_side(&mut p, x0, h0, -1.0)?;
    let scale = up.norm().max(down.norm());
    let tol = |extra: f64| LIMIT_TOL * scale.max(extra);
    if up_err > tol(1e-3 * p.peak) || down_err > tol(1e-3 * p.peak) {
        return Err(invalid(format!(
            "the limit did not converge (estimated error {:.1e})",
            up_err.max(down_err)
        )));
    }
    if (up - down).norm() > 1e-2 * LIMIT_TOL * scale.max(1e-3 * p.peak) {
        return Err(invalid(
            "the limit does not exist: the two sides approach different values",
        ));
    }
    Ok((up + down) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(f: impl Fn(f64) -> f64, x: f64) -> Result<f64, String> {
        let mut g = |x: f64| Ok(Complex64::new(f(x), 0.0));
        derivative(&mut g, x)
            .map(|v| v.re)
            .map_err(|l| l.error.to_string())
    }

    fn lim(f: impl Fn(f64) -> f64, x: f64) -> Result<f64, String> {
        let mut g = |x: f64| Ok(Complex64::new(f(x), 0.0));
        limit(&mut g, x)
            .map(|v| v.re)
            .map_err(|l| l.error.to_string())
    }

    #[test]
    fn derivatives_of_known_functions() {
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-8 * b.abs().max(1.0);
        assert!(close(d(|x| x * x, 3.0).unwrap(), 6.0));
        assert!(close(d(f64::sin, 0.0).unwrap(), 1.0));
        assert!(close(d(f64::exp, 2.0).unwrap(), 2f64.exp()));
        assert!(close(d(f64::ln, 5.0).unwrap(), 0.2));
        assert!(close(d(|_| 7.0, 1.0).unwrap(), 0.0));
        assert!(close(d(|x| x.powi(5), -2.0).unwrap(), 80.0));
        assert!(close(d(|x| 1e6 + x, 0.0).unwrap(), 1.0));
    }

    #[test]
    fn a_smooth_extremum_is_not_mistaken_for_a_corner() {
        assert!(d(f64::cos, 0.0).unwrap().abs() < 1e-8);
        assert!(
            d(|x| (100.0 * x).sin(), std::f64::consts::PI / 200.0)
                .unwrap()
                .abs()
                < 1e-5
        );
    }

    #[test]
    fn a_corner_has_no_derivative() {
        let e = d(f64::abs, 0.0).unwrap_err();
        assert!(e.contains("corner"), "{e}");
        assert!(d(f64::abs, 1.0).is_ok(), "away from the corner it is fine");
    }

    #[test]
    fn a_jump_or_pole_has_no_derivative() {
        assert!(d(|x| if x < 0.0 { 0.0 } else { 1.0 }, 0.0).is_err());
        assert!(d(|x| 1.0 / x, 0.0).is_err());
    }

    #[test]
    fn limits_that_exist() {
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-8 * b.abs().max(1.0);
        assert!(close(lim(|x| x.sin() / x, 0.0).unwrap(), 1.0));
        assert!(close(lim(|x| (1.0 - x.cos()) / (x * x), 0.0).unwrap(), 0.5));
        assert!(close(lim(|x| (x * x - 1.0) / (x - 1.0), 1.0).unwrap(), 2.0));
        assert!(close(lim(|x| (x.exp() - 1.0) / x, 0.0).unwrap(), 1.0));
        assert!(close(lim(|x| x * x, 3.0).unwrap(), 9.0), "continuous: f(a)");
    }

    #[test]
    fn limits_that_do_not_exist_are_errors() {
        let e = lim(|x| x.abs() / x, 0.0).unwrap_err();
        assert!(e.contains("does not exist"), "{e}");
        assert!(lim(|x| 1.0 / x, 0.0).is_err(), "a pole");
        assert!(lim(|x| (1.0 / x).sin(), 0.0).is_err(), "oscillation");
    }
}
