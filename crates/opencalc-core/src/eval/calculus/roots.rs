//! Real roots of a function of one variable.
//!
//! With a bracket `[a, b]` the root is found by Brent's method, which is
//! guaranteed to converge when `f(a)` and `f(b)` have opposite signs. With only a
//! guess, the search steps outward from it (growing by 1.6 each time, to both
//! sides) until `f` changes sign, then runs the same method on that bracket. So a
//! bad guess gives an error or a different root, never a wandering iteration.
//! Complex roots are not searched for.
//!
//! The root is accurate to about `2 eps |x|` plus `1e-14` of the bracket's size.

use super::super::node::Located;
use super::invalid;

pub type RealFn<'a> = dyn FnMut(f64) -> Result<f64, Located> + 'a;

/// Most Brent iterations (each is one evaluation of `f`).
const MAX_ITERATIONS: usize = 200;
/// Most outward steps when starting from a guess.
const MAX_EXPANSIONS: usize = 60;
/// Absolute accuracy as a fraction of the bracket's size.
const SIZE_TOL: f64 = 1e-14;

fn opposite(a: f64, b: f64) -> bool {
    (a > 0.0) != (b > 0.0)
}

/// The root between `a` and `b`, which must bracket one.
pub fn bracketed(f: &mut RealFn, a: f64, b: f64) -> Result<f64, Located> {
    if a == b {
        return Err(invalid("the two ends of the bracket are the same"));
    }
    let (fa, fb) = (f(a)?, f(b)?);
    if fa == 0.0 {
        return Ok(a);
    }
    if fb == 0.0 {
        return Ok(b);
    }
    if !opposite(fa, fb) {
        return Err(invalid(
            "f has the same sign at both ends, so a root between them is not guaranteed",
        ));
    }
    brent(f, a, b, fa, fb)
}

/// A root near `x0`.
pub fn from_guess(f: &mut RealFn, x0: f64) -> Result<f64, Located> {
    let f0 = f(x0)?;
    if f0 == 0.0 {
        return Ok(x0);
    }
    let mut h = 0.1 * x0.abs().max(1.0);
    let mut alive = [true, true];
    let mut first_error: Option<Located> = None;
    for _ in 0..MAX_EXPANSIONS {
        for (side, sign) in [1.0, -1.0].into_iter().enumerate() {
            if !alive[side] {
                continue;
            }
            let x = x0 + sign * h;
            match f(x) {
                Ok(0.0) => return Ok(x),
                Ok(fx) if opposite(f0, fx) => return brent(f, x0, x, f0, fx),
                Ok(_) => {}
                // A side where f cannot be evaluated (a domain error) is dropped;
                // the other side may still hold a root.
                Err(e) => {
                    alive[side] = false;
                    first_error.get_or_insert(e);
                }
            }
        }
        if !alive[0] && !alive[1] {
            break;
        }
        h *= 1.6;
    }
    Err(first_error.unwrap_or_else(|| {
        invalid("no sign change found near the guess; give a bracket [a, b] instead")
    }))
}

/// Brent's method on a bracket whose end values `fa`, `fb` have opposite signs.
fn brent(f: &mut RealFn, a0: f64, b0: f64, fa0: f64, fb0: f64) -> Result<f64, Located> {
    let size = a0.abs().max(b0.abs());
    let (mut a, mut b, mut c) = (a0, b0, b0);
    let (mut fa, mut fb, mut fc) = (fa0, fb0, fb0);
    let (mut d, mut e) = (b - a, b - a);
    for _ in 0..MAX_ITERATIONS {
        if (fb > 0.0 && fc > 0.0) || (fb < 0.0 && fc < 0.0) {
            // b and c are on the same side: restart the bracket from a.
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            (a, b, c) = (b, c, b);
            (fa, fb, fc) = (fb, fc, fb);
        }
        let tol = 2.0 * f64::EPSILON * b.abs() + 0.5 * SIZE_TOL * size;
        let xm = 0.5 * (c - b);
        if xm.abs() <= tol || fb == 0.0 {
            // A sign change at a pole (1/x at 0) closes in on the jump, where the
            // function is huge. A root is where it is small.
            if fb.abs() >= fa0.abs().max(fb0.abs()) {
                return Err(invalid(
                    "f changes sign at a discontinuity here, not at a root",
                ));
            }
            return Ok(b);
        }
        if e.abs() >= tol && fa.abs() > fb.abs() {
            // Interpolate: secant if only two points are distinct, else inverse quadratic.
            let s = fb / fa;
            let (mut p, mut q);
            if a == c {
                p = 2.0 * xm * s;
                q = 1.0 - s;
            } else {
                let (qq, r) = (fa / fc, fb / fc);
                p = s * (2.0 * xm * qq * (qq - r) - (b - a) * (r - 1.0));
                q = (qq - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            }
            p = p.abs();
            // Take the interpolated step only if it stays inside the bracket and
            // shrinks fast enough; otherwise bisect.
            if 2.0 * p < (3.0 * xm * q - (tol * q).abs()).min((e * q).abs()) {
                e = d;
                d = p / q;
            } else {
                d = xm;
                e = d;
            }
        } else {
            d = xm;
            e = d;
        }
        a = b;
        fa = fb;
        b += if d.abs() > tol { d } else { tol.copysign(xm) };
        fb = f(b)?;
    }
    Err(invalid("the root search did not converge"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(g: impl Fn(f64) -> f64, range: Option<(f64, f64)>, guess: f64) -> Result<f64, String> {
        let mut f = |x: f64| Ok(g(x));
        match range {
            Some((a, b)) => bracketed(&mut f, a, b),
            None => from_guess(&mut f, guess),
        }
        .map_err(|l| l.error.to_string())
    }

    #[test]
    fn brackets_find_known_roots() {
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * b.abs().max(1.0);
        assert!(close(
            find(|x| x * x - 2.0, Some((0.0, 2.0)), 0.0).unwrap(),
            2f64.sqrt()
        ));
        assert!(close(
            find(f64::cos, Some((0.0, 3.0)), 0.0).unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
        assert!(close(
            find(|x| x.exp() - 5.0, Some((0.0, 10.0)), 0.0).unwrap(),
            5f64.ln()
        ));
        assert!(
            close(
                find(|x| x.powi(3) - 8.0, Some((10.0, -10.0)), 0.0).unwrap(),
                2.0
            ),
            "ends in either order"
        );
    }

    #[test]
    fn a_root_at_zero_or_at_an_end_is_exact_enough() {
        let r = find(|x| x.powi(3), Some((-1.0, 1.0)), 0.0).unwrap();
        assert!(
            r.abs() < 1e-4,
            "a triple root is flat, so only roughly located: {r}"
        );
        assert_eq!(find(|x| x, Some((0.0, 1.0)), 0.0).unwrap(), 0.0);
        assert_eq!(find(|x| x - 1.0, Some((0.0, 1.0)), 0.0).unwrap(), 1.0);
        let r = find(|x| 3.0 * x - 1e-9, Some((-1.0, 1.0)), 0.0).unwrap();
        assert!((r - 3.333_333_333_333_333e-10).abs() < 1e-14, "{r}");
    }

    #[test]
    fn no_sign_change_is_refused() {
        let e = find(|x| x * x + 1.0, Some((-1.0, 1.0)), 0.0).unwrap_err();
        assert!(e.contains("same sign"), "{e}");
        assert!(find(|x| x, Some((1.0, 1.0)), 0.0).is_err());
    }

    #[test]
    fn a_guess_finds_the_nearest_sign_change_on_either_side() {
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-10 * b.abs().max(1.0);
        assert!(close(
            find(|x| x * x - 2.0, None, 1.0).unwrap(),
            2f64.sqrt()
        ));
        assert!(close(
            find(|x| x * x - 2.0, None, -1.0).unwrap(),
            -(2f64.sqrt())
        ));
        assert!(close(
            find(|x| x.exp() - 1000.0, None, 0.0).unwrap(),
            1000f64.ln()
        ));
        assert_eq!(find(|x| x - 3.0, None, 3.0).unwrap(), 3.0);
        // A far-away root is still reached by the growing steps.
        assert!(close(find(|x| x - 1e6, None, 0.0).unwrap(), 1e6));
    }

    #[test]
    fn a_guess_survives_a_domain_error_on_one_side() {
        // ln is undefined (here: an error) for x <= 0; the root at e is to the right.
        let mut f = |x: f64| {
            if x <= 0.0 {
                Err(invalid("domain"))
            } else {
                Ok(x.ln() - 1.0)
            }
        };
        let r = from_guess(&mut f, 0.5).ok().unwrap();
        assert!((r - std::f64::consts::E).abs() < 1e-10);
    }

    #[test]
    fn a_function_with_no_root_gives_an_error_in_bounded_work() {
        let mut calls = 0;
        let mut f = |x: f64| {
            calls += 1;
            Ok(x * x + 1.0)
        };
        assert!(from_guess(&mut f, 0.0).is_err());
        assert!(calls <= 2 * MAX_EXPANSIONS + 1, "{calls}");
    }

    #[test]
    fn a_sign_change_at_a_pole_or_jump_is_not_a_root() {
        let e = find(|x| 1.0 / x, Some((-1.0, 2.0)), 0.0).unwrap_err();
        assert!(e.contains("discontinuity"), "{e}");
        let e = find(|x| if x < 0.5 { -1.0 } else { 1.0 }, Some((0.0, 1.0)), 0.0).unwrap_err();
        assert!(e.contains("discontinuity"), "{e}");
        // A steep but continuous crossing is still a root.
        assert!(find(|x| (1e6 * (x - 0.3)).tanh(), Some((0.0, 1.0)), 0.0).is_ok());
    }

    #[test]
    fn iterations_are_bounded() {
        let mut calls = 0;
        let mut f = |x: f64| {
            calls += 1;
            Ok((1e6 * x).tanh() + 0.3 * (50.0 * x).sin())
        };
        let _ = bracketed(&mut f, -1.0, 1.0);
        assert!(calls <= MAX_ITERATIONS + 2, "{calls}");
    }
}
