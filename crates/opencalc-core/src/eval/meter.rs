//! A limit on how long one region's calculation may run, and a way to stop it.
//!
//! Anything that repeats an evaluation (sums, products, integrals, derivatives,
//! limits) calls [`Meter::charge`] before each one. The count is deterministic:
//! the same worksheet always stops at the same point, whatever the machine. The
//! worker also raises a cancel flag when a newer edit arrives, so a long
//! calculation on a stale worksheet gives way at once.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use super::EvalError;

/// Most repeated evaluations one region may make.
pub const MAX_STEPS: u64 = 1_000_000;

pub struct Meter<'a> {
    steps: Cell<u64>,
    cancel: Option<&'a AtomicBool>,
}

impl<'a> Meter<'a> {
    pub fn new(cancel: Option<&'a AtomicBool>) -> Meter<'a> {
        Meter {
            steps: Cell::new(0),
            cancel,
        }
    }

    /// Count one repeated evaluation. Fails when the budget is spent or the
    /// calculation was cancelled.
    pub fn charge(&self) -> Result<(), EvalError> {
        let steps = self.steps.get() + 1;
        self.steps.set(steps);
        if self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EvalError::Cancelled);
        }
        if steps > MAX_STEPS {
            return Err(EvalError::Invalid(format!(
                "the calculation needs more than {MAX_STEPS} repeated evaluations"
            )));
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn steps(&self) -> u64 {
        self.steps.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_runs_out_at_a_fixed_count() {
        let m = Meter::new(None);
        for _ in 0..MAX_STEPS {
            m.charge().unwrap();
        }
        assert!(matches!(m.charge(), Err(EvalError::Invalid(_))));
        assert_eq!(m.steps(), MAX_STEPS + 1);
    }

    #[test]
    fn a_raised_flag_cancels_at_the_next_charge() {
        let flag = AtomicBool::new(false);
        let m = Meter::new(Some(&flag));
        assert!(m.charge().is_ok());
        flag.store(true, Ordering::Relaxed);
        assert_eq!(m.charge(), Err(EvalError::Cancelled));
    }
}
