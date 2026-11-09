//! Wakes event streams when a turn appends to a log, so an open tab costs
//! nothing while nothing happens (instead of polling the store).

use crate::agent::ports::EventLog;
use crate::domain::event::Event;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// A counter bumped on every append. One for all sessions: a stream woken
/// for another session's event just finds nothing new, which is cheap.
#[derive(Default)]
pub struct Notifier {
    generation: Mutex<u64>,
    changed: Condvar,
}

impl Notifier {
    pub fn generation(&self) -> u64 {
        *crate::locks::lock(&self.generation)
    }

    pub fn notify(&self) {
        *crate::locks::lock(&self.generation) += 1;
        self.changed.notify_all();
    }

    /// Blocks until the generation moves past `seen`, or `timeout`.
    pub fn wait_past(&self, seen: u64, timeout: Duration) {
        let guard = crate::locks::lock(&self.generation);
        let _ = self
            .changed
            .wait_timeout_while(guard, timeout, |g| *g == seen)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
}

/// An `EventLog` that notifies after each append.
pub struct NotifyingLog<'a> {
    pub log: &'a dyn EventLog,
    pub notifier: &'a Notifier,
}

impl EventLog for NotifyingLog<'_> {
    fn append(&self, session_id: &str, event: &Event) -> Result<u64, String> {
        let seq = self.log.append(session_id, event)?;
        self.notifier.notify();
        Ok(seq)
    }

    fn events(&self, session_id: &str) -> Result<Vec<Event>, String> {
        self.log.events(session_id)
    }

    fn window(&self, session_id: &str) -> Result<Vec<Event>, String> {
        self.log.window(session_id)
    }

    fn events_of(&self, session_id: &str, kinds: &[&str]) -> Result<Vec<Event>, String> {
        self.log.events_of(session_id, kinds)
    }
}
