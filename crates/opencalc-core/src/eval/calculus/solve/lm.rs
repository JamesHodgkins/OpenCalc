//! Levenberg-Marquardt for a system of equations and inequalities.
//!
//! The residual vector holds, for each constraint, its signed difference (left side
//! minus right side) for an equation, and the amount by which it is violated for an
//! inequality (zero when it holds). The signed value matters: the absolute value
//! has a kink at the solution, which wrecks a finite-difference Jacobian once a
//! step is smaller than the distance to it. The method
//! minimises half the sum of their squares with a damped Gauss-Newton step: the
//! damping grows when a step does not reduce the cost and shrinks when it does,
//! so it behaves like Gauss-Newton near a solution and like a short gradient step
//! far from one. The Jacobian is by forward differences.
//!
//! A trial point where the constraints cannot be evaluated (a domain error) only
//! counts as a rejected step. Running out of budget or being cancelled always
//! stops the whole solve.

use super::{floors, satisfied, Kind, Term};
use crate::eval::node::Located;

/// Most iterations (each one Jacobian and a few trial points).
const MAX_ITERATIONS: usize = 200;
/// Most damping adjustments within one iteration.
const MAX_ATTEMPTS: usize = 30;
/// Relative step for the finite-difference Jacobian.
const STEP: f64 = 1.5e-8;

pub struct Outcome {
    pub x: Vec<f64>,
    pub terms: Vec<Term>,
    pub satisfied: bool,
}

type Residuals<'a> = dyn FnMut(&[f64]) -> Result<Vec<Term>, Located> + 'a;

/// The value that is driven to zero: signed for an equation, clamped for an inequality.
fn residual(t: &Term) -> f64 {
    match t.kind {
        Kind::Eq => t.value,
        Kind::Le => t.value.max(0.0),
    }
}

fn cost(terms: &[Term]) -> f64 {
    0.5 * terms.iter().map(|t| residual(t).powi(2)).sum::<f64>()
}

/// Solve `a x = b` (n by n, row-major) by elimination with partial pivoting.
pub fn solve_linear(mut a: Vec<f64>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot =
            (col..n).max_by(|&i, &j| a[i * n + col].abs().total_cmp(&a[j * n + col].abs()))?;
        if a[pivot * n + col].abs() < 1e-300 {
            return None;
        }
        if pivot != col {
            for k in 0..n {
                a.swap(pivot * n + k, col * n + k);
            }
            b.swap(pivot, col);
        }
        for r in col + 1..n {
            let f = a[r * n + col] / a[col * n + col];
            for k in col..n {
                let sub = f * a[col * n + k];
                a[r * n + k] -= sub;
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let tail: f64 = (i + 1..n).map(|k| a[i * n + k] * x[k]).sum();
        x[i] = (b[i] - tail) / a[i * n + i];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// Solve the constraints from `x0`, to relative tolerance `rel`.
pub fn solve(f: &mut Residuals, x0: &[f64], rel: f64) -> Result<Outcome, Located> {
    let n = x0.len();
    let mut x = x0.to_vec();
    let mut terms = f(&x)?;
    let floors = floors(&terms);
    let mut current = cost(&terms);
    let mut lambda = 1e-3;
    let reference: Vec<f64> = x0
        .iter()
        .map(|v| if *v != 0.0 { v.abs() } else { 1.0 })
        .collect();
    for _ in 0..MAX_ITERATIONS {
        if satisfied(&terms, &floors, rel) {
            break;
        }
        let r: Vec<f64> = terms.iter().map(residual).collect();
        let m = r.len();
        let mut jac = vec![0.0; m * n];
        for j in 0..n {
            let h = STEP * x[j].abs().max(reference[j]);
            let mut moved = x.clone();
            moved[j] += h;
            let t = f(&moved)?;
            for (i, ti) in t.iter().enumerate() {
                jac[i * n + j] = (residual(ti) - r[i]) / h;
            }
        }
        let mut g = vec![0.0; n];
        let mut a = vec![0.0; n * n];
        for i in 0..m {
            for j in 0..n {
                g[j] += jac[i * n + j] * r[i];
                for k in 0..n {
                    a[j * n + k] += jac[i * n + j] * jac[i * n + k];
                }
            }
        }
        if g.iter().all(|v| *v == 0.0) {
            break;
        }
        let mut improved = false;
        for _ in 0..MAX_ATTEMPTS {
            let mut damped = a.clone();
            for j in 0..n {
                damped[j * n + j] += lambda * a[j * n + j].max(1e-12);
            }
            let rhs: Vec<f64> = g.iter().map(|v| -v).collect();
            let Some(step) = solve_linear(damped, rhs) else {
                lambda *= 10.0;
                continue;
            };
            let trial: Vec<f64> = x.iter().zip(&step).map(|(a, b)| a + b).collect();
            match f(&trial) {
                Ok(t) if cost(&t) < current => {
                    current = cost(&t);
                    terms = t;
                    x = trial;
                    lambda = (lambda / 3.0).max(1e-12);
                    improved = true;
                    break;
                }
                Ok(_) => lambda *= 4.0,
                Err(l) if l.error.is_stop() => return Err(l),
                Err(_) => lambda *= 4.0,
            }
            if lambda > 1e12 {
                break;
            }
        }
        if !improved {
            break;
        }
    }
    let ok = satisfied(&terms, &floors, rel);
    Ok(Outcome {
        x,
        terms,
        satisfied: ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(value: f64, scale: f64) -> Term {
        Term {
            value,
            scale,
            kind: Kind::Eq,
        }
    }

    fn run(f: impl Fn(&[f64]) -> Vec<Term>, x0: &[f64]) -> Outcome {
        solve(&mut |x| Ok(f(x)), x0, 1e-9).ok().unwrap()
    }

    #[test]
    fn linear_systems_by_elimination() {
        // 2x + y = 5, x - y = 1  ->  x = 2, y = 1
        let x = solve_linear(vec![2.0, 1.0, 1.0, -1.0], vec![5.0, 1.0]).unwrap();
        assert!((x[0] - 2.0).abs() < 1e-14 && (x[1] - 1.0).abs() < 1e-14);
        assert!(
            solve_linear(vec![1.0, 2.0, 2.0, 4.0], vec![1.0, 2.0]).is_none(),
            "singular"
        );
        assert!(solve_linear(vec![0.0], vec![1.0]).is_none());
    }

    #[test]
    fn a_circle_and_a_line() {
        // x^2 + y^2 = 25 and x - y = 1: (4, 3) and (-3, -4).
        let sys = |p: &[f64]| {
            vec![
                eq(p[0] * p[0] + p[1] * p[1] - 25.0, 25.0),
                eq(p[0] - p[1] - 1.0, 1.0),
            ]
        };
        let o = run(sys, &[1.0, 1.0]);
        assert!(o.satisfied);
        assert!(
            (o.x[0] - 4.0).abs() < 1e-7 && (o.x[1] - 3.0).abs() < 1e-7,
            "{:?}",
            o.x
        );
        let o = run(sys, &[-1.0, -1.0]);
        assert!(o.satisfied);
        assert!(
            (o.x[0] + 3.0).abs() < 1e-7 && (o.x[1] + 4.0).abs() < 1e-7,
            "{:?}",
            o.x
        );
    }

    #[test]
    fn a_three_by_three_nonlinear_system() {
        // x + y + z = 6, x*y*z = 6, x^2 + y^2 + z^2 = 14: a permutation of 1, 2, 3.
        let sys = |p: &[f64]| {
            vec![
                eq(p[0] + p[1] + p[2] - 6.0, 6.0),
                eq(p[0] * p[1] * p[2] - 6.0, 6.0),
                eq(p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - 14.0, 14.0),
            ]
        };
        let o = run(sys, &[0.5, 2.5, 3.5]);
        assert!(o.satisfied, "{:?}", o.x);
        let mut got = o.x.clone();
        got.sort_by(f64::total_cmp);
        for (g, want) in got.iter().zip([1.0, 2.0, 3.0]) {
            assert!((g - want).abs() < 1e-6, "{got:?}");
        }
    }

    #[test]
    fn inequalities_count_only_when_violated() {
        // x >= 2 written as 2 - x <= 0, starting at x = 0.
        let sys = |p: &[f64]| {
            vec![Term {
                value: 2.0 - p[0],
                scale: 2.0f64.max(p[0].abs()),
                kind: Kind::Le,
            }]
        };
        let o = run(sys, &[0.0]);
        assert!(o.satisfied && o.x[0] >= 2.0 - 1e-6, "{:?}", o.x);
        // Already satisfied: untouched.
        let o = run(sys, &[5.0]);
        assert!(o.satisfied && o.x[0] == 5.0);
    }

    #[test]
    fn a_system_with_no_solution_is_not_satisfied() {
        let sys = |p: &[f64]| vec![eq(p[0] * p[0] + 1.0, 1.0)];
        let o = run(sys, &[3.0]);
        assert!(!o.satisfied);
        assert!(o.terms[0].violation() > 0.5, "the best attempt is reported");
    }

    #[test]
    fn an_underdetermined_system_finds_some_solution() {
        // One equation, two unknowns: x + y = 3.
        let o = run(|p| vec![eq(p[0] + p[1] - 3.0, 3.0)], &[0.0, 0.0]);
        assert!(o.satisfied && (o.x[0] + o.x[1] - 3.0).abs() < 1e-8);
    }

    #[test]
    fn a_trial_point_that_cannot_be_evaluated_is_only_a_rejected_step() {
        // sqrt(x) = 2 evaluated as an error for x < 0.
        let mut f = |p: &[f64]| -> Result<Vec<Term>, Located> {
            if p[0] < 0.0 {
                Err(super::super::invalid("domain"))
            } else {
                Ok(vec![eq(p[0].sqrt() - 2.0, 2.0)])
            }
        };
        let o = solve(&mut f, &[0.25], 1e-9).ok().unwrap();
        assert!(o.satisfied && (o.x[0] - 4.0).abs() < 1e-6, "{:?}", o.x);
    }

    #[test]
    fn an_error_at_the_start_or_a_stop_is_passed_on() {
        let mut bad =
            |_: &[f64]| -> Result<Vec<Term>, Located> { Err(super::super::invalid("boom")) };
        assert!(solve(&mut bad, &[1.0], 1e-9).is_err());
        let mut calls = 0;
        let mut stop = |p: &[f64]| -> Result<Vec<Term>, Located> {
            calls += 1;
            if calls > 3 {
                Err(Located::here(crate::eval::EvalError::Cancelled))
            } else {
                Ok(vec![eq(p[0] * p[0] - 2.0, 2.0)])
            }
        };
        let e = solve(&mut stop, &[1.0], 1e-9).err().unwrap();
        assert!(e.error.is_stop());
    }

    #[test]
    fn work_is_bounded() {
        let mut calls = 0;
        let mut f = |p: &[f64]| -> Result<Vec<Term>, Located> {
            calls += 1;
            // Never solvable, never improving much.
            Ok(vec![eq((p[0] * 1e-3).sin() + 2.0, 1.0)])
        };
        let _ = solve(&mut f, &[1.0], 1e-9);
        assert!(calls <= MAX_ITERATIONS * (MAX_ATTEMPTS + 2) + 2, "{calls}");
    }
}
