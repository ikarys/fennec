//! Streamed chunks coalesced before they reach the log. A model sends a
//! token or two per chunk; one event each would cost a database row, a
//! stream message and a UI update per token. Coalescing per kind and per
//! `FLUSH_EVERY` keeps the stream live to the eye at a tenth of the events.

use crate::domain::event::Event;
use std::time::{Duration, Instant};

const FLUSH_EVERY: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, PartialEq)]
pub enum DeltaKind {
    Text,
    Thinking,
}

pub struct DeltaBuffer {
    message_id: String,
    pending: Option<(DeltaKind, String)>,
    since: Instant,
}

impl DeltaBuffer {
    pub fn new(message_id: &str) -> DeltaBuffer {
        DeltaBuffer {
            message_id: message_id.into(),
            pending: None,
            since: Instant::now(),
        }
    }

    /// Adds a chunk; returns the events to log now: the pending chunks when
    /// the kind changes, everything when `FLUSH_EVERY` has passed.
    pub fn push(&mut self, kind: DeltaKind, chunk: &str, now: Instant) -> Vec<Event> {
        let mut out = Vec::new();
        if self.pending.as_ref().is_some_and(|(k, _)| *k != kind) {
            out.extend(self.flush(now));
        }
        match &mut self.pending {
            Some((_, text)) => text.push_str(chunk),
            None => {
                self.pending = Some((kind, chunk.to_string()));
            }
        }
        if now.duration_since(self.since) >= FLUSH_EVERY {
            out.extend(self.flush(now));
        }
        out
    }

    /// The pending chunks as one event, if any.
    pub fn flush(&mut self, now: Instant) -> Option<Event> {
        self.since = now;
        let (kind, content) = self.pending.take()?;
        let message_id = self.message_id.clone();
        Some(match kind {
            DeltaKind::Text => Event::MessageDelta {
                message_id,
                content,
            },
            DeltaKind::Thinking => Event::MessageThinking {
                message_id,
                content,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(e: &Event) -> (&str, &str) {
        match e {
            Event::MessageDelta { content, .. } => ("text", content),
            Event::MessageThinking { content, .. } => ("thinking", content),
            _ => panic!("not a delta"),
        }
    }

    #[test]
    fn chunks_coalesce_until_the_kind_changes_or_time_passes() {
        let t0 = Instant::now();
        let mut buf = DeltaBuffer::new("m");
        assert!(buf.push(DeltaKind::Thinking, "a", t0).is_empty());
        assert!(buf.push(DeltaKind::Thinking, "b", t0).is_empty());
        let out = buf.push(DeltaKind::Text, "c", t0);
        assert_eq!(
            out.iter().map(text).collect::<Vec<_>>(),
            [("thinking", "ab")]
        );
        assert!(buf.push(DeltaKind::Text, "d", t0).is_empty());
        let out = buf.push(DeltaKind::Text, "e", t0 + FLUSH_EVERY);
        assert_eq!(out.iter().map(text).collect::<Vec<_>>(), [("text", "cde")]);
        assert!(buf.flush(t0).is_none());
    }
}
