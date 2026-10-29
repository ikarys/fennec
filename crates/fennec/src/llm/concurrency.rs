//! The model's concurrency gate: a process-wide counting semaphore that
//! bounds how many model rounds may run at once. One local GPU serves one
//! round at a time, so the default is one; `FENNEC_MODEL_CONCURRENCY` raises
//! it when the endpoint tolerates parallel requests. Tool calls are not
//! gated: a long command in one session never blocks another's model round.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// Bounds the model rounds in flight to a fixed capacity.
pub struct ModelGate {
    free: Mutex<usize>,
    cv: Condvar,
}

impl ModelGate {
    /// A gate of `capacity` slots; a capacity below one is clamped to one.
    pub fn new(capacity: usize) -> Self {
        ModelGate {
            free: Mutex::new(capacity.max(1)),
            cv: Condvar::new(),
        }
    }

    /// Takes a slot, waiting for a free one in 200 ms slices; returns `None`
    /// when `stop` is set so a stopped session does not queue behind a long
    /// round in another session.
    pub fn enter(&self, stop: &AtomicBool) -> Option<Guard<'_>> {
        let mut free = self.free.lock().unwrap();
        loop {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if *free > 0 {
                *free -= 1;
                return Some(Guard { gate: self });
            }
            free = self
                .cv
                .wait_timeout(free, Duration::from_millis(200))
                .unwrap()
                .0;
        }
    }
}

/// A held slot; returns it to the gate when dropped.
pub struct Guard<'a> {
    gate: &'a ModelGate,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let mut free = self.gate.free.lock().unwrap();
        *free += 1;
        self.gate.cv.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_capacity_two_gate_allows_two_and_blocks_the_third() {
        let gate = Arc::new(ModelGate::new(2));
        let _a = gate.enter(&AtomicBool::new(false)).unwrap();
        let _b = gate.enter(&AtomicBool::new(false)).unwrap();

        // The third caller cannot take a slot while two are held.
        let (tx, rx) = std::sync::mpsc::channel();
        let gate3 = gate.clone();
        let worker = std::thread::spawn(move || {
            let _c = gate3.enter(&AtomicBool::new(false)).unwrap();
            let _ = tx.send(());
        });
        assert!(rx.recv_timeout(Duration::from_millis(80)).is_err());

        // Free one slot; the third now passes.
        drop(_b);
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_ok());
        worker.join().unwrap();
    }

    #[test]
    fn a_zero_capacity_is_clamped_to_one() {
        let gate = Arc::new(ModelGate::new(0));
        let _a = gate.enter(&AtomicBool::new(false)).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        let gate2 = gate.clone();
        let worker = std::thread::spawn(move || {
            let _c = gate2.enter(&AtomicBool::new(false)).unwrap();
            let _ = tx.send(());
        });
        assert!(rx.recv_timeout(Duration::from_millis(80)).is_err());
        drop(_a);
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_ok());
        worker.join().unwrap();
    }

    #[test]
    fn a_stopped_waiter_returns_none_while_the_gate_is_full() {
        let gate = Arc::new(ModelGate::new(1));
        let _a = gate.enter(&AtomicBool::new(false)).unwrap();
        let stop = Arc::new(AtomicBool::new(true));

        let (tx, rx) = std::sync::mpsc::channel();
        let gate2 = gate.clone();
        let stop2 = stop.clone();
        let worker = std::thread::spawn(move || {
            let entered = gate2.enter(&stop2);
            let _ = tx.send(entered.is_none());
        });
        assert!(rx.recv_timeout(Duration::from_millis(300)).unwrap());
        worker.join().unwrap();
    }
}
