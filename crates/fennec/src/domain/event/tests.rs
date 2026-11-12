use super::*;
use serde_json::json;

#[test]
fn mode_changed_round_trips() {
    let wire = json!({
        "type": "mode.changed",
        "data": {
            "mode": "builder",
            "auto": false
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        &event,
        Event::ModeChanged {
            mode,
            auto: false,
            reason: None
        } if mode == "builder"
    ));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn plan_proposed_round_trips() {
    use crate::domain::plan::{Plan, PlanStep, PlannedCriterion};
    let wire = json!({
        "type": "plan.proposed",
        "data": {
            "callId": "c1",
            "plan": {
                "title": "Add the plan tool",
                "goal": "The plan ends with propose_plan",
                "steps": [
                    {"title": "Write the domain", "files": ["src/plan.rs"], "change": "The vocabulary"}
                ],
                "doneWhen": [
                    {"text": "The build passes", "check": "cargo build"},
                    {"text": "The code stays clean"}
                ],
                "risks": []
            },
            "markdown": "## Plan: Add the plan tool"
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        event,
        Event::PlanProposed {
            call_id: "c1".into(),
            plan: Plan {
                title: "Add the plan tool".into(),
                goal: "The plan ends with propose_plan".into(),
                context: String::new(),
                constraints: String::new(),
                out_of_scope: String::new(),
                deliverable: String::new(),
                steps: vec![PlanStep {
                    title: "Write the domain".into(),
                    files: vec!["src/plan.rs".into()],
                    change: "The vocabulary".into(),
                }],
                done_when: vec![
                    PlannedCriterion {
                        text: "The build passes".into(),
                        check: Some("cargo build".into()),
                    },
                    PlannedCriterion {
                        text: "The code stays clean".into(),
                        check: None,
                    },
                ],
                risks: Vec::new(),
            },
            markdown: "## Plan: Add the plan tool".into(),
        }
    );
    assert_eq!(event.kind(), "plan.proposed");
    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn report_submitted_round_trips() {
    use crate::domain::report::{ChangedFile, Report, ReportCriterion, ReportStep};
    let wire = json!({
        "type": "report.submitted",
        "data": {
            "callId": "c1",
            "report": {
                "title": "Add the report tool",
                "summary": "The builder ends its turn with submit_report",
                "steps": [
                    {"title": "Write the domain", "done": true, "note": "The vocabulary"},
                    {"title": "Wire the tool", "done": false}
                ],
                "doneWhen": [
                    {"text": "The build passes", "met": true, "how": "cargo build"},
                    {"text": "The code stays clean", "met": false}
                ],
                "changed": [{"path": "src/report.rs", "what": "The vocabulary"}],
                "test": "",
                "todo": ""
            },
            "markdown": "## Report: Add the report tool"
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        event,
        Event::ReportSubmitted {
            call_id: "c1".into(),
            report: Report {
                title: "Add the report tool".into(),
                summary: "The builder ends its turn with submit_report".into(),
                steps: vec![
                    ReportStep {
                        title: "Write the domain".into(),
                        done: true,
                        note: Some("The vocabulary".into()),
                    },
                    ReportStep {
                        title: "Wire the tool".into(),
                        done: false,
                        note: None,
                    },
                ],
                done_when: vec![
                    ReportCriterion {
                        text: "The build passes".into(),
                        met: true,
                        how: Some("cargo build".into()),
                    },
                    ReportCriterion {
                        text: "The code stays clean".into(),
                        met: false,
                        how: None,
                    },
                ],
                changed: vec![ChangedFile {
                    path: "src/report.rs".into(),
                    what: "The vocabulary".into(),
                }],
                test: String::new(),
                todo: String::new(),
            },
            markdown: "## Report: Add the report tool".into(),
        }
    );
    assert_eq!(event.kind(), "report.submitted");
    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn model_changed_round_trips() {
    let wire = json!({
        "type": "model.changed",
        "data": {
            "model": {"provider": "local", "model": "m"}
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        &event,
        Event::ModelChanged { model }
            if model.provider == "local" && model.model == "m"
    ));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn effort_changed_round_trips() {
    let wire = json!({
        "type": "effort.changed",
        "data": {
            "effort": "high"
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        &event,
        Event::EffortChanged {
            effort: Some(ReasoningEffort::High)
        }
    ));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);

    let wire = json!({
        "type": "effort.changed",
        "data": {
            "effort": null
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(&event, Event::EffortChanged { effort: None }));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn session_rewound_round_trips() {
    let wire = json!({
        "type": "session.rewound",
        "data": {
            "messageId": "m"
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        &event,
        Event::SessionRewound { message_id } if message_id == "m"
    ));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn session_name_generated_round_trips() {
    let wire = json!({
        "type": "session.name_generated",
        "data": {
            "name": "React setup"
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        &event,
        Event::SessionNameGenerated { name } if name == "React setup"
    ));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn message_start_metadata_round_trips() {
    let wire = json!({
        "type": "message.start",
        "data": {
            "messageId": "m",
            "role": "user",
            "content": "reminder",
            "isSystemGenerated": true,
            "metadata": {
                "type": "agent",
                "name": "Builder",
                "color": "#3b82f6",
                "kind": "definition"
            }
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        &event,
        Event::MessageStart {
            message_id,
            role: Role::User,
            content: Some(content),
            is_system_generated: true,
            metadata: Some(MessageMetadata {
                source: MetadataSource::Agent,
                name,
                color,
                kind: ReminderKind::Definition,
            }),
            ..
        } if message_id == "m"
            && content == "reminder"
            && name == "Builder"
            && color == "#3b82f6"
    ));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}

#[test]
fn message_start_attachments_round_trip_and_omit_when_empty() {
    let with = Event::MessageStart {
        message_id: "m".into(),
        role: Role::User,
        content: Some("hi".into()),
        is_system_generated: false,
        metadata: None,
        attachments: vec![Attachment {
            id: "a".into(),
            filename: "a.png".into(),
            mime_type: "image/png".into(),
            size: 1,
            data: "data:image/png;base64,AAAA".into(),
        }],
    };
    let wire = serde_json::to_value(&with).unwrap();
    assert_eq!(wire["data"]["attachments"][0]["mimeType"], "image/png");
    assert_eq!(serde_json::from_value::<Event>(wire).unwrap(), with);

    let without = Event::MessageStart {
        message_id: "m".into(),
        role: Role::User,
        content: Some("hi".into()),
        is_system_generated: false,
        metadata: None,
        attachments: Vec::new(),
    };
    let wire = serde_json::to_value(&without).unwrap();
    assert!(wire["data"].get("attachments").is_none());
    assert_eq!(serde_json::from_value::<Event>(wire).unwrap(), without);
}

#[test]
fn workflow_node_started_with_via_round_trips() {
    let wire = json!({
        "type": "workflow.node.started",
        "data": {
            "runId": "r",
            "nodeId": "n",
            "pass": 1,
            "via": {
                "from": "f",
                "port": "done"
            }
        }
    });

    let event: Event = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(
        &event,
        Event::WorkflowNodeStarted {
            run_id,
            node_id,
            pass: 1,
            via: Some(via),
            child_session_id: None,
            started_at: None,
        } if run_id == "r"
            && node_id == "n"
            && via.from == "f"
            && via.port == "done"
    ));

    assert_eq!(serde_json::to_value(&event).unwrap(), wire);
}
