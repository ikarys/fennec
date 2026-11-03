//! The session's pending blocks, served ready to display: the call waiting for the human's approval, the waiver waiting for an answer, the messages queued for the next rounds. Folded by the domain, the UI computes none.

use crate::domain::event::Event;
use crate::domain::queue;
use crate::domain::rule_approval;
use crate::domain::waiver;
use serde::Serialize;

/// The approval block the UI shows while a tool call waits for a human decision.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalView {
    pub call_id: String,
    pub subject: String,
    pub kind: String,
    pub pattern: String,
    pub access: Option<String>,
    pub folders: Vec<String>,
}

/// The waiver block the UI shows while a protected-file request waits for an answer.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaiverView {
    pub call_id: String,
    pub path: String,
    pub reason: String,
}

/// One queued message, in the order the domain says it will run.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedView {
    pub queue_id: String,
    pub content: String,
}

/// The ready-to-display thread state for `GET /api/sessions/{id}/thread`.
// All three keys are part of the UI contract, so `None` serializes as `null`.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub pending_approval: Option<ApprovalView>,
    pub pending_waiver: Option<WaiverView>,
    pub queued: Vec<QueuedView>,
}

// From impls keep the domain-to-view projection local to each type.
impl From<rule_approval::Pending> for ApprovalView {
    fn from(pending: rule_approval::Pending) -> Self {
        Self {
            call_id: pending.call_id,
            subject: pending.subject,
            kind: pending.kind,
            pattern: pending.pattern,
            access: pending.access,
            folders: pending.folders,
        }
    }
}

impl From<waiver::Request> for WaiverView {
    fn from(request: waiver::Request) -> Self {
        Self {
            call_id: request.call_id,
            path: request.path,
            reason: request.reason,
        }
    }
}

impl From<queue::Queued> for QueuedView {
    fn from(queued: queue::Queued) -> Self {
        Self {
            queue_id: queued.queue_id,
            content: queued.content,
        }
    }
}

/// Build the view by asking the domain for its three pending folds.
// The domain owns the folding; this module only reshapes it for JSON.
pub fn thread_view(events: &[Event]) -> ThreadView {
    ThreadView {
        pending_approval: rule_approval::pending(events).map(ApprovalView::from),
        pending_waiver: waiver::pending(events).map(WaiverView::from),
        queued: queue::pending(events)
            .into_iter()
            .map(QueuedView::from)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{QueueMode, ToolCall};

    fn make_tool_call(id: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: "tool".into(),
            arguments: serde_json::Value::Null,
            parse_error: None,
            raw_arguments: None,
        }
    }

    #[test]
    fn empty_log_has_nothing_pending() {
        assert_eq!(
            thread_view(&[]),
            ThreadView {
                pending_approval: None,
                pending_waiver: None,
                queued: vec![],
            }
        );
    }

    #[test]
    fn serves_the_pending_approval() {
        let events = vec![
            Event::ToolCall {
                message_id: "m1".into(),
                tool_call: make_tool_call("t1"),
            },
            Event::RuleApprovalRequested {
                call_id: "t1".into(),
                subject: "git push".into(),
                rule_id: None,
                kind: "command".into(),
                pattern: "git push".into(),
                access: None,
                folders: vec![],
            },
        ];

        assert_eq!(
            thread_view(&events).pending_approval,
            Some(ApprovalView {
                call_id: "t1".into(),
                subject: "git push".into(),
                kind: "command".into(),
                pattern: "git push".into(),
                access: None,
                folders: vec![],
            })
        );
    }

    #[test]
    fn an_answered_approval_is_not_pending() {
        let mut events = vec![
            Event::ToolCall {
                message_id: "m1".into(),
                tool_call: make_tool_call("t1"),
            },
            Event::RuleApprovalRequested {
                call_id: "t1".into(),
                subject: "git push".into(),
                rule_id: None,
                kind: "command".into(),
                pattern: "git push".into(),
                access: None,
                folders: vec![],
            },
        ];
        events.push(Event::RuleApprovalAnswered {
            call_id: "t1".into(),
            choice: "once".into(),
            rule_id: None,
        });

        assert_eq!(thread_view(&events).pending_approval, None);
    }

    #[test]
    fn serves_the_pending_waiver() {
        let events = vec![
            Event::ToolCall {
                message_id: "m2".into(),
                tool_call: make_tool_call("t2"),
            },
            Event::WaiverRequested {
                call_id: "t2".into(),
                path: "protected.txt".into(),
                reason: "needed".into(),
            },
        ];

        assert_eq!(
            thread_view(&events).pending_waiver,
            Some(WaiverView {
                call_id: "t2".into(),
                path: "protected.txt".into(),
                reason: "needed".into(),
            })
        );

        let mut answered = events;
        answered.push(Event::WaiverAnswered {
            call_id: "t2".into(),
            granted: true,
            expires_at: None,
            adr: None,
        });

        assert_eq!(thread_view(&answered).pending_waiver, None);
    }

    #[test]
    fn serves_the_queue_in_order_without_the_drained_or_cancelled() {
        let events = vec![
            Event::QueueAdded {
                queue_id: "q1".into(),
                mode: QueueMode::Asap,
                content: "first".into(),
                queued_at: 1,
            },
            Event::QueueAdded {
                queue_id: "q2".into(),
                mode: QueueMode::Asap,
                content: "second".into(),
                queued_at: 2,
            },
            Event::QueueAdded {
                queue_id: "q3".into(),
                mode: QueueMode::Asap,
                content: "third".into(),
                queued_at: 3,
            },
            Event::QueueDrained {
                queue_id: "q1".into(),
            },
            Event::QueueCancelled {
                queue_id: "q3".into(),
            },
        ];

        assert_eq!(
            thread_view(&events).queued,
            vec![QueuedView {
                queue_id: "q2".into(),
                content: "second".into(),
            }]
        );
    }

    #[test]
    fn serializes_with_camel_case_and_nulls() {
        assert_eq!(
            serde_json::to_value(thread_view(&[])).unwrap(),
            serde_json::json!({
                "pendingApproval": null,
                "pendingWaiver": null,
                "queued": []
            })
        );
    }
}
