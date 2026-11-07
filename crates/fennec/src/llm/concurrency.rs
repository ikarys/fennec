//! The model's concurrency gate, with session affinity: it bounds how many
//! model rounds run at once (one local GPU serves one round at a time, so
//! the default capacity is one; `FENNEC_MODEL_CONCURRENCY` raises it) and,
//! at capacity one, keeps serving the session it just served.
//!
//! Why affinity: a local server reuses its prompt cache only for the exact
//! prefix of its last request, so two sessions interleaving round by round
//! evict each other and every call re-prefills its whole prompt (measured:
//! 8% cache hits for two concurrent sessions, 311k tokens re-read for 14
//! calls). A session's rounds are separated by its tool calls, mostly a few
//! seconds; the gate waits that long for the holder to come back before it
//! lets another session in (`FENNEC_MODEL_AFFINITY_SECS`, default 10). The
//! holder's turn ending releases the gate at once, and a holder is never
//! kept past `MAX_HOLD` so a session with many short rounds cannot starve
//! the others. Tool calls are not gated.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// The longest a session keeps the gate to itself across its rounds; past
/// it a waiting session gets in and becomes the holder.
// ponytail: one fixed bound; the holder's measured prefill time could set it.
const MAX_HOLD: Duration = Duration::from_secs(120);

/// Bounds the model rounds in flight and prefers the last session served.
pub struct ModelGate {
    state: Mutex<State>,
    cv: Condvar,
    capacity: usize,
    grace: Duration,
}

struct State {
    free: usize,
    /// The session whose prompt the server's cache holds: the last one
    /// served, until it releases the gate or another session takes it.
    holder: Option<Holder>,
}

struct Holder {
    session: String,
    since: Instant,
    /// Set between the holder's rounds; `None` while one runs.
    idle_since: Option<Instant>,
}

impl ModelGate {
    /// A gate of `capacity` slots (clamped to one at least) that waits
    /// `grace` for its holder to come back between rounds.
    pub fn new(capacity: usize, grace: Duration) -> Self {
        ModelGate {
            state: Mutex::new(State {
                free: capacity.max(1),
                holder: None,
            }),
            cv: Condvar::new(),
            capacity: capacity.max(1),
            grace,
        }
    }

    /// Takes a slot for one of `session`'s rounds, waiting in 200 ms slices
    /// while the gate is full or held for another session; returns `None`
    /// when `stop` is set so a stopped session does not queue behind a long
    /// round in another session.
    pub fn enter(&self, session: &str, stop: &AtomicBool) -> Option<Guard<'_>> {
        let mut state = crate::locks::lock(&self.state);
        loop {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if state.free > 0 && self.admits(&state, session) {
                state.free -= 1;
                match &mut state.holder {
                    // A one-off call (a session's name, a commit subject) never
                    // releases: as the holder it would make the next session wait the grace.
                    _ if session.is_empty() => {}
                    Some(holder) if holder.session == session => holder.idle_since = None,
                    _ => {
                        state.holder = Some(Holder {
                            session: session.to_string(),
                            since: Instant::now(),
                            idle_since: None,
                        })
                    }
                }
                return Some(Guard { gate: self });
            }
            state = self
                .cv
                .wait_timeout(state, Duration::from_millis(200))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// Whether `session` may take a free slot now: always above capacity
    /// one; otherwise when it is the holder, when there is none, or when the
    /// holder has been away longer than the grace or held for too long.
    fn admits(&self, state: &State, session: &str) -> bool {
        if self.capacity > 1 {
            return true;
        }
        match &state.holder {
            None => true,
            Some(holder) if holder.session == session => true,
            Some(holder) => match holder.idle_since {
                None => false,
                Some(idle) => idle.elapsed() >= self.grace || holder.since.elapsed() >= MAX_HOLD,
            },
        }
    }

    /// `session`'s turn ended: the gate stops waiting for it.
    pub fn release(&self, session: &str) {
        let mut state = crate::locks::lock(&self.state);
        if state.holder.as_ref().is_some_and(|h| h.session == session) {
            state.holder = None;
            self.cv.notify_all();
        }
    }
}

/// A held slot; returns it to the gate when dropped, the holder kept.
pub struct Guard<'a> {
    gate: &'a ModelGate,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let mut state = crate::locks::lock(&self.gate.state);
        state.free += 1;
        if let Some(holder) = &mut state.holder {
            holder.idle_since = Some(Instant::now());
        }
        self.gate.cv.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::mpsc::{Receiver, channel};

    fn no_stop() -> AtomicBool {
        AtomicBool::new(false)
    }

    /// Tries to enter for `session` on another thread; the receiver tells
    /// when it got in.
    fn try_enter(gate: &Arc<ModelGate>, session: &str) -> Receiver<()> {
        let (tx, rx) = channel();
        let gate = gate.clone();
        let session = session.to_string();
        std::thread::spawn(move || {
            let _slot = gate.enter(&session, &no_stop()).unwrap();
            let _ = tx.send(());
        });
        rx
    }

    fn got_in(rx: &Receiver<()>, within_ms: u64) -> bool {
        rx.recv_timeout(Duration::from_millis(within_ms)).is_ok()
    }

    #[test]
    fn a_capacity_two_gate_allows_two_and_blocks_the_third() {
        let gate = Arc::new(ModelGate::new(2, Duration::ZERO));
        let _a = gate.enter("a", &no_stop()).unwrap();
        let _b = gate.enter("b", &no_stop()).unwrap();

        // The third caller cannot take a slot while two are held.
        let rx = try_enter(&gate, "c");
        assert!(!got_in(&rx, 80));

        // Free one slot; the third now passes.
        drop(_b);
        assert!(got_in(&rx, 300));
    }

    #[test]
    fn a_zero_capacity_is_clamped_to_one() {
        let gate = Arc::new(ModelGate::new(0, Duration::ZERO));
        let _a = gate.enter("a", &no_stop()).unwrap();
        let rx = try_enter(&gate, "b");
        assert!(!got_in(&rx, 80));
        drop(_a);
        assert!(got_in(&rx, 300));
    }

    #[test]
    fn a_stopped_waiter_returns_none_while_the_gate_is_full() {
        let gate = Arc::new(ModelGate::new(1, Duration::ZERO));
        let _a = gate.enter("a", &no_stop()).unwrap();
        let stop = Arc::new(AtomicBool::new(true));

        let (tx, rx) = channel();
        let gate2 = gate.clone();
        let stop2 = stop.clone();
        std::thread::spawn(move || {
            let entered = gate2.enter("b", &stop2);
            let _ = tx.send(entered.is_none());
        });
        assert!(rx.recv_timeout(Duration::from_millis(300)).unwrap());
    }

    #[test]
    fn the_holder_keeps_the_gate_between_its_rounds_and_another_waits_the_grace() {
        let gate = Arc::new(ModelGate::new(1, Duration::from_millis(400)));
        let a = gate.enter("a", &no_stop()).unwrap();
        drop(a);

        // Between a's rounds the slot is free, but b must wait the grace…
        let rx = try_enter(&gate, "b");
        assert!(!got_in(&rx, 100));
        // …while a goes straight back in.
        let a = gate.enter("a", &no_stop()).unwrap();
        drop(a);

        // a stays away: b gets in once the grace has run out.
        assert!(got_in(&rx, 1000));
    }

    #[test]
    fn the_holder_ending_its_turn_lets_another_session_in_at_once() {
        let gate = Arc::new(ModelGate::new(1, Duration::from_secs(60)));
        drop(gate.enter("a", &no_stop()).unwrap());
        let rx = try_enter(&gate, "b");
        assert!(!got_in(&rx, 100));

        gate.release("a");
        assert!(got_in(&rx, 300));
    }

    #[test]
    fn releasing_another_session_changes_nothing() {
        let gate = Arc::new(ModelGate::new(1, Duration::from_secs(60)));
        drop(gate.enter("a", &no_stop()).unwrap());
        gate.release("b");
        let rx = try_enter(&gate, "b");
        assert!(!got_in(&rx, 100));
    }

    #[test]
    fn a_newcomer_becomes_the_holder_once_in() {
        let gate = Arc::new(ModelGate::new(1, Duration::from_millis(100)));
        drop(gate.enter("a", &no_stop()).unwrap());
        std::thread::sleep(Duration::from_millis(150));
        drop(gate.enter("b", &no_stop()).unwrap());

        // Now a is the one waiting.
        let rx = try_enter(&gate, "a");
        assert!(!got_in(&rx, 50));
        assert!(got_in(&rx, 500));
    }

    #[test]
    fn a_one_off_call_does_not_hold_the_gate() {
        let gate = Arc::new(ModelGate::new(1, Duration::from_secs(60)));
        drop(gate.enter("", &no_stop()).unwrap());
        // No grace to wait: the one-off call never comes back.
        assert!(got_in(&try_enter(&gate, "a"), 500));
    }

    #[test]
    fn above_capacity_one_there_is_no_affinity() {
        let gate = Arc::new(ModelGate::new(2, Duration::from_secs(60)));
        drop(gate.enter("a", &no_stop()).unwrap());
        let rx = try_enter(&gate, "b");
        assert!(got_in(&rx, 300));
    }
}
