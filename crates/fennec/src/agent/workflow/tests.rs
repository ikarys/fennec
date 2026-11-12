use super::*;
use crate::agent::ports::{AgentRun, ChildSessions, CommandRun, Commands, Completion, EventLog};
use crate::agent::test_support::{Harness, Scripted, plan, reply};
use crate::domain::criteria::{CriterionStatus, Provenance};
use crate::domain::event::{DoneReason, Event, ToolCall, ToolResult};
use crate::domain::workflow::definition::{
    AgentContext, Edge, EndOutcome, Node, NodeKind, Position, Workflow,
};
use crate::domain::workflow::run::{RunOutcome, Via};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[derive(Default)]
struct FakeChildren {
    calls: Mutex<Vec<String>>,
    replies: Mutex<Vec<String>>,
    /// The end reason of each `send`, popped from the back; Complete when empty.
    reasons: Mutex<Vec<DoneReason>>,
    /// The answers to `last_run`, popped from the back; a completed "last" when empty.
    last_runs: Mutex<Vec<AgentRun>>,
}

impl ChildSessions for FakeChildren {
    fn create(&self, parent: &str, title: &str) -> Result<String, String> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(format!("create:{parent}:{title}"));
        Ok(format!("child-{}", calls.len()))
    }

    fn send(
        &self,
        child: &str,
        agent: &str,
        prompt: &str,
        _stop: &AtomicBool,
    ) -> Result<AgentRun, String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("send:{child}:{agent}:{prompt}"));
        let text = self.replies.lock().unwrap().pop().unwrap_or_default();
        let reason = self
            .reasons
            .lock()
            .unwrap()
            .pop()
            .unwrap_or(DoneReason::Complete);
        Ok(AgentRun { reason, text })
    }

    fn last_run(&self, child: &str) -> Result<AgentRun, String> {
        self.calls.lock().unwrap().push(format!("last_run:{child}"));
        Ok(self.last_runs.lock().unwrap().pop().unwrap_or(AgentRun {
            reason: DoneReason::Complete,
            text: "last".into(),
        }))
    }
}

#[derive(Default)]
struct FakeCommands {
    exit_codes: Mutex<Vec<i32>>,
}

impl Commands for FakeCommands {
    fn run(&self, _command: &str, _timeout: Duration, _stop: &AtomicBool) -> CommandRun {
        // One code per call, the last one repeating.
        let mut codes = self.exit_codes.lock().unwrap();
        let code = if codes.len() > 1 {
            codes.remove(0)
        } else {
            codes.first().copied().unwrap_or(0)
        };
        CommandRun {
            exit_code: Some(code),
            stopped: false,
            output: "output".into(),
        }
    }
}

fn simple_workflow() -> Workflow {
    Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "a".into(),
                name: Some("Agent A".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "{{task}}".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "a".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "a".into(),
                port: "done".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
        ],
    }
}

#[test]
fn start_agent_end() {
    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: simple_workflow(),
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowStarted { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowNodeStarted {
            node_id, ..
        } if node_id == "a"))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowNodeFinished {
            node_id, port, ..
        } if node_id == "a" && port == "done"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Done,
            ..
        }
    )));
    // A session-context node runs inside the walk's own running mark: the
    // session must stay running until the run ends, not drop between nodes.
    let marks: Vec<bool> = events
        .iter()
        .filter_map(|e| match e {
            Event::RunningChanged { is_running } => Some(*is_running),
            _ => None,
        })
        .collect();
    assert_eq!(marks, [true, false]);
}

#[test]
fn agent_input_stats_cover_the_calls_before_the_interruption() {
    // The agent asks a question (its turn ends), the human answers, the agent
    // finishes: the node's stats sum both segments' model calls.
    let call = ToolCall {
        id: "c1".into(),
        name: "ask_user".into(),
        arguments: serde_json::json!({"question": "Which color?"}),
        parse_error: None,
        raw_arguments: None,
    };
    let usage = |content: &str, calls: Vec<ToolCall>, prompt: u64, completion: u64| {
        Ok(Completion {
            content: content.into(),
            finish_reason: if calls.is_empty() {
                "stop"
            } else {
                "tool_calls"
            }
            .into(),
            tool_calls: calls,
            prompt_tokens: prompt,
            completion_tokens: completion,
            ..Completion::default()
        })
    };
    let model = Scripted(RefCell::new(vec![
        usage("", vec![call], 100, 5),
        usage("done", vec![], 200, 10),
    ]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: simple_workflow(),
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::AgentInput {
            prompt: "blue".into(),
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
}

fn criteria_workflow() -> Workflow {
    Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "plan".into(),
                name: Some("Plan".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "{{task}}".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "criteria".into(),
                name: Some("Criteria".into()),
                position: Position::default(),
                kind: NodeKind::Criteria {
                    plan: "plan".into(),
                },
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "plan".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "plan".into(),
                port: "done".into(),
                to: "criteria".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "criteria".into(),
                port: "pass".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "criteria".into(),
                port: "fail".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
        ],
    }
}

fn verdicts_workflow() -> Workflow {
    Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "plan".into(),
                name: Some("Plan".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "{{task}}".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "criteria".into(),
                name: Some("Criteria".into()),
                position: Position::default(),
                kind: NodeKind::Criteria {
                    plan: "plan".into(),
                },
            },
            Node {
                id: "verify".into(),
                name: Some("Verify".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "judge".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: true,
                },
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "plan".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "plan".into(),
                port: "done".into(),
                to: "criteria".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "criteria".into(),
                port: "pass".into(),
                to: "verify".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "criteria".into(),
                port: "fail".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "verify".into(),
                port: "done".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
        ],
    }
}

#[test]
fn criteria_node_runs_checks_and_reports_failures() {
    let model = Scripted(RefCell::new(vec![reply(
        "## Done when\n- C1 — check: `true`\n- C2 — check: `false`\n- C3",
        vec![],
    )]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    *commands.exit_codes.lock().unwrap() = vec![0, 1];
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: criteria_workflow(),
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::CriteriaSet { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::CriterionUpdated { status, .. }
                        if matches!(
                            status,
                            CriterionStatus::Passed {
                                provenance: Provenance::Verified { .. }
                            }
                                | CriterionStatus::Failed {
                                    provenance: Provenance::Verified { .. }
                                }
                        )
                )
            })
            .count(),
        2
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowNodeFinished {
            node_id,
            port,
            output,
            ..
        } if node_id == "criteria" && port == "fail" && output.contains("false")
    )));
}

#[test]
fn verdicts_record_indicative_only_for_unchecked_criteria() {
    let model = Scripted(RefCell::new(vec![
        reply("## Done when\n- first — check: `true`\n- second", vec![]),
        reply("C1: pass — checked\nC2: fail — not done", vec![]),
    ]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    *commands.exit_codes.lock().unwrap() = vec![0];
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: verdicts_workflow(),
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::CriterionUpdated {
            criterion_id,
            status
        } if criterion_id == "C2"
            && matches!(
                status,
                CriterionStatus::Failed {
                    provenance: Provenance::Indicative { agent, reason }
                } if agent == "plan" && reason == "not done"
            )
    )));
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::CriterionUpdated {
            criterion_id,
            status
        } if criterion_id == "C1"
            && matches!(
                status,
                CriterionStatus::Passed {
                    provenance: Provenance::Indicative { .. }
                }
            )
    )));
}

#[test]
fn shell_success_and_failure() {
    let workflow = Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "check".into(),
                name: Some("Check".into()),
                position: Position::default(),
                kind: NodeKind::Shell {
                    command: "echo {{task}}".into(),
                    timeout_secs: None,
                    success_codes: None,
                },
            },
            Node {
                id: "fix".into(),
                name: Some("Fix".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "fix it".into(),
                    retry_prompt: Some("retry: {{nodes.check.output}}".into()),
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "done".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
            Node {
                id: "stuck".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Blocked,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "check".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "check".into(),
                port: "success".into(),
                to: "done".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "check".into(),
                port: "failure".into(),
                to: "fix".into(),
                max_passes: Some(2),
                pause: false,
            },
            Edge {
                from: "check".into(),
                port: "exhausted".into(),
                to: "stuck".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "fix".into(),
                port: "done".into(),
                to: "check".into(),
                max_passes: None,
                pause: false,
            },
        ],
    };

    // Success case
    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands {
        exit_codes: Mutex::new(vec![0]),
    };
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: workflow.clone(),
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let events = h.log.0.into_inner();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Done,
            ..
        }
    )));

    // Failure loop case: fail twice, then exhausted
    let model = Scripted(RefCell::new(vec![
        reply("fix1", vec![]),
        reply("fix2", vec![]),
    ]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands {
        exit_codes: Mutex::new(vec![1, 1, 1]),
    };
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let events = h.log.0.into_inner();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Blocked,
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(e, Event::MessageStart {
            content: Some(content),
            is_system_generated: true,
            ..
        } if content.contains("retry:") && content.contains("output"))));
}

#[test]
fn human_node() {
    let workflow = approval_workflow(vec!["Go".into()]);

    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);
    // The human reads the question with its names filled in.
    assert!(
        h.log
            .0
            .borrow()
            .iter()
            .any(|e| matches!(e, Event::WorkflowHumanAsked {
            node_id, question, ..
        } if node_id == "approve" && question == "Go on go?"))
    );

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Answer {
            choice: "Go".into(),
            comment: "careful".into(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowNodeFinished {
            node_id, port, comment, ..
        } if node_id == "approve" && port == "Go" && comment.as_deref() == Some("careful")))
    );
    assert!(events.iter().any(|e| matches!(e, Event::MessageStart {
            content: Some(content),
            is_system_generated: true,
            ..
        } if content.ends_with("Human comment: careful"))));
}

fn approval_workflow(choices: Vec<String>) -> Workflow {
    let first = choices.first().expect("a choice");
    let mut edges = vec![
        Edge {
            from: "start".into(),
            port: "next".into(),
            to: "approve".into(),
            max_passes: None,
            pause: false,
        },
        Edge {
            from: "approve".into(),
            port: first.clone(),
            to: "a".into(),
            max_passes: None,
            pause: false,
        },
        Edge {
            from: "a".into(),
            port: "done".into(),
            to: "end".into(),
            max_passes: None,
            pause: false,
        },
    ];
    for choice in choices.iter().skip(1) {
        edges.push(Edge {
            from: "approve".into(),
            port: choice.clone(),
            to: "end".into(),
            max_passes: None,
            pause: false,
        });
    }
    Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "approve".into(),
                name: Some("Approve".into()),
                position: Position::default(),
                kind: NodeKind::Human {
                    question: "Go on {{task}}?".into(),
                    choices,
                },
            },
            Node {
                id: "a".into(),
                name: Some("Agent".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "do it".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges,
    }
}

#[test]
fn night_mode_answers_a_human_node_with_its_first_choice() {
    let workflow = approval_workflow(vec!["Go".into(), "Revise".into()]);

    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];
    let reason = run_workflow(
        &mut h.turn_autonomous(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let events = h.log.0.into_inner();
    assert!(events.iter().any(|event| {
        matches!(
            event,
            Event::WorkflowNodeFinished {
                node_id,
                port,
                comment,
                ..
            } if node_id == "approve" && port == "Go" && comment.is_none()
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            Event::DecisionRecorded {
                title,
                choice,
                alternatives,
                ..
            } if title.contains("Approve")
                && choice == "Go"
                && alternatives == &vec!["Revise".to_string()]
        )
    }));
}

#[test]
fn approving_the_plan_records_its_criteria_before_the_criteria_node() {
    let workflow = Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "plan".into(),
                name: Some("Plan".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "{{task}}".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "approve".into(),
                name: Some("Approve".into()),
                position: Position::default(),
                kind: NodeKind::Human {
                    question: "Go?".into(),
                    choices: vec!["Go".into(), "Revise".into()],
                },
            },
            Node {
                id: "criteria".into(),
                name: Some("Criteria".into()),
                position: Position::default(),
                kind: NodeKind::Criteria {
                    plan: "plan".into(),
                },
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "plan".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "plan".into(),
                port: "done".into(),
                to: "approve".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "approve".into(),
                port: "Go".into(),
                to: "criteria".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "approve".into(),
                port: "Revise".into(),
                to: "plan".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "criteria".into(),
                port: "pass".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "criteria".into(),
                port: "fail".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
        ],
    };

    let model = Scripted(RefCell::new(vec![
        reply("## Done when\n- C1 — check: `true`", vec![]),
        reply("## Done when\n- C2 — check: `true`", vec![]),
    ]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    *commands.exit_codes.lock().unwrap() = vec![0];
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);

    // "Revise" sends the plan back: nothing is recorded.
    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Answer {
            choice: "Revise".into(),
            comment: String::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);
    assert!(
        !h.log
            .0
            .borrow()
            .iter()
            .any(|e| matches!(e, Event::CriteriaSet { .. }))
    );

    // "Go" records the latest plan's criteria — once, before the criteria node.
    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Answer {
            choice: "Go".into(),
            comment: String::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    let set_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::CriteriaSet { .. }))
        .map(|(i, _)| i)
        .collect();
    let criteria_start = events
        .iter()
        .enumerate()
        .find(|(_, e)| {
            matches!(e, Event::WorkflowNodeStarted { node_id, .. } if node_id == "criteria")
        })
        .map(|(i, _)| i)
        .unwrap();
    assert_eq!(set_positions.len(), 1);
    assert!(set_positions[0] < criteria_start);
    let recorded = events
        .iter()
        .find_map(|e| match e {
            Event::CriteriaSet { criteria } => Some(criteria.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].description, "C2");
}

#[test]
fn paused_edge() {
    let mut workflow = simple_workflow();
    workflow.edges[1].pause = true;

    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);

    let events = h.log.0.borrow().clone();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowPaused { .. }))
    );

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Continue {
            comment: "ok".into(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
}

#[test]
fn agent_outcome() {
    let workflow = Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "a".into(),
                name: Some("Agent".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "do it".into(),
                    retry_prompt: None,
                    outputs: vec!["ok".into(), "ko".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "good".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
            Node {
                id: "bad".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Blocked,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "a".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "a".into(),
                port: "ok".into(),
                to: "good".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "a".into(),
                port: "ko".into(),
                to: "bad".into(),
                max_passes: None,
                pause: false,
            },
        ],
    };

    // Takes the ko edge
    let model = Scripted(RefCell::new(vec![reply("result\nOutcome: ko", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: workflow.clone(),
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let events = h.log.0.into_inner();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowNodeFinished {
            node_id, port, output, ..
        } if node_id == "a" && port == "ko" && output == "result"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Blocked,
            ..
        }
    )));

    // No outcome → blocked
    let model = Scripted(RefCell::new(vec![
        reply("no outcome here", vec![]),
        reply("still no outcome", vec![]),
    ]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let events = h.log.0.into_inner();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Blocked,
            ..
        }
    )));
}

#[test]
fn parallel_branches() {
    let workflow = Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "p".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Parallel,
            },
            Node {
                id: "a".into(),
                name: Some("Branch A".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "task a".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Fresh,
                    verdicts: false,
                },
            },
            Node {
                id: "b".into(),
                name: Some("Branch B".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "task b".into(),
                    retry_prompt: None,
                    outputs: vec!["done".into()],
                    context: AgentContext::Fresh,
                    verdicts: false,
                },
            },
            Node {
                id: "j".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Join,
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "p".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "p".into(),
                port: "branch".into(),
                to: "a".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "p".into(),
                port: "branch".into(),
                to: "b".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "a".into(),
                port: "done".into(),
                to: "j".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "b".into(),
                port: "done".into(),
                to: "j".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "j".into(),
                port: "next".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
        ],
    };

    let model = Scripted(RefCell::new(vec![reply("unused", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren {
        calls: Mutex::new(Vec::new()),
        replies: Mutex::new(vec!["result a".into(), "result b".into()]),
        ..FakeChildren::default()
    };
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let calls = children.calls.lock().unwrap();
    assert_eq!(calls.len(), 4); // 2 creates + 2 sends
    assert!(calls.iter().any(|c| c.starts_with("create:")));

    let events = h.log.0.into_inner();
    let join_output = events
        .iter()
        .find_map(|e| match e {
            Event::WorkflowNodeFinished {
                node_id, output, ..
            } if node_id == "j" => Some(output.clone()),
            _ => None,
        })
        .unwrap();
    assert!(join_output.contains("Branch A"));
    assert!(join_output.contains("result a"));
    assert!(join_output.contains("Branch B"));
    assert!(join_output.contains("result b"));
}

#[test]
fn stop_paused_run() {
    let mut workflow = simple_workflow();
    workflow.edges[1].pause = true;

    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);

    let reason = run_workflow(&mut h.turn(), &env, WorkflowJob::Stop).unwrap();
    assert_eq!(reason, DoneReason::Stopped);

    let events = h.log.0.into_inner();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Stopped,
            ..
        }
    )));
}

#[test]
fn invalid_workflow_refused() {
    let workflow = Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![Node {
            id: "a".into(),
            name: None,
            position: Position::default(),
            kind: NodeKind::Agent {
                agent: "unknown".into(),
                prompt: "p".into(),
                retry_prompt: None,
                outputs: vec!["done".into()],
                context: AgentContext::Session,
                verdicts: false,
            },
        }],
        edges: Vec::new(),
    };

    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let err = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap_err();
    assert!(!err.is_empty());

    let events = h.log.0.into_inner();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WorkflowStarted { .. }))
    );
}

#[test]
fn start_from_plan_records_the_plan_and_runs_the_criteria() {
    let model = Scripted(RefCell::new(vec![]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    *commands.exit_codes.lock().unwrap() = vec![0];
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::StartFromPlan {
            workflow: criteria_workflow(),
            task: "do it".into(),
            plan: "## Done when\n- C1 — check: `true`".into(),
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    // The session was empty: the plan lands in the thread for the builder.
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageStart { role: crate::domain::event::Role::User, content: Some(content), .. }
            if content.starts_with("do it") && content.contains("C1 — check")
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowNodeFinished {
            node_id,
            port,
            output,
            ..
        } if node_id == "plan"
            && port == "done"
            && output == "## Done when\n- C1 — check: `true`"
    )));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::CriteriaSet { .. }))
            .count(),
        1
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowNodeFinished {
            node_id,
            port,
            ..
        } if node_id == "criteria" && port == "pass"
    )));
}

#[test]
fn start_from_plan_answers_the_approval_node() {
    let mut workflow = criteria_workflow();
    workflow.nodes.push(Node {
        id: "approve".into(),
        name: Some("Approve".into()),
        position: Position::default(),
        kind: NodeKind::Human {
            question: "Go?".into(),
            choices: vec!["Go".into(), "Revise".into()],
        },
    });
    workflow
        .edges
        .retain(|edge| !(edge.from == "plan" && edge.port == "done"));
    workflow.edges.push(Edge {
        from: "plan".into(),
        port: "done".into(),
        to: "approve".into(),
        max_passes: None,
        pause: false,
    });
    workflow.edges.push(Edge {
        from: "approve".into(),
        port: "Go".into(),
        to: "criteria".into(),
        max_passes: None,
        pause: false,
    });
    workflow.edges.push(Edge {
        from: "approve".into(),
        port: "Revise".into(),
        to: "plan".into(),
        max_passes: None,
        pause: false,
    });

    let model = Scripted(RefCell::new(vec![]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    *commands.exit_codes.lock().unwrap() = vec![0];
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::StartFromPlan {
            workflow,
            task: "do it".into(),
            plan: "## Done when\n- C1 — check: `true`".into(),
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowNodeFinished {
            node_id,
            port,
            comment,
            ..
        } if node_id == "approve"
            && port == "Go"
            && comment.as_deref() == Some("approved from the plan card")
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowNodeFinished {
            node_id,
            port,
            ..
        } if node_id == "criteria" && port == "pass"
    )));
}

#[test]
fn start_with_attachments_puts_them_in_the_session_before_the_run() {
    use crate::domain::attachment::Attachment;
    use crate::domain::event::Role;

    let attachment = Attachment {
        id: "a".into(),
        filename: "a.png".into(),
        mime_type: "image/png".into(),
        size: 8,
        data: "data:image/png;base64,AAAA".into(),
    };
    let expected = vec![attachment.clone()];
    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: simple_workflow(),
            task: "do it".into(),
            step_by_step: false,
            attachments: expected.clone(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    let message_index = events
        .iter()
        .position(|e| {
            matches!(e, Event::MessageStart { role: Role::User, attachments, .. } if *attachments == expected)
        })
        .unwrap();
    let started_index = events
        .iter()
        .position(|e| matches!(e, Event::WorkflowStarted { .. }))
        .unwrap();
    assert!(message_index < started_index);
}

#[test]
fn relaunch_a_run_that_ended_in_error_reruns_the_open_node() {
    let workflow = simple_workflow();
    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    h.log.0.borrow_mut().extend([
        Event::WorkflowStarted {
            run_id: "r".into(),
            workflow: workflow.clone(),
            task: "do it".into(),
            step_by_step: false,
            started_at: None,
        },
        Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "start".into(),
            pass: 1,
            via: None,
            child_session_id: None,
            started_at: None,
        },
        Event::WorkflowNodeFinished {
            run_id: "r".into(),
            node_id: "start".into(),
            port: "next".into(),
            output: "do it".into(),
            exit_code: None,
            comment: None,
            duration_ms: None,
        },
        Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "a".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
            started_at: None,
        },
        Event::WorkflowFinished {
            run_id: "r".into(),
            outcome: RunOutcome::Error,
            reason: Some("boom".into()),
            stats: None,
        },
    ]);

    let reason = run_workflow(&mut h.turn(), &env, WorkflowJob::Resume).unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    let a_started = events
        .iter()
        .filter(|e| matches!(e, Event::WorkflowNodeStarted { node_id, .. } if node_id == "a"))
        .count();
    assert_eq!(a_started, 2);
    let finished = events
        .iter()
        .filter(|e| matches!(e, Event::WorkflowFinished { .. }))
        .count();
    assert_eq!(finished, 2);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Done,
            ..
        }
    )));
}

#[test]
fn relaunch_with_no_reopenable_node_is_refused() {
    let workflow = simple_workflow();
    let model = Scripted(RefCell::new(vec![]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    h.log.0.borrow_mut().extend([
        Event::WorkflowStarted {
            run_id: "r".into(),
            workflow,
            task: "do it".into(),
            step_by_step: false,
            started_at: None,
        },
        Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "start".into(),
            pass: 1,
            via: None,
            child_session_id: None,
            started_at: None,
        },
        Event::WorkflowNodeFinished {
            run_id: "r".into(),
            node_id: "start".into(),
            port: "next".into(),
            output: "do it".into(),
            exit_code: None,
            comment: None,
            duration_ms: None,
        },
        Event::WorkflowFinished {
            run_id: "r".into(),
            outcome: RunOutcome::Error,
            reason: Some("boom".into()),
            stats: None,
        },
    ]);

    let err = run_workflow(&mut h.turn(), &env, WorkflowJob::Resume).unwrap_err();
    assert!(err.contains("no node can be re-run"));
}

#[test]
fn resume_interrupted_agent_keeps_its_pass_and_prompt() {
    let workflow = Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "a".into(),
                name: Some("Agent".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "first prompt".into(),
                    retry_prompt: Some("retry prompt".into()),
                    outputs: vec!["done".into()],
                    context: AgentContext::Session,
                    verdicts: false,
                },
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "a".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "a".into(),
                port: "done".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            },
        ],
    };

    let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    h.log.0.borrow_mut().extend([
        Event::WorkflowStarted {
            run_id: "r".into(),
            workflow,
            task: "do it".into(),
            step_by_step: false,
            started_at: None,
        },
        Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "start".into(),
            pass: 1,
            via: None,
            child_session_id: None,
            started_at: None,
        },
        Event::WorkflowNodeFinished {
            run_id: "r".into(),
            node_id: "start".into(),
            port: "next".into(),
            output: "do it".into(),
            exit_code: None,
            comment: None,
            duration_ms: None,
        },
        Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "a".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
            started_at: None,
        },
        // The model call the interrupted first segment made.
        Event::MessageDone {
            message_id: "m1".into(),
            stats: Some(crate::domain::stats::MessageStats {
                model: "scripted".into(),
                total_time: 0.0,
                prefill_tokens: 100,
                cached_prompt_tokens: 0,
                generation_tokens: 5,
                prefill_speed: 0.0,
                generation_speed: 0.0,
            }),
            partial: false,
        },
    ]);

    let reason = run_workflow(&mut h.turn(), &env, WorkflowJob::Resume).unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    let started = events
        .iter()
        .filter(|e| matches!(e, Event::WorkflowNodeStarted { node_id, .. } if node_id == "a"))
        .collect::<Vec<_>>();
    assert_eq!(started.len(), 2);
    assert!(matches!(
        started.last().unwrap(),
        Event::WorkflowNodeStarted { pass, .. } if *pass == 1
    ));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageStart {
            content: Some(content),
            is_system_generated: true,
            ..
        } if content.contains("first prompt") && !content.contains("retry prompt")
    )));
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::MessageStart {
            content: Some(content),
            is_system_generated: true,
            ..
        } if content.contains("retry prompt")
    )));
}

// The test gate: validating the tests records them, reworking records nothing.
fn test_gate_workflow() -> Workflow {
    serde_json::from_value(serde_json::json!({
        "id": "wf",
        "name": "wf",
        "description": "",
        "maxSteps": 10,
        "nodes": [
            { "id": "start", "kind": "start", "position": { "x": 0, "y": 0 } },
            { "id": "tests", "kind": "agent", "name": "Tests", "agent": "tests",
              "prompt": "write tests", "position": { "x": 0, "y": 140 } },
            { "id": "approve-tests", "kind": "human", "name": "Validate",
              "question": "validate?", "choices": ["Validate", "Rework"],
              "position": { "x": 0, "y": 280 } }
        ],
        "edges": [
            { "from": "start", "port": "next", "to": "tests" },
            { "from": "tests", "port": "done", "to": "approve-tests" }
        ]
    }))
    .unwrap()
}

fn events_with_open_tests_gate(run_id: &str) -> Vec<Event> {
    vec![
        Event::WorkflowStarted {
            run_id: run_id.into(),
            workflow: test_gate_workflow(),
            task: "task".into(),
            step_by_step: false,
            started_at: None,
        },
        Event::WorkflowNodeStarted {
            run_id: run_id.into(),
            node_id: "start".into(),
            pass: 1,
            via: None,
            child_session_id: None,
            started_at: None,
        },
        Event::WorkflowNodeFinished {
            run_id: run_id.into(),
            node_id: "start".into(),
            port: "next".into(),
            output: "task".into(),
            exit_code: None,
            comment: None,
            duration_ms: None,
        },
        Event::WorkflowNodeStarted {
            run_id: run_id.into(),
            node_id: "tests".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
            started_at: None,
        },
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: "c1".into(),
                name: "write_file".into(),
                arguments: serde_json::json!({"path": "tests/a.rs"}),
                parse_error: None,
                raw_arguments: None,
            },
        },
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: "c1".into(),
            result: ToolResult::success("ok".into(), false),
        },
        Event::WorkflowNodeFinished {
            run_id: run_id.into(),
            node_id: "tests".into(),
            port: "done".into(),
            output: "## Tests".into(),
            exit_code: None,
            comment: None,
            duration_ms: None,
        },
        Event::WorkflowNodeStarted {
            run_id: run_id.into(),
            node_id: "approve-tests".into(),
            pass: 1,
            via: Some(Via {
                from: "tests".into(),
                port: "done".into(),
            }),
            child_session_id: None,
            started_at: None,
        },
    ]
}

fn answer_test_gate(choice: &str) -> Vec<Event> {
    let stop = AtomicBool::new(false);
    let mut h = Harness::new(
        Box::new(Scripted(RefCell::new(vec![]))),
        PathBuf::from("/tmp"),
        &stop,
    );
    for event in events_with_open_tests_gate("r") {
        EventLog::append(&h.log, "s", &event).unwrap();
    }
    super::jobs::answer_human(&mut h.turn(), choice, "").unwrap();
    EventLog::events(&h.log, "s").unwrap()
}

#[test]
fn validating_the_tests_records_the_written_test_files() {
    let events = answer_test_gate("Validate");
    assert!(
        events.iter().any(|event| {
            matches!(
                event,
                Event::TestsApproved { files, .. }
                    if files == &vec!["tests/a.rs".to_string()]
            )
        }),
        "{events:?}"
    );
}

#[test]
fn night_mode_validates_the_tests_like_a_click() {
    let stop = AtomicBool::new(false);
    let mut h = Harness::new(
        Box::new(Scripted(RefCell::new(vec![]))),
        PathBuf::from("/tmp"),
        &stop,
    );
    for event in events_with_open_tests_gate("r") {
        EventLog::append(&h.log, "s", &event).unwrap();
    }
    let choices = ["Validate".to_string(), "Rework".to_string()];
    let node = Node {
        id: "approve-tests".into(),
        name: Some("Validate the tests".into()),
        position: Position::default(),
        kind: NodeKind::Human {
            question: "Validate?".into(),
            choices: choices.to_vec(),
        },
    };
    super::jobs::answer_by_night(&mut h.turn_autonomous(), &node, &choices).unwrap();

    let events = EventLog::events(&h.log, "s").unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::DecisionRecorded { choice, .. } if choice == "Validate"
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::TestsApproved { .. }))
    );
}

#[test]
fn reworking_the_tests_records_nothing() {
    let events = answer_test_gate("Rework");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::TestsApproved { .. }))
    );
}

#[test]
fn approval_in_a_session_agent_node_finishes_the_node_and_walks_on() {
    let id = crate::domain::id::new_id();
    let workdir = std::env::temp_dir().join(format!("fennec-ap-work-{id}"));
    let outside = std::env::temp_dir().join(format!("fennec-ap-outside-{id}"));
    std::fs::create_dir_all(&workdir).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("note.txt"), "outside text").unwrap();

    let call = ToolCall {
        id: "c1".into(),
        name: "read_file".into(),
        arguments: serde_json::json!({
            "path": outside.join("note.txt").to_string_lossy()
        }),
        parse_error: None,
        raw_arguments: None,
    };
    let model = Scripted(RefCell::new(vec![
        reply("", vec![call]),
        reply("done", vec![]),
    ]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), workdir.clone(), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: simple_workflow(),
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);
    let events = h.log.0.borrow().clone();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::RuleApprovalRequested { call_id, .. } if call_id == "c1"))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowNodeStarted { node_id, .. } if node_id == "a"))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WorkflowNodeFinished { node_id, .. } if node_id == "a"))
    );

    let reason = answer_in_node(&mut h.turn(), &env, |turn| {
        crate::agent::rule_approval::answer_approval(
            turn,
            "c1",
            crate::domain::rule_approval::Choice::Once,
            None,
        )
    })
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorkflowNodeFinished { node_id, port, .. } if node_id == "a" && port == "done"))
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::WorkflowFinished {
            outcome: RunOutcome::Done,
            ..
        }
    )));
    // A finished run waits for nothing: no resume is asked.
    let state = crate::domain::workflow::run::current_run(&events).unwrap();
    assert_eq!(crate::domain::workflow::run::waiting(&state, &events), None);

    let _ = std::fs::remove_dir_all(&workdir);
    let _ = std::fs::remove_dir_all(&outside);
}

fn fresh_workflow(outputs: Vec<String>, loops: bool) -> Workflow {
    let mut edges = vec![
        Edge {
            from: "start".into(),
            port: "next".into(),
            to: "verify".into(),
            max_passes: None,
            pause: false,
        },
        Edge {
            from: "verify".into(),
            port: outputs[0].clone(),
            to: "end".into(),
            max_passes: None,
            pause: false,
        },
    ];
    if loops {
        edges.push(Edge {
            from: "verify".into(),
            port: outputs[1].clone(),
            to: "verify".into(),
            max_passes: Some(3),
            pause: false,
        });
    }
    Workflow {
        id: "wf".into(),
        name: "wf".into(),
        description: String::new(),
        max_steps: 10,
        nodes: vec![
            Node {
                id: "start".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::Start,
            },
            Node {
                id: "verify".into(),
                name: Some("Verify".into()),
                position: Position::default(),
                kind: NodeKind::Agent {
                    agent: "plan".into(),
                    prompt: "verify".into(),
                    retry_prompt: None,
                    outputs,
                    context: AgentContext::Fresh,
                    verdicts: false,
                },
            },
            Node {
                id: "end".into(),
                name: None,
                position: Position::default(),
                kind: NodeKind::End {
                    outcome: EndOutcome::Done,
                },
            },
        ],
        edges,
    }
}

#[test]
fn a_fresh_child_that_asks_keeps_the_run_open_and_resumes_on_the_same_child() {
    let stop = AtomicBool::new(false);
    let model = Scripted(RefCell::new(vec![reply("unused", vec![])]));
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];
    let children = FakeChildren {
        replies: Mutex::new(vec!["asked".into()]),
        reasons: Mutex::new(vec![DoneReason::WaitingForUser]),
        last_runs: Mutex::new(vec![AgentRun {
            reason: DoneReason::Complete,
            text: "verified".into(),
        }]),
        ..FakeChildren::default()
    };
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let workflow = fresh_workflow(vec!["ok".into()], false);
    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: workflow.clone(),
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);
    let events = h.log.0.borrow().clone();
    let state = run::current_run(&events).unwrap();
    assert!(state.finished.is_none());
    let open = state
        .open
        .iter()
        .find(|open| open.node_id == "verify")
        .unwrap();
    let child_id = open.child_session_id.clone().unwrap();
    assert_eq!(
        run::waiting(&state, &events),
        Some(run::Waiting::ChildQuestion)
    );
    assert!(run::waits_on_child(&events, &child_id));

    let reason = run_workflow(&mut h.turn(), &env, WorkflowJob::Resume).unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let calls = children.calls.lock().unwrap();
    assert!(calls.contains(&format!("last_run:{child_id}")));
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.starts_with("create:"))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.starts_with("send:"))
            .count(),
        1
    );
    let events = h.log.0.borrow().clone();
    let finished = events
        .iter()
        .find(|event| matches!(event, Event::WorkflowNodeFinished { node_id, .. } if node_id == "verify"))
        .unwrap();
    let Event::WorkflowNodeFinished { output, .. } = finished else {
        panic!("expected node finished");
    };
    assert!(output.contains("verified"));
}

#[test]
fn a_new_pass_of_a_fresh_node_gets_a_new_child() {
    let stop = AtomicBool::new(false);
    let model = Scripted(RefCell::new(vec![reply("unused", vec![])]));
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];
    let children = FakeChildren {
        replies: Mutex::new(vec![
            "work\nOutcome: done".into(),
            "work\nOutcome: again".into(),
        ]),
        ..FakeChildren::default()
    };
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let workflow = fresh_workflow(vec!["done".into(), "again".into()], true);
    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow,
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let calls = children.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.starts_with("create:"))
            .count(),
        2
    );
    assert!(!calls.iter().any(|call| call.starts_with("last_run:")));
}

#[test]
fn a_failed_fresh_child_is_not_reused_on_resume() {
    let stop = AtomicBool::new(false);
    let model = Scripted(RefCell::new(vec![reply("unused", vec![])]));
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];
    let children = FakeChildren {
        replies: Mutex::new(vec!["fixed".into(), "broken".into()]),
        reasons: Mutex::new(vec![DoneReason::Complete, DoneReason::Error]),
        last_runs: Mutex::new(vec![AgentRun {
            reason: DoneReason::Error,
            text: String::new(),
        }]),
        ..FakeChildren::default()
    };
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let workflow = fresh_workflow(vec!["ok".into()], false);
    run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: workflow.clone(),
            task: "go".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    let reason = run_workflow(&mut h.turn(), &env, WorkflowJob::Resume).unwrap();
    assert_eq!(reason, DoneReason::Complete);
    let calls = children.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.starts_with("create:"))
            .count(),
        2
    );
    let events = h.log.0.borrow().clone();
    let finished = events
        .iter()
        .find(|event| matches!(event, Event::WorkflowNodeFinished { node_id, .. } if node_id == "verify"))
        .unwrap();
    let Event::WorkflowNodeFinished { port, .. } = finished else {
        panic!("expected node finished");
    };
    assert_eq!(port, "ok");
}
