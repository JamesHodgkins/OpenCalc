//! Definite integrals by adaptive Gauss-Kronrod quadrature (7-point Gauss rule
//! inside a 15-point Kronrod rule; the nodes and weights are the standard ones).
//!
//! The interval is bisected where the estimated error is largest until the total
//! error is within `REL_TOL` of the result (or within `FLOOR` of the integral of
//! the absolute value, for results that cancel to nearly nothing). The end points
//! are never sampled, so an integrable singularity there is allowed, though it
//! converges slowly and may be refused.

use super::super::node::Located;
use super::invalid;
use crate::numeric::Complex64;

pub type Integrand<'a> = dyn FnMut(f64) -> Result<Complex64, Located> + 'a;

/// Relative accuracy asked of the result.
pub const REL_TOL: f64 = 1e-10;
/// Accuracy asked, as a fraction of the integral of |f|, when the result is near 0.
const FLOOR: f64 = 1e-12;
/// Most pieces the interval may be cut into (15 evaluations each).
const MAX_PIECES: usize = 400;

const XGK: [f64; 8] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_4,
    0.586_087_235_467_691_1,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_47,
    0.0,
];
const WGK: [f64; 8] = [
    0.022_935_322_010_529_225,
    0.063_092_092_629_978_55,
    0.104_790_010_322_250_18,
    0.140_653_259_715_525_92,
    0.169_004_726_639_267_9,
    0.190_350_578_064_785_4,
    0.204_432_940_075_298_9,
    0.209_482_141_084_727_83,
];
/// Weights of the Gauss rule, at the Kronrod nodes `XGK[1]`, `XGK[3]`, `XGK[5]`, 0.
const WG: [f64; 4] = [
    0.129_484_966_168_869_7,
    0.279_705_391_489_276_7,
    0.381_830_050_505_118_9,
    0.417_959_183_673_469_4,
];

struct Piece {
    a: f64,
    b: f64,
    value: Complex64,
    err: f64,
    abs: f64,
}

fn rule(f: &mut Integrand, a: f64, b: f64) -> Result<Piece, Located> {
    let (c, h) = (0.5 * (a + b), 0.5 * (b - a));
    let fc = f(c)?;
    let mut resk = fc * WGK[7];
    let mut resg = fc * WG[3];
    let mut resabs = fc.norm() * WGK[7];
    for j in 0..7 {
        let dx = h * XGK[j];
        let (f1, f2) = (f(c - dx)?, f(c + dx)?);
        let sum = f1 + f2;
        resk += sum * WGK[j];
        resabs += (f1.norm() + f2.norm()) * WGK[j];
        if j % 2 == 1 {
            resg += sum * WG[j / 2];
        }
    }
    let abs = resabs * h.abs();
    Ok(Piece {
        a,
        b,
        value: resk * h,
        err: ((resk - resg) * h).norm().max(50.0 * f64::EPSILON * abs),
        abs,
    })
}

/// The integral of `f` from `a` to `b` and its estimated error.
pub fn integrate(f: &mut Integrand, a: f64, b: f64) -> Result<(Complex64, f64), Located> {
    let mut pieces = vec![rule(f, a, b)?];
    loop {
        let total: Complex64 = pieces.iter().map(|p| p.value).sum();
        let err: f64 = pieces.iter().map(|p| p.err).sum();
        let abs: f64 = pieces.iter().map(|p| p.abs).sum();
        if err <= (REL_TOL * total.norm()).max(FLOOR * abs) {
            return Ok((total, err));
        }
        let worst = pieces
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.err.total_cmp(&y.1.err))
            .map_or(0, |(i, _)| i);
        let (lo, hi) = (pieces[worst].a, pieces[worst].b);
        let mid = 0.5 * (lo + hi);
        if pieces.len() >= MAX_PIECES || mid <= lo.min(hi) || mid >= lo.max(hi) {
            return Err(invalid(format!(
                "the integral did not converge (estimated error {err:.1e})"
            )));
        }
        pieces.swap_remove(worst);
        pieces.push(rule(f, lo, mid)?);
        pieces.push(rule(f, mid, hi)?);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real(f: impl Fn(f64) -> f64, a: f64, b: f64) -> Result<(f64, f64), String> {
        let mut g = |x: f64| Ok(Complex64::new(f(x), 0.0));
        integrate(&mut g, a, b)
            .map(|(v, e)| (v.re, e))
            .map_err(|l| l.error.to_string())
    }

    #[test]
    fn the_weights_sum_to_the_interval_length() {
        let kronrod: f64 = 2.0 * WGK[..7].iter().sum::<f64>() + WGK[7];
        assert!((kronrod - 2.0).abs() < 1e-15, "{kronrod}");
        let gauss: f64 = 2.0 * (WG[0] + WG[1] + WG[2]) + WG[3];
        assert!((gauss - 2.0).abs() < 1e-15, "{gauss}");
    }

    #[test]
    fn polynomials_are_exact() {
        // The 15-point rule is exact to degree 22.
        let (v, _) = real(|x| x.powi(10), 0.0, 1.0).unwrap();
        assert!((v - 1.0 / 11.0).abs() < 1e-15);
        let (v, _) = real(|x| 3.0 * x * x - 2.0 * x + 1.0, -1.0, 2.0).unwrap();
        assert!((v - 9.0).abs() < 1e-13, "{v}");
    }

    #[test]
    fn known_integrals() {
        let pi = std::f64::consts::PI;
        let (v, _) = real(f64::sin, 0.0, pi).unwrap();
        assert!((v - 2.0).abs() < 1e-10);
        let (v, _) = real(f64::exp, 0.0, 1.0).unwrap();
        assert!((v - (std::f64::consts::E - 1.0)).abs() < 1e-10);
        let (v, _) = real(|x| 1.0 / (1.0 + x * x), -1.0, 1.0).unwrap();
        assert!((v - pi / 2.0).abs() < 1e-10);
        let (v, _) = real(|x| (-x * x).exp(), -6.0, 6.0).unwrap();
        assert!((v - pi.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn reversed_bounds_negate_and_equal_bounds_give_zero() {
        let (v, _) = real(|x| x, 2.0, 0.0).unwrap();
        assert!((v + 2.0).abs() < 1e-13);
        let (v, _) = real(|x| x, 1.0, 1.0).unwrap();
        assert_eq!(v, 0.0);
    }

    #[test]
    fn a_result_that_cancels_to_zero_is_still_accepted() {
        let (v, _) = real(f64::sin, -3.0, 3.0).unwrap();
        assert!(v.abs() < 1e-12, "{v}");
    }

    #[test]
    fn a_mild_end_point_singularity_converges() {
        let (v, _) = real(|x| x.sqrt(), 0.0, 1.0).unwrap();
        assert!((v - 2.0 / 3.0).abs() < 1e-8, "{v}");
    }

    #[test]
    fn a_pole_inside_the_interval_is_refused_not_guessed() {
        let e = real(|x| 1.0 / x, -1.0, 2.0).unwrap_err();
        assert!(e.contains("did not converge"), "{e}");
    }

    #[test]
    fn a_complex_integrand_integrates_both_parts() {
        let mut g = |x: f64| Ok(Complex64::new(x, 1.0));
        let (v, _) = integrate(&mut g, 0.0, 2.0).ok().unwrap();
        assert!((v - Complex64::new(2.0, 2.0)).norm() < 1e-13);
    }

    #[test]
    fn work_is_bounded() {
        let mut calls = 0u32;
        let mut g = |x: f64| {
            calls += 1;
            Ok(Complex64::new((1.0 / x).sin(), 0.0))
        };
        let _ = integrate(&mut g, 0.0, 1.0);
        assert!(calls <= 15 * (2 * MAX_PIECES as u32), "{calls} evaluations");
    }
}
