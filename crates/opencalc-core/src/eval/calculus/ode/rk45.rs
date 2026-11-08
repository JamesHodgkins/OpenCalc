//! Adaptive Runge-Kutta (Dormand-Prince 5(4)) for `y' = f(t, y)`.
//!
//! Each step computes a fifth-order result and a fourth-order one; their
//! difference is the error estimate. A step is accepted when that error is within
//! tolerance and the step size for the next one is chosen from it. The last
//! derivative of an accepted step is the first of the next ("first same as last").
//!
//! Tolerance is `RTOL` of the state plus a small absolute part scaled to the
//! problem (the size of the initial state, or of how far the derivative would
//! carry it over the whole span), so it means the same in metres and in
//! nanometres. A trial step where `f` cannot be evaluated or gives a non-finite
//! state is only a rejected step; running out of steps or being stopped is an
//! error. The solution is reached at exactly `t1`, and at `segments` equally
//! spaced times on the way when more than one is asked for.

use crate::eval::node::Located;

use super::super::invalid;

pub type Rhs<'a> = dyn FnMut(f64, &[f64]) -> Result<Vec<f64>, Located> + 'a;

/// The outcome of one trial step: the new state, the derivative there and the
/// scaled error; `None` when the step could not be taken.
type Trial = Option<(Vec<f64>, Vec<f64>, f64)>;

/// Most steps (accepted or rejected) over the whole solution.
const MAX_STEPS: usize = 100_000;
/// Relative tolerance of the state.
const RTOL: f64 = 1e-9;
/// Absolute tolerance as a fraction of the problem's scale.
const ATOL: f64 = 1e-10;

const C: [f64; 7] = [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];
const A: [[f64; 6]; 7] = [
    [0.0; 6],
    [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0],
    [
        19372.0 / 6561.0,
        -25360.0 / 2187.0,
        64448.0 / 6561.0,
        -212.0 / 729.0,
        0.0,
        0.0,
    ],
    [
        9017.0 / 3168.0,
        -355.0 / 33.0,
        46732.0 / 5247.0,
        49.0 / 176.0,
        -5103.0 / 18656.0,
        0.0,
    ],
    [
        35.0 / 384.0,
        0.0,
        500.0 / 1113.0,
        125.0 / 192.0,
        -2187.0 / 6784.0,
        11.0 / 84.0,
    ],
];
/// Fifth-order weights minus fourth-order weights: the error estimate.
const E: [f64; 7] = [
    71.0 / 57600.0,
    0.0,
    -71.0 / 16695.0,
    71.0 / 1920.0,
    -17253.0 / 339200.0,
    22.0 / 525.0,
    -1.0 / 40.0,
];

struct State {
    t: f64,
    y: Vec<f64>,
    /// `f(t, y)`, carried from one step to the next.
    k1: Vec<f64>,
    /// The step size to try next.
    h: f64,
    steps: usize,
}

/// The states at `t0 + k (t1 - t0) / segments` for `k = 0 ..= segments`.
pub fn solve(
    f: &mut Rhs,
    t0: f64,
    y0: &[f64],
    t1: f64,
    segments: usize,
) -> Result<Vec<Vec<f64>>, Located> {
    let k1 = f(t0, y0)?;
    let span = (t1 - t0).abs();
    let atol: Vec<f64> = y0
        .iter()
        .zip(&k1)
        .map(|(y, d)| {
            let scale = y.abs().max(d.abs() * span);
            ATOL * if scale > 0.0 { scale } else { 1.0 }
        })
        .collect();
    let mut out = vec![y0.to_vec()];
    if span == 0.0 {
        out.resize(segments + 1, y0.to_vec());
        return Ok(out);
    }
    let mut s = State {
        t: t0,
        y: y0.to_vec(),
        k1,
        h: 0.01 * (t1 - t0),
        steps: 0,
    };
    for k in 1..=segments {
        // Computed from the ends, so the last segment lands exactly on t1.
        let target = if k == segments {
            t1
        } else {
            t0 + (t1 - t0) * k as f64 / segments as f64
        };
        advance(f, &mut s, target, &atol)?;
        out.push(s.y.clone());
    }
    Ok(out)
}

fn advance(f: &mut Rhs, s: &mut State, target: f64, atol: &[f64]) -> Result<(), Located> {
    let direction = (target - s.t).signum();
    let n = s.y.len();
    while (target - s.t) * direction > 0.0 {
        if s.steps >= MAX_STEPS {
            return Err(invalid(format!(
                "odesolve needed more than {MAX_STEPS} steps: the equation may be stiff or singular"
            )));
        }
        s.steps += 1;
        let mut h = s.h;
        let last = (s.t + h - target) * direction >= 0.0;
        if last {
            h = target - s.t;
        }
        if h.abs() < 1e-14 * s.t.abs().max(1.0) {
            return Err(invalid(
                "odesolve's step became too small: the solution may be singular here",
            ));
        }
        match trial(f, s, h, atol, n) {
            Ok(Some((y_new, k_new, error))) if error <= 1.0 => {
                s.t = if last { target } else { s.t + h };
                s.y = y_new;
                s.k1 = k_new;
                let grown = h * factor(error);
                s.h = if last {
                    grown.abs().max(s.h.abs()) * direction
                } else {
                    grown
                };
            }
            Ok(Some((_, _, error))) => s.h = h * factor(error).min(1.0),
            // The step could not be evaluated (a domain error, or a non-finite
            // state): try a much smaller one.
            Ok(None) => s.h = h * 0.2,
            Err(l) => return Err(l),
        }
    }
    Ok(())
}

/// How much to change the step by, for a scaled error (1 is exactly on tolerance).
fn factor(error: f64) -> f64 {
    if !error.is_finite() {
        0.2
    } else if error == 0.0 {
        5.0
    } else {
        (0.9 * error.powf(-0.2)).clamp(0.2, 5.0)
    }
}

/// One step of size `h`: the new state, the derivative there and the scaled error,
/// or `None` if `f` failed at a stage or the result is not finite.
fn trial(f: &mut Rhs, s: &State, h: f64, atol: &[f64], n: usize) -> Result<Trial, Located> {
    let mut k: Vec<Vec<f64>> = Vec::with_capacity(7);
    k.push(s.k1.clone());
    for stage in 1..7 {
        let y: Vec<f64> = (0..n)
            .map(|i| s.y[i] + h * (0..stage).map(|j| A[stage][j] * k[j][i]).sum::<f64>())
            .collect();
        match f(s.t + C[stage] * h, &y) {
            Ok(d) if d.len() == n && d.iter().all(|v| v.is_finite()) => k.push(d),
            Ok(_) => return Ok(None),
            Err(l) if l.error.is_stop() => return Err(l),
            Err(_) => return Ok(None),
        }
        if stage == 6 {
            // The seventh stage is evaluated at the fifth-order result.
            if y.iter().any(|v| !v.is_finite()) {
                return Ok(None);
            }
            let error = (0..n)
                .map(|i| {
                    let e = h * (0..7).map(|j| E[j] * k[j][i]).sum::<f64>();
                    let scale = atol[i] + RTOL * s.y[i].abs().max(y[i].abs());
                    (e / scale).abs()
                })
                .fold(0.0f64, f64::max);
            let derivative = k.pop().unwrap_or_default();
            return Ok(Some((y, derivative, error)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        f: impl Fn(f64, &[f64]) -> Vec<f64>,
        t0: f64,
        y0: &[f64],
        t1: f64,
        segments: usize,
    ) -> Result<Vec<Vec<f64>>, String> {
        solve(&mut |t, y| Ok(f(t, y)), t0, y0, t1, segments).map_err(|l| l.error.to_string())
    }

    #[test]
    fn the_weights_and_nodes_are_consistent() {
        // Each row of A sums to its node; the fifth-order weights sum to 1 and the
        // error weights to 0.
        for (row, c) in A.iter().zip(C) {
            assert!((row.iter().sum::<f64>() - c).abs() < 1e-14, "{row:?}");
        }
        assert!((A[6].iter().sum::<f64>() - 1.0).abs() < 1e-14);
        assert!(E.iter().sum::<f64>().abs() < 1e-14);
    }

    #[test]
    fn exponential_growth_and_decay() {
        let r = run(|_, y| vec![y[0]], 0.0, &[1.0], 1.0, 1).unwrap();
        assert!((r[1][0] - std::f64::consts::E).abs() < 1e-8 * std::f64::consts::E);
        let r = run(|_, y| vec![-2.0 * y[0]], 0.0, &[3.0], 5.0, 1).unwrap();
        assert!((r[1][0] - 3.0 * (-10f64).exp()).abs() < 1e-9);
    }

    #[test]
    fn a_harmonic_oscillator_keeps_its_amplitude() {
        // y'' = -y as (y, v): after ten full turns it is back at (1, 0).
        let tau = std::f64::consts::TAU;
        let r = run(|_, y| vec![y[1], -y[0]], 0.0, &[1.0, 0.0], 10.0 * tau, 1).unwrap();
        assert!(
            (r[1][0] - 1.0).abs() < 1e-6 && r[1][1].abs() < 1e-6,
            "{:?}",
            r[1]
        );
    }

    #[test]
    fn the_time_dependent_right_hand_side_is_followed() {
        // y' = 2t, y(0) = 1  ->  y = 1 + t^2.
        let r = run(|t, _| vec![2.0 * t], 0.0, &[1.0], 3.0, 1).unwrap();
        assert!((r[1][0] - 10.0).abs() < 1e-8);
    }

    #[test]
    fn backwards_in_time() {
        let e = std::f64::consts::E;
        let r = run(|_, y| vec![y[0]], 1.0, &[e], 0.0, 1).unwrap();
        assert!((r[1][0] - 1.0).abs() < 1e-8);
    }

    #[test]
    fn samples_are_equally_spaced_and_include_the_start() {
        let r = run(|_, y| vec![y[0]], 0.0, &[1.0], 1.0, 4).unwrap();
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], vec![1.0]);
        for (k, state) in r.iter().enumerate() {
            let want = (k as f64 / 4.0).exp();
            assert!((state[0] - want).abs() < 1e-8 * want, "{k}: {}", state[0]);
        }
    }

    #[test]
    fn a_zero_length_interval_returns_the_start() {
        let r = run(|_, y| vec![y[0]], 2.0, &[5.0], 2.0, 3).unwrap();
        assert_eq!(r, vec![vec![5.0]; 4]);
    }

    #[test]
    fn the_tolerance_does_not_depend_on_the_size_of_the_numbers() {
        // The same problem in nanometres and in kilometres: equal relative accuracy.
        for scale in [1e-9, 1.0, 1e9] {
            let r = run(|_, y| vec![-y[0]], 0.0, &[scale], 4.0, 1).unwrap();
            let want = scale * (-4f64).exp();
            assert!(
                (r[1][0] - want).abs() < 1e-8 * want,
                "scale {scale}: {}",
                r[1][0]
            );
        }
        // A start from zero is scaled by how far the derivative carries it.
        let r = run(|_, _| vec![1e-6], 0.0, &[0.0], 10.0, 1).unwrap();
        assert!((r[1][0] - 1e-5).abs() < 1e-13);
    }

    #[test]
    fn a_solution_that_blows_up_is_an_error() {
        // y' = y^2, y(0) = 1 reaches infinity at t = 1.
        let e = run(|_, y| vec![y[0] * y[0]], 0.0, &[1.0], 2.0, 1).unwrap_err();
        assert!(e.contains("too small") || e.contains("steps"), "{e}");
    }

    #[test]
    fn a_stiff_problem_runs_out_of_steps_instead_of_hanging() {
        let e = run(|_, y| vec![-1e7 * y[0]], 0.0, &[1.0], 1.0, 1).unwrap_err();
        assert!(e.contains("stiff"), "{e}");
    }

    #[test]
    fn a_domain_error_at_a_stage_only_shortens_the_step() {
        // The right-hand side is undefined for y < 0, but the solution y = t^2
        // (y' = 2 sqrt(y)) stays positive.
        let mut f = |_: f64, y: &[f64]| -> Result<Vec<f64>, Located> {
            if y[0] < 0.0 {
                Err(invalid("domain"))
            } else {
                Ok(vec![2.0 * y[0].sqrt()])
            }
        };
        let r = solve(&mut f, 0.0, &[1e-12], 3.0, 1).ok().unwrap();
        assert!((r[1][0] - 9.0).abs() < 1e-3, "{}", r[1][0]);
    }

    #[test]
    fn a_failure_at_the_start_or_a_stop_is_passed_on() {
        let mut bad = |_: f64, _: &[f64]| -> Result<Vec<f64>, Located> { Err(invalid("boom")) };
        assert!(solve(&mut bad, 0.0, &[1.0], 1.0, 1).is_err());
        let mut calls = 0;
        let mut stop = |_: f64, y: &[f64]| -> Result<Vec<f64>, Located> {
            calls += 1;
            if calls > 10 {
                Err(Located::here(crate::eval::EvalError::Cancelled))
            } else {
                Ok(vec![y[0]])
            }
        };
        let e = solve(&mut stop, 0.0, &[1.0], 5.0, 1).err().unwrap();
        assert!(e.error.is_stop());
    }
}
