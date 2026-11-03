//! Queue state (a message queued, removed, drained)
//! as a fold of the log.

use super::event::Event;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Queued {
    pub queue_id: String,
    pub content: String,
}

/// The messages still waiting: every `queue.added` that has not been drained
/// or cancelled. The whole log is used, not the model view: a compaction must
/// not drop a queued message.
pub fn pending(events: &[Event]) -> Vec<Queued> {
    let gone: HashSet<&str> = events
        .iter()
        .filter_map(|event| match event {
            Event::QueueDrained { queue_id } | Event::QueueCancelled { queue_id } => {
                Some(queue_id.as_str())
            }
            _ => None,
        })
        .collect();

    events
        .iter()
        .filter_map(|event| match event {
            Event::QueueAdded {
                queue_id, content, ..
            } if !gone.contains(queue_id.as_str()) => Some(Queued {
                queue_id: queue_id.clone(),
                content: content.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// The leading run of queued messages that can be dropped into the running
/// turn between tool rounds. A slash command (`/compact`, `/<skill>`) waits
/// for the end of the turn so it is resolved like a typed message; what
/// follows it keeps its order behind it.
pub fn steerable(pending: &[Queued]) -> &[Queued] {
    let end = pending
        .iter()
        .take_while(|queued| !queued.content.starts_with('/'))
        .count();
    &pending[..end]
}

#[derive(Debug, PartialEq)]
pub enum AfterTurn {
    Idle,
    Cancel(Vec<String>),
    Hold,
    Start(Queued),
}

/// What a finished turn does with the queue.
pub fn after_turn(events: &[Event], stopped: bool) -> AfterTurn {
    let pending = pending(events);
    if pending.is_empty() {
        return AfterTurn::Idle;
    }
    if stopped {
        return AfterTurn::Cancel(pending.into_iter().map(|q| q.queue_id).collect());
    }
    // A workflow still running (waiting on the human, paused) is not a turn to
    // start beside: its same-session agent nodes drain the queue, and what is
    // left waits for the run to end.
    if super::rule_approval::pending(events).is_some()
        || super::waiver::pending(events).is_some()
        || super::workflow::run::current_run(events).is_some_and(|run| run.finished.is_none())
    {
        return AfterTurn::Hold;
    }
    AfterTurn::Start(pending.into_iter().next().expect("pending is not empty"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{QueueMode, ToolCall};
    use serde_json::json;

    fn added(id: &str, content: &str) -> Event {
        Event::QueueAdded {
            queue_id: id.into(),
            mode: QueueMode::Asap,
            content: content.into(),
            queued_at: 1,
        }
    }

    fn drained(id: &str) -> Event {
        Event::QueueDrained {
            queue_id: id.into(),
        }
    }

    fn cancelled(id: &str) -> Event {
        Event::QueueCancelled {
            queue_id: id.into(),
        }
    }

    #[test]
    fn pending_keeps_added_until_drained_or_cancelled() {
        let events = vec![
            added("a", "first"),
            added("b", "second"),
            drained("a"),
            added("c", "third"),
            cancelled("c"),
        ];
        assert_eq!(
            pending(&events),
            vec![Queued {
                queue_id: "b".into(),
                content: "second".into()
            }]
        );
    }

    #[test]
    fn steerable_stops_at_the_first_slash_command() {
        let queued = vec![
            Queued {
                queue_id: "a".into(),
                content: "go on".into(),
            },
            Queued {
                queue_id: "b".into(),
                content: "/compact".into(),
            },
            Queued {
                queue_id: "c".into(),
                content: "after".into(),
            },
        ];
        assert_eq!(steerable(&queued), &queued[..1]);
        assert_eq!(steerable(&queued[1..]), &queued[1..][..0]);
        assert_eq!(steerable(&queued[..1]), &queued[..1]);
    }

    #[test]
    fn after_turn_is_idle_without_a_queue() {
        assert_eq!(after_turn(&[], false), AfterTurn::Idle);
    }

    #[test]
    fn after_turn_cancels_everything_when_stopped() {
        let events = vec![added("a", "first"), added("b", "second")];
        assert_eq!(
            after_turn(&events, true),
            AfterTurn::Cancel(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn after_turn_starts_the_first_pending_message() {
        let events = vec![added("a", "first"), added("b", "second")];
        assert_eq!(
            after_turn(&events, false),
            AfterTurn::Start(Queued {
                queue_id: "a".into(),
                content: "first".into()
            })
        );
    }

    #[test]
    fn after_turn_holds_while_a_waiver_is_pending() {
        let events = vec![
            Event::ToolCall {
                message_id: "m".into(),
                tool_call: ToolCall {
                    id: "c1".into(),
                    name: "request_waiver".into(),
                    arguments: json!({}),
                    parse_error: None,
                    raw_arguments: None,
                },
            },
            Event::WaiverRequested {
                call_id: "c1".into(),
                path: "AGENTS.md".into(),
                reason: "why".into(),
            },
            added("a", "first"),
        ];
        assert_eq!(after_turn(&events, false), AfterTurn::Hold);
    }

    #[test]
    fn after_turn_holds_while_a_workflow_runs() {
        let workflow = serde_json::from_value(json!({
            "id": "w", "name": "w", "nodes": [], "edges": []
        }))
        .expect("a minimal workflow");
        let events = vec![
            Event::WorkflowStarted {
                run_id: "r".into(),
                workflow,
                task: String::new(),
                step_by_step: false,
                started_at: None,
            },
            added("a", "first"),
        ];
        assert_eq!(after_turn(&events, false), AfterTurn::Hold);
    }
}
