//! BFGS minimisation with a finite-difference gradient and a backtracking line
//! search, and the augmented-Lagrangian penalty used for constraints.
//!
//! This finds a **local** minimum near the starting point. A trial point where the
//! function cannot be evaluated is a failed line-search step (the step is halved);
//! running out of budget or being cancelled stops everything.

use super::{Kind, Term};
use crate::eval::node::Located;

const MAX_ITERATIONS: usize = 200;
const MAX_HALVINGS: usize = 40;
/// Relative step of the central-difference gradient.
const STEP: f64 = 6e-6;
/// Stop when the largest gradient component is this small, relative to the size of
/// the function and of the point.
const GRADIENT_TOL: f64 = 1e-9;

pub struct Minimum {
    pub x: Vec<f64>,
    pub converged: bool,
}

type Objective<'a> = dyn FnMut(&[f64]) -> Result<f64, Located> + 'a;

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn gradient(f: &mut Objective, x: &[f64], reference: &[f64]) -> Result<Vec<f64>, Located> {
    let mut g = Vec::with_capacity(x.len());
    for j in 0..x.len() {
        let h = STEP * x[j].abs().max(reference[j]);
        let (mut up, mut down) = (x.to_vec(), x.to_vec());
        up[j] += h;
        down[j] -= h;
        g.push((f(&up)? - f(&down)?) / (2.0 * h));
    }
    Ok(g)
}

/// The penalty terms of the augmented Lagrangian for multipliers `lambda` and
/// weight `mu`: equations `lambda h + mu h^2 / 2`, inequalities
/// `(max(0, lambda + mu g)^2 - lambda^2) / (2 mu)`.
pub fn penalty(terms: &[Term], lambda: &[f64], mu: f64) -> f64 {
    terms
        .iter()
        .zip(lambda)
        .map(|(t, l)| match t.kind {
            Kind::Eq => l * t.value + 0.5 * mu * t.value * t.value,
            Kind::Le => ((l + mu * t.value).max(0.0).powi(2) - l * l) / (2.0 * mu),
        })
        .sum()
}

pub fn minimise(f: &mut Objective, x0: &[f64]) -> Result<Minimum, Located> {
    let n = x0.len();
    let reference: Vec<f64> = x0
        .iter()
        .map(|v| if *v != 0.0 { v.abs() } else { 1.0 })
        .collect();
    let mut x = x0.to_vec();
    let mut fx = f(&x)?;
    let mut g = gradient(f, &x, &reference)?;
    // The inverse Hessian estimate, row-major.
    let identity = |n: usize| {
        let mut h = vec![0.0; n * n];
        for i in 0..n {
            h[i * n + i] = 1.0;
        }
        h
    };
    let mut h = identity(n);
    for iteration in 0..MAX_ITERATIONS {
        let x_size = x.iter().fold(1.0f64, |m, v| m.max(v.abs()));
        let small = GRADIENT_TOL * fx.abs().max(1.0) / x_size;
        let g_size = g.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        if g_size <= small {
            return Ok(Minimum { x, converged: true });
        }
        let mut d: Vec<f64> = (0..n)
            .map(|i| -(0..n).map(|j| h[i * n + j] * g[j]).sum::<f64>())
            .collect();
        if dot(&g, &d) >= 0.0 {
            h = identity(n);
            d = g.iter().map(|v| -v).collect();
        }
        // The first step has no curvature information: keep it modest.
        let mut t = if iteration == 0 {
            (1.0 / g_size).min(1.0)
        } else {
            1.0
        };
        let slope = dot(&g, &d);
        let mut accepted = None;
        for _ in 0..MAX_HALVINGS {
            let trial: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + t * b).collect();
            match f(&trial) {
                Ok(ft) if ft <= fx + 1e-4 * t * slope => {
                    accepted = Some((trial, ft));
                    break;
                }
                Ok(_) => t *= 0.5,
                Err(l) if l.error.is_stop() => return Err(l),
                Err(_) => t *= 0.5,
            }
        }
        let Some((xn, fn_)) = accepted else {
            // No decrease along the search direction: as converged as it gets.
            return Ok(Minimum {
                x,
                converged: g_size <= 1e-5 * fx.abs().max(1.0) / x_size,
            });
        };
        let gn = gradient(f, &xn, &reference)?;
        let s: Vec<f64> = xn.iter().zip(&x).map(|(a, b)| a - b).collect();
        let y: Vec<f64> = gn.iter().zip(&g).map(|(a, b)| a - b).collect();
        let sy = dot(&s, &y);
        if sy > 1e-12 * dot(&s, &s).sqrt() * dot(&y, &y).sqrt() {
            // H <- (I - rho s y') H (I - rho y s') + rho s s'
            let rho = 1.0 / sy;
            let hy: Vec<f64> = (0..n)
                .map(|i| (0..n).map(|j| h[i * n + j] * y[j]).sum())
                .collect();
            let yhy = dot(&y, &hy);
            for i in 0..n {
                for j in 0..n {
                    h[i * n + j] +=
                        (1.0 + rho * yhy) * rho * s[i] * s[j] - rho * (hy[i] * s[j] + s[i] * hy[j]);
                }
            }
        }
        let progress = fx - fn_;
        let moved = s.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        x = xn;
        fx = fn_;
        g = gn;
        // Near the limit of what the arithmetic can resolve the function stops
        // changing, so the strict gradient test above cannot be met: accept a
        // small gradient together with no measurable progress.
        let g_now = g.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let flat = progress.abs() <= 1e-15 * fx.abs().max(1.0) || moved <= 1e-12 * x_size;
        if flat && g_now <= 1e-6 * fx.abs().max(1.0) / x_size {
            return Ok(Minimum { x, converged: true });
        }
    }
    Ok(Minimum {
        x,
        converged: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn min(f: impl Fn(&[f64]) -> f64, x0: &[f64]) -> Minimum {
        minimise(&mut |x| Ok(f(x)), x0).ok().unwrap()
    }

    #[test]
    fn a_quadratic_bowl() {
        let m = min(
            |p| (p[0] - 3.0).powi(2) + 2.0 * (p[1] + 1.0).powi(2),
            &[0.0, 0.0],
        );
        assert!(m.converged);
        assert!(
            (m.x[0] - 3.0).abs() < 1e-6 && (m.x[1] + 1.0).abs() < 1e-6,
            "{:?}",
            m.x
        );
    }

    #[test]
    fn rosenbrocks_valley() {
        let rosenbrock = |p: &[f64]| 100.0 * (p[1] - p[0] * p[0]).powi(2) + (1.0 - p[0]).powi(2);
        let m = min(rosenbrock, &[-1.2, 1.0]);
        assert!(m.converged, "{:?}", m.x);
        assert!(
            (m.x[0] - 1.0).abs() < 1e-4 && (m.x[1] - 1.0).abs() < 1e-4,
            "{:?}",
            m.x
        );
    }

    #[test]
    fn one_variable_and_a_start_that_is_already_the_minimum() {
        let m = min(|p| (p[0] - 2.0).powi(4) + 1.0, &[5.0]);
        assert!((m.x[0] - 2.0).abs() < 1e-2, "{:?}", m.x);
        let m = min(|p| p[0] * p[0], &[0.0]);
        assert!(m.converged && m.x[0] == 0.0);
    }

    #[test]
    fn a_function_unbounded_below_does_not_converge() {
        let m = min(|p| p[0], &[0.0]);
        assert!(!m.converged);
    }

    #[test]
    fn a_trial_point_that_cannot_be_evaluated_only_shortens_the_step() {
        // ln has no value for x <= 0; the minimum of x - ln x is at 1.
        let mut f = |p: &[f64]| -> Result<f64, Located> {
            if p[0] <= 0.0 {
                Err(super::super::super::invalid("domain"))
            } else {
                Ok(p[0] - p[0].ln())
            }
        };
        let m = minimise(&mut f, &[4.0]).ok().unwrap();
        assert!((m.x[0] - 1.0).abs() < 1e-4, "{:?}", m.x);
    }

    #[test]
    fn the_penalty_is_zero_for_a_satisfied_inequality_and_grows_with_violation() {
        let ok = Term {
            value: -1.0,
            scale: 1.0,
            kind: Kind::Le,
        };
        let bad = Term {
            value: 1.0,
            scale: 1.0,
            kind: Kind::Le,
        };
        assert_eq!(penalty(&[ok], &[0.0], 10.0), 0.0);
        assert!(penalty(&[bad], &[0.0], 10.0) > 0.0);
        let eq = Term {
            value: 0.5,
            scale: 1.0,
            kind: Kind::Eq,
        };
        assert!((penalty(&[eq], &[0.0], 10.0) - 1.25).abs() < 1e-12);
    }

    #[test]
    fn work_is_bounded() {
        let mut calls = 0;
        let mut f = |p: &[f64]| -> Result<f64, Located> {
            calls += 1;
            Ok(p[0].sin() * 1e3 + p[0])
        };
        let _ = minimise(&mut f, &[1.0]);
        assert!(calls <= MAX_ITERATIONS * (MAX_HALVINGS + 3) + 3, "{calls}");
    }
}
