//! Drain the queue.

use super::turn::Turn;
use crate::domain::event::Event;
use crate::domain::queue;

/// Drops the steerable queued messages into the running turn, between tool
/// rounds. Append-only, so the prompt cache prefix is kept.
pub(super) fn drain(turn: &mut Turn) -> Result<(), String> {
    let events = turn.log.events(turn.session_id)?;
    let pending = queue::pending(&events);
    for item in queue::steerable(&pending) {
        let content = item.content.clone();
        let queue_id = item.queue_id.clone();
        turn.append_user(&content, false, None, Vec::new())?;
        turn.append(Event::QueueDrained { queue_id })?;
    }
    Ok(())
}
