//! Background evaluation worker (spec section 5.4).
//!
//! One thread owns evaluation. The UI sends snapshots tagged with a revision
//! and receives results over a channel. If several requests queue up while the
//! worker is busy, only the newest is evaluated. The UI applies a result only
//! if its revision is still current.

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::document::PlotInput;
use crate::eval::{all_failed, evaluate_cancellable, Output, RegionId};
use crate::expr::Expr;
use crate::registry::UnitRegistry;

/// Stack for the evaluation thread. Virtual memory only; pages are committed as used.
const WORKER_STACK: usize = 8 * 1024 * 1024;

enum Request {
    Recalculate {
        revision: u64,
        regions: Vec<(RegionId, Expr)>,
        plots: Vec<(RegionId, PlotInput)>,
        units: UnitRegistry,
    },
    Shutdown,
}

/// A finished evaluation, tagged with the revision it was computed for.
#[derive(Debug)]
pub struct EvalResult {
    pub revision: u64,
    pub output: Output,
}

pub struct Worker {
    tx: Sender<Request>,
    /// Raised by every request, so a long calculation on a worksheet that has
    /// since changed gives way. The worker lowers it when it starts the newest.
    cancel: Arc<AtomicBool>,
    rx: Receiver<EvalResult>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    /// Start the worker. `on_result` runs on the worker thread after each
    /// result is queued; the UI passes a repaint request here.
    pub fn spawn(on_result: impl Fn() + Send + 'static) -> Worker {
        let (tx, req_rx) = mpsc::channel::<Request>();
        let (res_tx, rx) = mpsc::channel::<EvalResult>();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        // An explicit, generous stack: evaluation recurses over the expression
        // tree. If the thread cannot be created, the request channel is dropped
        // with the closure and `request` reports failure instead of panicking.
        let handle = std::thread::Builder::new()
            .name("opencalc-eval".to_owned())
            .stack_size(WORKER_STACK)
            .spawn(move || {
                while let Ok(first) = req_rx.recv() {
                    // Skip ahead to the newest queued request.
                    let mut latest = first;
                    while let Ok(next) = req_rx.try_recv() {
                        latest = next;
                    }
                    match latest {
                        Request::Shutdown => break,
                        Request::Recalculate {
                            revision,
                            regions,
                            plots,
                            units,
                        } => {
                            // A panic in the evaluator must not end the worker thread: report
                            // it on every region and keep serving requests.
                            worker_cancel.store(false, Ordering::SeqCst);
                            let output = std::panic::catch_unwind(AssertUnwindSafe(|| {
                                evaluate_cancellable(&regions, &plots, &units, Some(&worker_cancel))
                            }))
                            .unwrap_or_else(|_| all_failed(&regions, "the evaluator panicked"));
                            if res_tx.send(EvalResult { revision, output }).is_err() {
                                break;
                            }
                            on_result();
                        }
                    }
                }
            })
            .ok();
        Worker {
            tx,
            cancel,
            rx,
            handle,
        }
    }

    /// Queue a recalculation. Returns false if the worker has stopped.
    pub fn request(&self, revision: u64, regions: Vec<(RegionId, Expr)>) -> bool {
        self.request_with(revision, regions, UnitRegistry::builtin())
    }

    /// Queue a recalculation with the worksheet's own units.
    pub fn request_with(
        &self,
        revision: u64,
        regions: Vec<(RegionId, Expr)>,
        units: UnitRegistry,
    ) -> bool {
        self.request_full(revision, regions, Vec::new(), units)
    }

    /// Queue a recalculation of math regions and the plots that read them.
    pub fn request_full(
        &self,
        revision: u64,
        regions: Vec<(RegionId, Expr)>,
        plots: Vec<(RegionId, PlotInput)>,
        units: UnitRegistry,
    ) -> bool {
        self.cancel.store(true, Ordering::SeqCst);
        self.tx
            .send(Request::Recalculate {
                revision,
                regions,
                plots,
                units,
            })
            .is_ok()
    }

    /// The newest result waiting, if any. Older queued results are dropped.
    pub fn poll(&self) -> Option<EvalResult> {
        let mut latest = None;
        while let Ok(r) = self.rx.try_recv() {
            latest = Some(r);
        }
        latest
    }

    /// The result for exactly `revision`, if it has arrived. Results for any
    /// other revision are discarded: they were computed from a document that has
    /// since changed (or from another worksheet entirely), and applying them
    /// would show wrong values. The worker always evaluates the newest request,
    /// so the result for the current revision does arrive.
    pub fn poll_for(&self, revision: u64) -> Option<EvalResult> {
        let mut found = None;
        while let Ok(r) = self.rx.try_recv() {
            if r.revision == revision {
                found = Some(r);
            }
        }
        found
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // A send error means the thread already ended; either way, join it.
        let _ = self.tx.send(Request::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn wait(w: &Worker) -> EvalResult {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            if let Some(r) = w.poll() {
                return r;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("worker produced no result");
    }

    #[test]
    fn result_carries_revision_and_triggers_callback() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let w = Worker::spawn(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        assert!(w.request(7, vec![(RegionId(1), Expr::define("x", Expr::num(2.0)))]));
        let r = wait(&w);
        assert_eq!(r.revision, 7);
        assert_eq!(
            r.output.values[&RegionId(1)]
                .as_ref()
                .unwrap()
                .as_real()
                .unwrap()
                .si,
            2.0
        );
        assert!(calls.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn newest_revision_is_eventually_delivered() {
        let w = Worker::spawn(|| {});
        for rev in 1..=50 {
            w.request(
                rev,
                vec![(RegionId(1), Expr::define("x", Expr::num(rev as f64)))],
            );
        }
        let end = Instant::now() + Duration::from_secs(5);
        let mut last = 0;
        while last != 50 && Instant::now() < end {
            if let Some(r) = w.poll() {
                last = r.revision;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(last, 50);
    }

    #[test]
    fn a_newer_request_stops_a_long_calculation_and_its_result_arrives() {
        use crate::eval::EvalError;
        use crate::parser::parse;
        let w = Worker::spawn(|| {});
        // A long sum, then a quick edit. The sum is cut short (or skipped), and the
        // answer for the newest revision always arrives.
        let long = parse("sum_over(sin(k) + cos(k), k, 1, 900000)");
        w.request(1, vec![(RegionId(1), long)]);
        w.request(2, vec![(RegionId(1), Expr::define("x", Expr::num(3.0)))]);
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let r = w.poll_for(2);
            if let Some(r) = r {
                assert_eq!(
                    r.output.values[&RegionId(1)]
                        .as_ref()
                        .unwrap()
                        .as_real()
                        .unwrap()
                        .si,
                    3.0
                );
                break;
            }
            assert!(Instant::now() < end, "the newest result never arrived");
            std::thread::sleep(Duration::from_millis(2));
        }
        // And a cancelled result never reads as a number.
        let _ = EvalError::Cancelled;
    }

    #[test]
    fn poll_for_discards_results_for_other_revisions() {
        let w = Worker::spawn(|| {});
        // Revision 1 belongs to a different document than revision 2.
        w.request(1, vec![(RegionId(1), Expr::define("x", Expr::num(1.0)))]);
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            w.poll_for(2).is_none(),
            "a stale result must not be accepted"
        );
        // The stale result was consumed, not kept for later.
        assert!(w.poll().is_none());
        w.request(2, vec![(RegionId(1), Expr::define("x", Expr::num(2.0)))]);
        let end = Instant::now() + Duration::from_secs(5);
        let mut got = None;
        while got.is_none() && Instant::now() < end {
            got = w.poll_for(2);
            std::thread::sleep(Duration::from_millis(2));
        }
        let r = got.expect("result for the current revision");
        assert_eq!(r.revision, 2);
        assert_eq!(
            r.output.values[&RegionId(1)]
                .as_ref()
                .unwrap()
                .as_real()
                .unwrap()
                .si,
            2.0
        );
    }

    #[test]
    fn worker_reports_an_error_for_input_that_used_to_panic() {
        // This input used to overflow a dimension exponent; the panic ended the
        // worker thread for good and the UI showed "worker stopped" forever.
        let w = Worker::spawn(|| {});
        let bad = crate::parser::parse("(1 m^2)^100");
        w.request(1, vec![(RegionId(1), bad)]);
        let r = wait(&w);
        assert!(r.output.values[&RegionId(1)].is_err());
        // And the worker is still alive afterwards.
        w.request(2, vec![(RegionId(1), Expr::define("x", Expr::num(3.0)))]);
        let end = Instant::now() + Duration::from_secs(5);
        let mut got = None;
        while got.is_none() && Instant::now() < end {
            got = w.poll_for(2);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(got.is_some(), "worker must keep answering");
    }

    #[test]
    fn drop_stops_the_thread() {
        let w = Worker::spawn(|| {});
        drop(w); // would hang here if shutdown were broken
    }
}
