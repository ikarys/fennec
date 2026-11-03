//! The session's blocks served ready to display: the call waiting for the human's approval, the waiver waiting for an answer, the messages queued for the next rounds, the criteria in force and the decisions recorded. Folded by the domain, the UI computes none.

use crate::domain::criteria::{CriterionState, CriterionStatus};
use crate::domain::criteria_fold;
use crate::domain::decision::{Decision, decisions_in_force};
use crate::domain::event::Event;
use crate::domain::preflight;
use crate::domain::queue;
use crate::domain::rule_approval;
use crate::domain::waiver;
use serde::Serialize;

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

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaiverView {
    pub call_id: String,
    pub path: String,
    pub reason: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedView {
    pub queue_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionView {
    pub id: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
    pub status: CriterionStatus,
    pub passed_before: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub pending_approval: Option<ApprovalView>,
    pub pending_waiver: Option<WaiverView>,
    pub queued: Vec<QueuedView>,
    pub criteria: Vec<CriterionView>,
    pub decisions: Vec<Decision>,
}

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

impl CriterionView {
    pub fn new(state: &CriterionState, passed_before: &[String]) -> Self {
        // passed_before is a display hint from the last preflight, not part of the criterion state.
        Self {
            id: state.criterion.id.clone(),
            description: state.criterion.description.clone(),
            check: state.criterion.check.clone(),
            status: state.status.clone(),
            passed_before: passed_before.contains(&state.criterion.id),
        }
    }
}

pub fn thread_view(events: &[Event]) -> ThreadView {
    // The domain folds each block; the view only shapes it for the UI.
    let passed_before = preflight::passed_before(events);
    ThreadView {
        pending_approval: rule_approval::pending(events).map(ApprovalView::from),
        pending_waiver: waiver::pending(events).map(WaiverView::from),
        queued: queue::pending(events)
            .into_iter()
            .map(QueuedView::from)
            .collect(),
        criteria: criteria_fold::in_force(events)
            .iter()
            .map(|state| CriterionView::new(state, &passed_before))
            .collect(),
        decisions: decisions_in_force(events),
    }
}

#[cfg(test)]
mod tests {
    use super::{ApprovalView, CriterionView, QueuedView, WaiverView, thread_view};
    use crate::domain::criteria::{Criterion, CriterionState, CriterionStatus, Provenance};
    use crate::domain::event::{Event, QueueMode, ToolCall};
    use crate::domain::preflight::{PreflightResult, PreflightVerdict};

    fn make_tool_call(id: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: "tool".to_string(),
            arguments: serde_json::Value::Null,
            parse_error: None,
            raw_arguments: None,
        }
    }

    #[test]
    fn empty_log_has_nothing_pending() {
        let view = thread_view(&[]);

        assert!(view.pending_approval.is_none());
        assert!(view.pending_waiver.is_none());
        assert!(view.queued.is_empty());
        assert!(view.criteria.is_empty());
        assert!(view.decisions.is_empty());
    }

    #[test]
    fn serves_the_pending_approval() {
        let events = vec![
            Event::ToolCall {
                message_id: "m1".into(),
                tool_call: make_tool_call("call-1"),
            },
            Event::RuleApprovalRequested {
                call_id: "call-1".into(),
                subject: "subject".into(),
                rule_id: None,
                kind: "allow".into(),
                pattern: "pattern".into(),
                access: Some("read".into()),
                folders: vec!["folder".into()],
            },
        ];

        let view = thread_view(&events);

        assert_eq!(
            view.pending_approval,
            Some(ApprovalView {
                call_id: "call-1".into(),
                subject: "subject".into(),
                kind: "allow".into(),
                pattern: "pattern".into(),
                access: Some("read".into()),
                folders: vec!["folder".into()],
            })
        );
        assert!(view.pending_waiver.is_none());
        assert!(view.queued.is_empty());
        assert!(view.criteria.is_empty());
        assert!(view.decisions.is_empty());
    }

    #[test]
    fn an_answered_approval_is_not_pending() {
        let events = vec![
            Event::ToolCall {
                message_id: "m1".into(),
                tool_call: make_tool_call("call-1"),
            },
            Event::RuleApprovalRequested {
                call_id: "call-1".into(),
                subject: "subject".into(),
                rule_id: None,
                kind: "allow".into(),
                pattern: "pattern".into(),
                access: Some("read".into()),
                folders: vec!["folder".into()],
            },
            Event::RuleApprovalAnswered {
                call_id: "call-1".into(),
                choice: "yes".into(),
                rule_id: None,
            },
        ];

        assert!(thread_view(&events).pending_approval.is_none());
    }

    #[test]
    fn serves_the_pending_waiver() {
        let events = vec![
            Event::ToolCall {
                message_id: "m1".into(),
                tool_call: make_tool_call("call-1"),
            },
            Event::WaiverRequested {
                call_id: "call-1".into(),
                path: "path".into(),
                reason: "reason".into(),
            },
        ];

        let view = thread_view(&events);

        assert_eq!(
            view.pending_waiver,
            Some(WaiverView {
                call_id: "call-1".into(),
                path: "path".into(),
                reason: "reason".into(),
            })
        );
        assert!(view.pending_approval.is_none());
        assert!(view.queued.is_empty());
        assert!(view.criteria.is_empty());
        assert!(view.decisions.is_empty());
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
            Event::QueueDrained {
                queue_id: "q1".into(),
            },
            Event::QueueAdded {
                queue_id: "q3".into(),
                mode: QueueMode::Asap,
                content: "third".into(),
                queued_at: 3,
            },
            Event::QueueCancelled {
                queue_id: "q3".into(),
            },
        ];

        let view = thread_view(&events);

        assert_eq!(
            view.queued,
            vec![QueuedView {
                queue_id: "q2".into(),
                content: "second".into(),
            }]
        );
    }

    #[test]
    fn serves_the_criteria_with_their_status_and_passed_before() {
        let events = vec![
            Event::CriteriaSet {
                criteria: vec![
                    Criterion {
                        id: "C1".into(),
                        description: "d1".into(),
                        check: Some("true".into()),
                    },
                    Criterion {
                        id: "C2".into(),
                        description: "d2".into(),
                        check: None,
                    },
                ],
            },
            Event::CriteriaPreflight {
                message_id: "m1".into(),
                results: vec![PreflightResult {
                    criterion_id: "C1".into(),
                    command: "true".into(),
                    verdict: PreflightVerdict::PassesAlready,
                    exit_code: Some(0),
                    output: String::new(),
                }],
            },
            Event::CriterionUpdated {
                criterion_id: "C2".into(),
                status: CriterionStatus::Passed {
                    provenance: Provenance::Indicative {
                        agent: "verify".into(),
                        reason: "ok".into(),
                    },
                },
            },
        ];

        let view = thread_view(&events);

        assert_eq!(
            view.criteria,
            vec![
                CriterionView {
                    id: "C1".into(),
                    description: "d1".into(),
                    check: Some("true".into()),
                    status: CriterionStatus::Pending,
                    passed_before: true,
                },
                CriterionView {
                    id: "C2".into(),
                    description: "d2".into(),
                    check: None,
                    status: CriterionStatus::Passed {
                        provenance: Provenance::Indicative {
                            agent: "verify".into(),
                            reason: "ok".into(),
                        },
                    },
                    passed_before: false,
                },
            ]
        );
    }

    #[test]
    fn serves_the_decisions_in_order() {
        let events = vec![
            Event::DecisionRecorded {
                call_id: "call-1".into(),
                title: "first".into(),
                choice: "a".into(),
                reason: "r".into(),
                alternatives: vec!["b".into()],
            },
            Event::DecisionRecorded {
                call_id: "call-2".into(),
                title: "second".into(),
                choice: "c".into(),
                reason: "r".into(),
                alternatives: vec![],
            },
        ];

        let view = thread_view(&events);

        assert_eq!(
            view.decisions
                .iter()
                .map(|decision| decision.title.clone())
                .collect::<Vec<_>>(),
            vec!["first".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn serializes_with_camel_case_and_nulls() {
        let value = serde_json::to_value(thread_view(&[])).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "pendingApproval": null,
                "pendingWaiver": null,
                "queued": [],
                "criteria": [],
                "decisions": []
            })
        );
    }

    #[test]
    fn a_criterion_serializes_its_status_with_its_provenance() {
        let state = CriterionState {
            criterion: Criterion {
                id: "C1".into(),
                description: "d".into(),
                check: None,
            },
            status: CriterionStatus::Failed {
                provenance: Provenance::Verified {
                    command: "cargo test".into(),
                    exit_code: Some(1),
                    output: "boom".into(),
                },
            },
        };

        let value = serde_json::to_value(CriterionView::new(&state, &[])).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "id": "C1",
                "description": "d",
                "status": {
                    "type": "failed",
                    "provenance": {
                        "kind": "verified",
                        "command": "cargo test",
                        "exitCode": 1,
                        "output": "boom"
                    }
                },
                "passedBefore": false
            })
        );
    }
}
