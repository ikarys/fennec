//! What a finished turn leaves that needs the human's eye: a question to
//! answer, a workflow waiting on a choice, work ready to integrate, an error.
//! Derived from a session's state and its log, so it clears on its own once
//! the human acts (no "read" flag to maintain).

use super::event::{DoneReason, Event};
use super::open_calls::open_calls;
use super::project::SessionSummary;
use serde::Serialize;
use std::collections::HashSet;

/// The kind of thing a session is waiting for; the badge and the notification
/// are worded from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AttentionKind {
    /// `ask_user` is open: the model asked a question and stops on the answer.
    Question,
    /// A workflow stopped on a human node, waiting for a choice.
    WorkflowPause,
    /// A workflow ended and nothing integrated or discarded its work since.
    AwaitingIntegration,
    /// Finished work whose branch conflicts with the human's branch: it cannot
    /// be integrated until resolved.
    Conflict,
    /// The last turn failed.
    Error,
}

/// One session that needs the human, as the badge list and the notification
/// carry it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    pub session_id: String,
    pub project_id: String,
    pub title: String,
    pub kind: AttentionKind,
    /// The question text, the workflow's question, or the error message; empty
    /// for work that is ready to integrate.
    pub detail: String,
}

/// What, if anything, a session needs the human for. `None` while a turn
/// runs, on a plain answer, and when nothing is left to look at.
pub fn attention(summary: &SessionSummary, events: &[Event], running: bool) -> Option<Attention> {
    if running {
        return None;
    }
    let base = || Attention {
        session_id: summary.id.clone(),
        project_id: summary.project_id.clone(),
        title: summary.title.clone(),
        detail: String::new(),
        kind: AttentionKind::Question,
    };
    // A question awaiting an answer is the most urgent thing to look at.
    if let Some(question) = open_question(events) {
        return Some(Attention {
            detail: question,
            kind: AttentionKind::Question,
            ..base()
        });
    }
    // A workflow stopped on a human node, waiting for a choice.
    if summary.last_done == Some(DoneReason::WaitingForUser)
        && let Some(question) = last_workflow_question(events)
    {
        return Some(Attention {
            detail: question,
            kind: AttentionKind::WorkflowPause,
            ..base()
        });
    }
    // A workflow finished and nothing integrated or discarded its work since.
    if summary.awaiting_integration {
        return Some(Attention {
            kind: AttentionKind::AwaitingIntegration,
            ..base()
        });
    }
    // The last turn failed.
    if summary.last_done == Some(DoneReason::Error) {
        return Some(Attention {
            detail: last_error(events).unwrap_or_default(),
            kind: AttentionKind::Error,
            ..base()
        });
    }
    None
}

/// The most recent `ask_user` that is still open (no answer recorded).
fn open_question(events: &[Event]) -> Option<String> {
    let calls = open_calls(events);
    let open: HashSet<&str> = calls
        .iter()
        .filter(|call| call.awaits_answer)
        .map(|call| call.call_id.as_str())
        .collect();
    events.iter().rev().find_map(|e| match e {
        Event::ChatAskUser {
            call_id, question, ..
        } if open.contains(call_id.as_str()) => Some(question.clone()),
        _ => None,
    })
}

/// The most recent human node's question in the log.
fn last_workflow_question(events: &[Event]) -> Option<String> {
    events.iter().rev().find_map(|e| match e {
        Event::WorkflowHumanAsked { question, .. } => Some(question.clone()),
        _ => None,
    })
}

/// The most recent error message in the log.
fn last_error(events: &[Event]) -> Option<String> {
    events.iter().rev().find_map(|e| match e {
        Event::ChatError { error, .. } => Some(error.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{AskType, ToolCall};
    use serde_json::json;

    fn summary(last_done: Option<DoneReason>, awaiting: bool) -> SessionSummary {
        SessionSummary {
            id: "s1".into(),
            project_id: "p1".into(),
            title: "My session".into(),
            created_at: 0,
            updated_at: 0,
            last_done,
            parent_id: None,
            awaiting_integration: awaiting,
            linked_project_id: None,
        }
    }

    fn call(id: &str, name: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: name.into(),
                arguments: json!({}),
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    fn result(id: &str) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: id.into(),
            result: crate::domain::event::ToolResult::success("ok".into(), false),
        }
    }

    #[test]
    fn an_open_question_needs_an_answer() {
        let events = vec![
            call("c1", "ask_user"),
            Event::ChatAskUser {
                call_id: "c1".into(),
                question: "Which color?".into(),
                ask_type: AskType::Text,
                options: None,
            },
        ];
        let a = attention(
            &summary(Some(DoneReason::WaitingForUser), false),
            &events,
            false,
        )
        .unwrap();
        assert_eq!(a.kind, AttentionKind::Question);
        assert_eq!(a.detail, "Which color?");
        assert_eq!(a.session_id, "s1");
    }

    #[test]
    fn an_answered_question_does_not() {
        let events = vec![
            call("c1", "ask_user"),
            Event::ChatAskUser {
                call_id: "c1".into(),
                question: "Which color?".into(),
                ask_type: AskType::Text,
                options: None,
            },
            result("c1"),
        ];
        assert!(
            attention(
                &summary(Some(DoneReason::WaitingForUser), false),
                &events,
                false
            )
            .is_none()
        );
    }

    #[test]
    fn a_workflow_waiting_on_a_choice() {
        let events = vec![Event::WorkflowHumanAsked {
            run_id: "r".into(),
            node_id: "n".into(),
            question: "Approve the plan?".into(),
        }];
        let a = attention(
            &summary(Some(DoneReason::WaitingForUser), false),
            &events,
            false,
        )
        .unwrap();
        assert_eq!(a.kind, AttentionKind::WorkflowPause);
        assert_eq!(a.detail, "Approve the plan?");
    }

    #[test]
    fn finished_work_waits_for_integration() {
        let a = attention(&summary(Some(DoneReason::Complete), true), &[], false).unwrap();
        assert_eq!(a.kind, AttentionKind::AwaitingIntegration);
        assert!(a.detail.is_empty());
    }

    #[test]
    fn a_failed_turn_needs_a_look() {
        let events = vec![Event::ChatError {
            error: "model timeout".into(),
            recoverable: false,
        }];
        let a = attention(&summary(Some(DoneReason::Error), false), &events, false).unwrap();
        assert_eq!(a.kind, AttentionKind::Error);
        assert_eq!(a.detail, "model timeout");
    }

    #[test]
    fn a_plain_answer_needs_nothing() {
        assert!(attention(&summary(Some(DoneReason::Complete), false), &[], false).is_none());
    }

    #[test]
    fn a_running_turn_needs_nothing() {
        let events = vec![Event::ChatAskUser {
            call_id: "c1".into(),
            question: "q".into(),
            ask_type: AskType::Text,
            options: None,
        }];
        assert!(
            attention(
                &summary(Some(DoneReason::WaitingForUser), false),
                &events,
                true
            )
            .is_none()
        );
    }

    #[test]
    fn a_question_outranks_a_workflow_pause() {
        let events = vec![
            call("c1", "ask_user"),
            Event::WorkflowHumanAsked {
                run_id: "r".into(),
                node_id: "n".into(),
                question: "plan?".into(),
            },
            Event::ChatAskUser {
                call_id: "c1".into(),
                question: "which?".into(),
                ask_type: AskType::Text,
                options: None,
            },
        ];
        let a = attention(
            &summary(Some(DoneReason::WaitingForUser), false),
            &events,
            false,
        )
        .unwrap();
        assert_eq!(a.kind, AttentionKind::Question);
        assert_eq!(a.detail, "which?");
    }

    #[test]
    fn the_kind_serializes_camel_case() {
        assert_eq!(
            serde_json::to_string(&AttentionKind::WorkflowPause).unwrap(),
            "\"workflowPause\""
        );
        assert_eq!(
            serde_json::to_string(&AttentionKind::AwaitingIntegration).unwrap(),
            "\"awaitingIntegration\""
        );
        assert_eq!(
            serde_json::to_string(&AttentionKind::Conflict).unwrap(),
            "\"conflict\""
        );
    }
}
