//! The session's blocks served ready to display: the call waiting for the human's approval, the waiver waiting for an answer, the messages queued for the next rounds, the criteria in force, the decisions recorded and the messages the human may replay. Folded by the domain, the UI computes none.

use crate::domain::check_kind;
use crate::domain::criteria::{CriterionState, CriterionStatus};
use crate::domain::criteria_fold;
use crate::domain::decision::{Decision, decisions_in_force};
use crate::domain::event::Event;
use crate::domain::modified_files::{self, ModifiedFiles};
use crate::domain::plan;
use crate::domain::preflight;
use crate::domain::queue;
use crate::domain::replay;
use crate::domain::rule_approval;
use crate::domain::waiver;
use backlog::{Idea, IdeaState};
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
    pub existence_only: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub pending_approval: Option<ApprovalView>,
    pub pending_waiver: Option<WaiverView>,
    /// The plan waiting for the human's decision (the decision bar), served whole.
    pub pending_plan: Option<plan::Pending>,
    /// The plan the human handed to the backlog instead: the bar gives way to a note.
    pub handed_plan: Option<HandedPlan>,
    pub queued: Vec<QueuedView>,
    pub criteria: Vec<CriterionView>,
    pub criteria_tally: criteria_fold::Tally,
    pub decisions: Vec<Decision>,
    /// The messages the human may replay: typed, in the model's current view.
    pub replayable: Vec<String>,
    pub modified_files: ModifiedFiles,
}

impl From<rule_approval::Pending> for ApprovalView {
    fn from(pending: rule_approval::Pending) -> Self {
        Self {
            call_id: pending.call_id,
            subject: pending.subject,
            kind: pending.kind.as_str().to_string(),
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
            existence_only: state
                .criterion
                .check
                .as_deref()
                .is_some_and(check_kind::is_existence_only),
        }
    }
}

/// The kinds the thread's whole-session folds read; a fold that reads another kind must add it
/// here (`targeted_reads_fold_like_the_whole_log` guards it).
pub const SESSION_KINDS: &[&str] = &[
    "criteria.set",
    "criterion.updated",
    "criteria.preflight",
    "queue.added",
    "queue.drained",
    "queue.cancelled",
    "decision.recorded",
    "tool.call",
];

/// `window` is the model's window (`EventLog::window`), `session` the `SESSION_KINDS` events;
/// either may be the whole log.
/// A pending plan carried by the session's idea into the backlog (`Idea ready`).
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandedPlan {
    pub idea_id: String,
    /// True while the idea only waits in the queue: the human may take the plan back.
    pub undoable: bool,
}

impl ThreadView {
    /// Once the session's idea carries the pending plan, the decision is made: the queue runs it,
    /// so the bar no longer offers a launch that would run it a second time.
    pub fn hand_to_backlog(&mut self, idea: Option<&Idea>) {
        let (Some(pending), Some(idea)) = (&self.pending_plan, idea) else {
            return;
        };
        // The ready route stores the plan trimmed.
        let carried = idea.plan.as_deref().map(str::trim) == Some(pending.markdown.trim())
            && matches!(
                idea.state,
                IdeaState::Ready | IdeaState::Started | IdeaState::Completed
            );
        if carried {
            self.handed_plan = Some(HandedPlan {
                idea_id: idea.id.clone(),
                undoable: idea.state == IdeaState::Ready,
            });
            self.pending_plan = None;
        }
    }
}

pub fn thread_view(window: &[Event], session: &[Event], workdir: &str) -> ThreadView {
    // The domain folds each block; the view only shapes it for the UI.
    let passed_before = preflight::passed_before(session);
    let criteria = criteria_fold::in_force(session);
    ThreadView {
        pending_approval: rule_approval::pending(window).map(ApprovalView::from),
        pending_waiver: waiver::pending(window).map(WaiverView::from),
        pending_plan: plan::pending(window),
        handed_plan: None,
        queued: queue::pending(session)
            .into_iter()
            .map(QueuedView::from)
            .collect(),
        criteria_tally: criteria_fold::tally(&criteria),
        criteria: criteria
            .iter()
            .map(|state| CriterionView::new(state, &passed_before))
            .collect(),
        decisions: decisions_in_force(session),
        replayable: replay::replayable_ids(window),
        modified_files: modified_files::tally(session, workdir),
    }
}

#[cfg(test)]
mod tests {
    use super::{ApprovalView, CriterionView, QueuedView, WaiverView, thread_view};
    use crate::domain::criteria::{Criterion, CriterionState, CriterionStatus, Provenance};
    use crate::domain::event::{Event, QueueMode, Role, ToolCall};
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

    fn idea(state: backlog::IdeaState, plan: &str) -> backlog::Idea {
        backlog::Idea {
            id: "i1".into(),
            text: String::new(),
            state,
            session_id: Some("s1".into()),
            plan: Some(plan.into()),
            created_at: 0,
            updated_at: 0,
            halted: None,
            started_at: None,
            autonomous: false,
            attachments: vec![],
            sheet: None,
            model_fields: Default::default(),
        }
    }

    fn plan_view() -> super::ThreadView {
        let proposed = Event::PlanProposed {
            call_id: "p1".into(),
            plan: crate::domain::plan::Plan {
                title: String::new(),
                goal: String::new(),
                steps: vec![],
                done_when: vec![],
                risks: vec![],
            },
            markdown: "the plan\n".into(),
        };
        thread_view(&[proposed], &[], "/w")
    }

    #[test]
    fn a_plan_handed_to_the_backlog_leaves_the_bar() {
        let mut view = plan_view();
        view.hand_to_backlog(Some(&idea(backlog::IdeaState::Ready, "the plan")));
        assert!(view.pending_plan.is_none());
        assert_eq!(
            view.handed_plan,
            Some(super::HandedPlan {
                idea_id: "i1".into(),
                undoable: true
            })
        );

        let mut started = plan_view();
        started.hand_to_backlog(Some(&idea(backlog::IdeaState::Started, "the plan")));
        assert_eq!(
            started.handed_plan.map(|handed| handed.undoable),
            Some(false)
        );
    }

    #[test]
    fn a_revised_or_framing_plan_stays_in_the_bar() {
        for idea in [
            idea(backlog::IdeaState::Ready, "an older plan"),
            idea(backlog::IdeaState::Framing, "the plan"),
        ] {
            let mut view = plan_view();
            view.hand_to_backlog(Some(&idea));
            assert!(view.pending_plan.is_some());
            assert!(view.handed_plan.is_none());
        }
    }

    #[test]
    fn empty_log_has_nothing_pending() {
        let view = thread_view(&[], &[], "/w");

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
                kind: "path".into(),
                pattern: "pattern".into(),
                access: Some("read".into()),
                folders: vec!["folder".into()],
            },
        ];

        let view = thread_view(&events, &events, "/w");

        assert_eq!(
            view.pending_approval,
            Some(ApprovalView {
                call_id: "call-1".into(),
                subject: "subject".into(),
                kind: "path".into(),
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

        assert!(
            thread_view(&events, &events, "/w")
                .pending_approval
                .is_none()
        );
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

        let view = thread_view(&events, &events, "/w");

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

        let view = thread_view(&events, &events, "/w");

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
                    existence_only: false,
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

        let view = thread_view(&events, &events, "/w");

        assert_eq!(
            view.criteria,
            vec![
                CriterionView {
                    id: "C1".into(),
                    description: "d1".into(),
                    check: Some("true".into()),
                    status: CriterionStatus::Pending,
                    passed_before: true,
                    existence_only: false,
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
                    existence_only: false,
                },
            ]
        );
    }

    #[test]
    fn serves_the_replayable_messages_of_the_current_view() {
        let typed = |id: &str, system_generated: bool| Event::MessageStart {
            message_id: id.into(),
            role: Role::User,
            content: Some("text".into()),
            is_system_generated: system_generated,
            metadata: None,
            attachments: Vec::new(),
        };
        let events = vec![
            typed("u0", false),
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "s".into(),
            },
            typed("u1", false),
            typed("u2", true),
        ];

        assert_eq!(
            thread_view(&events, &events, "/w").replayable,
            vec!["u1".to_string()]
        );
    }

    #[test]
    fn targeted_reads_fold_like_the_whole_log() {
        use crate::agent::ports::EventLog;
        use crate::domain::criteria::Criterion;
        use crate::store::SqliteStore;

        let write = ToolCall {
            name: "write_file".into(),
            arguments: serde_json::json!({"path": "/w/a.rs", "content": "x\ny\n"}),
            ..make_tool_call("call-w")
        };
        let typed = |id: &str| Event::MessageStart {
            message_id: id.into(),
            role: Role::User,
            content: Some("text".into()),
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        };
        let events = vec![
            typed("u0"),
            Event::CriteriaSet {
                criteria: vec![Criterion {
                    id: "C1".into(),
                    description: "d1".into(),
                    check: Some("true".into()),
                }],
            },
            Event::DecisionRecorded {
                call_id: "call-d".into(),
                title: "t".into(),
                choice: "a".into(),
                reason: "r".into(),
                alternatives: vec![],
            },
            Event::QueueAdded {
                queue_id: "q1".into(),
                mode: QueueMode::Asap,
                content: "queued".into(),
                queued_at: 1,
            },
            Event::ToolCall {
                message_id: "m0".into(),
                tool_call: write,
            },
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "s".into(),
            },
            typed("u1"),
            Event::CriterionUpdated {
                criterion_id: "C1".into(),
                status: CriterionStatus::Failed {
                    provenance: Provenance::Indicative {
                        agent: "verify".into(),
                        reason: "no".into(),
                    },
                },
            },
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
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/tmp/thread-targeted").unwrap();
        let session = store.create_session(&project, "t").unwrap();
        for event in &events {
            store.append(&session, event).unwrap();
        }

        let targeted = thread_view(
            &store.window(&session).unwrap(),
            &store.events_of(&session, super::SESSION_KINDS).unwrap(),
            "/w",
        );
        assert_eq!(targeted, thread_view(&events, &events, "/w"));
        assert!(targeted.pending_waiver.is_some());
        assert_eq!(targeted.modified_files.count, 1);
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

        let view = thread_view(&events, &events, "/w");

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
        let value = serde_json::to_value(thread_view(&[], &[], "/w")).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "pendingApproval": null,
                "pendingWaiver": null,
                "pendingPlan": null,
                "handedPlan": null,
                "queued": [],
                "criteria": [],
                "criteriaTally": {"total": 0, "passed": 0, "failed": 0, "indicative": 0, "percent": 0},
                "decisions": [],
                "replayable": [],
                "modifiedFiles": {"count": 0, "added": 0, "removed": 0, "files": []}
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
                "passedBefore": false,
                "existenceOnly": false
            })
        );
    }

    #[test]
    fn serves_existence_only_for_search_checks_only() {
        let grep_state = CriterionState {
            criterion: Criterion {
                id: "C1".into(),
                description: "d".into(),
                check: Some("rg -q 'x' src".into()),
            },
            status: CriterionStatus::Pending,
        };
        let cargo_state = CriterionState {
            criterion: Criterion {
                id: "C2".into(),
                description: "d".into(),
                check: Some("cargo test".into()),
            },
            status: CriterionStatus::Pending,
        };

        assert!(CriterionView::new(&grep_state, &[]).existence_only);
        assert!(!CriterionView::new(&cargo_state, &[]).existence_only);
    }
}
