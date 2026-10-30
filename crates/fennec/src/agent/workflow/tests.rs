use super::*;
use crate::agent::ports::{AgentRun, ChildSessions, CommandRun, Commands, Completion, Model};
use crate::agent::test_support::{MemoryLog, Scripted, builder, plan, reply};
use crate::domain::agent::AgentDefinition;
use crate::domain::criteria::{CriterionStatus, Provenance};
use crate::domain::event::{DoneReason, Event, ToolCall};
use crate::domain::workflow::definition::{
    AgentContext, Edge, EndOutcome, Node, NodeKind, Position, Workflow,
};
use crate::domain::workflow::run::{RunOutcome, Via};
use crate::tools::LocalTools;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

struct Harness<'a> {
    log: MemoryLog,
    model: Box<dyn Model>,
    tools: LocalTools,
    stop: &'a AtomicBool,
    workdir: PathBuf,
    on_event: Box<dyn FnMut(&Event) + 'a>,
    agents: Vec<AgentDefinition>,
}

impl<'a> Harness<'a> {
    fn new(model: Box<dyn Model>, workdir: PathBuf, stop: &'a AtomicBool) -> Self {
        Harness {
            log: MemoryLog::default(),
            model,
            tools: LocalTools::new(workdir.clone()),
            stop,
            workdir,
            on_event: Box::new(|_: &Event| {}),
            agents: builder(),
        }
    }

    fn turn(&mut self) -> Turn<'_> {
        Turn {
            model: &*self.model,
            log: &self.log,
            tools: &self.tools,
            session_id: "s",
            workdir: &self.workdir,
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &self.agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            autonomous: false,
            stop: self.stop,
            on_event: &mut self.on_event,
        }
    }
}

#[derive(Default)]
struct FakeChildren {
    calls: Mutex<Vec<String>>,
    replies: Mutex<Vec<String>>,
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
        Ok(AgentRun {
            reason: DoneReason::Complete,
            text,
        })
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
                id: "approve".into(),
                name: Some("Approve".into()),
                position: Position::default(),
                kind: NodeKind::Human {
                    question: "Go on {{task}}?".into(),
                    choices: vec!["Go".into()],
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
        edges: vec![
            Edge {
                from: "start".into(),
                port: "next".into(),
                to: "approve".into(),
                max_passes: None,
                pause: false,
            },
            Edge {
                from: "approve".into(),
                port: "Go".into(),
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
