//! Streamed chunks coalesced before they reach the log. A model sends a
//! token or two per chunk; one event each would cost a database row, a
//! stream message and a UI update per token. Coalescing per kind and per
//! `FLUSH_EVERY` keeps the stream live to the eye at a tenth of the events.
//!
//! The live speed is estimated from characters, not chunks: a server with
//! speculative decoding (ninfer) sends several tokens per chunk. The tokens
//! per character are learnt from the session's measured calls, since one
//! message's ratio swings from 0.24 to 0.67 (tool calls carry template
//! tokens no text shows). Tool-call arguments stream no delta, so the
//! figure is indicative.

use crate::domain::event::Event;
use std::time::{Duration, Instant};

const FLUSH_EVERY: Duration = Duration::from_millis(100);

/// Report a speed only after this span; a shorter span gives a jumpy figure.
const SPEED_AFTER: Duration = Duration::from_millis(500);

/// Before any measured call (Qwen-like tokenizers on code and prose).
const DEFAULT_TOKENS_PER_CHAR: f64 = 0.28;

/// Tokens per character over the session's measured calls; a short message
/// says too little to count.
pub fn tokens_per_char(events: &[Event]) -> f64 {
    let (mut tokens, mut chars) = (0u64, 0u64);
    let mut message_chars = std::collections::HashMap::<&str, u64>::new();
    for event in events {
        match event {
            Event::MessageDelta {
                message_id,
                content,
                ..
            }
            | Event::MessageThinking {
                message_id,
                content,
                ..
            } => *message_chars.entry(message_id).or_default() += content.len() as u64,
            // The arguments are generated tokens too: without them a message
            // that is mostly a write_file gives far too many tokens per char.
            Event::ToolCall {
                message_id,
                tool_call,
            } => {
                *message_chars.entry(message_id).or_default() +=
                    tool_call.arguments.to_string().len() as u64
            }
            Event::MessageDone {
                message_id,
                stats: Some(stats),
                partial: false,
            } => {
                let streamed = message_chars.remove(message_id.as_str()).unwrap_or(0);
                if streamed > 200 && stats.generation_tokens > 0 {
                    tokens += stats.generation_tokens;
                    chars += streamed;
                }
            }
            _ => {}
        }
    }
    if chars == 0 {
        DEFAULT_TOKENS_PER_CHAR
    } else {
        tokens as f64 / chars as f64
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DeltaKind {
    Text,
    Thinking,
}

pub struct DeltaBuffer {
    message_id: String,
    pending: Option<(DeltaKind, String)>,
    since: Instant,
    tokens_per_char: f64,
    /// Arrival of the first chunk, and the characters streamed after it.
    first: Option<Instant>,
    chars: u64,
}

impl DeltaBuffer {
    pub fn new(message_id: &str, tokens_per_char: f64) -> DeltaBuffer {
        DeltaBuffer {
            message_id: message_id.into(),
            pending: None,
            since: Instant::now(),
            tokens_per_char,
            first: None,
            chars: 0,
        }
    }

    /// Adds a chunk; returns the events to log now: the pending chunks when
    /// the kind changes, everything when `FLUSH_EVERY` has passed.
    pub fn push(&mut self, kind: DeltaKind, chunk: &str, now: Instant) -> Vec<Event> {
        // Counted before a flush so the estimate includes this chunk. The
        // first chunk's characters came with the prefill, not after it.
        if self.first.is_none() {
            self.first = Some(now);
        } else {
            self.chars += chunk.len() as u64;
        }

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
        let tokens_per_second = self.speed(now);
        Some(match kind {
            DeltaKind::Text => Event::MessageDelta {
                message_id,
                content,
                tokens_per_second,
            },
            DeltaKind::Thinking => Event::MessageThinking {
                message_id,
                content,
                tokens_per_second,
            },
        })
    }

    /// The characters after the first chunk, in tokens, over the time since
    /// it; once the span is long enough to be stable.
    fn speed(&self, now: Instant) -> Option<f64> {
        let elapsed = now.duration_since(self.first?);
        if elapsed < SPEED_AFTER || self.chars == 0 {
            return None;
        }
        Some(self.chars as f64 * self.tokens_per_char / elapsed.as_secs_f64())
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
        let mut buf = DeltaBuffer::new("m", 0.5);
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

    #[test]
    fn speed_counts_chunks_since_the_first_one() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let speed = |out: Vec<Event>| match out.as_slice() {
            [
                Event::MessageDelta {
                    tokens_per_second, ..
                },
            ] => *tokens_per_second,
            _ => panic!("expected one delta"),
        };
        let mut buf = DeltaBuffer::new("m", 0.5);
        assert!(buf.push(DeltaKind::Text, "a", t0).is_empty());
        // Flushed before SPEED_AFTER: no figure yet.
        assert_eq!(speed(buf.push(DeltaKind::Text, "b", ms(200))), None);
        assert_eq!(speed(buf.push(DeltaKind::Text, "c", ms(400))), None);
        // "bcd" after the first chunk: 3 chars, 1.5 tokens, in 1 s.
        assert_eq!(speed(buf.push(DeltaKind::Text, "d", ms(1000))), Some(1.5));
    }

    #[test]
    fn tokens_per_char_learns_from_measured_calls_only() {
        use crate::domain::stats::MessageStats;
        assert_eq!(tokens_per_char(&[]), DEFAULT_TOKENS_PER_CHAR);
        let delta = |id: &str, n: usize| Event::MessageDelta {
            message_id: id.into(),
            content: "x".repeat(n),
            tokens_per_second: None,
        };
        let done = |id: &str, tokens: u64| Event::MessageDone {
            message_id: id.into(),
            stats: Some(MessageStats {
                model: "m".into(),
                total_time: 1.0,
                prefill_tokens: 0,
                cached_prompt_tokens: 0,
                generation_tokens: tokens,
                prefill_speed: 0.0,
                generation_speed: 0.0,
            }),
            partial: false,
        };
        let events = [
            delta("a", 300),
            Event::ToolCall {
                message_id: "a".into(),
                tool_call: crate::domain::event::ToolCall {
                    id: "t".into(),
                    name: "write_file".into(),
                    arguments: serde_json::json!("y".repeat(98)),
                    parse_error: None,
                },
            },
            done("a", 100),
            // Too short to say anything.
            delta("b", 50),
            done("b", 1000),
        ];
        // 300 chars + 100 for the quoted arguments.
        assert_eq!(tokens_per_char(&events), 0.25);
    }
}
