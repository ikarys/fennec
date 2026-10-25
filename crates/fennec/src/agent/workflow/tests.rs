use super::*;
use crate::agent::ports::{AgentRun, ChildSessions, CommandRun, Commands, Model};
use crate::agent::test_support::{MemoryLog, Scripted, builder, plan, reply};
use crate::domain::agent::AgentDefinition;
use crate::domain::event::{DoneReason, Event};
use crate::domain::workflow::definition::{
    AgentContext, Edge, EndOutcome, Node, NodeKind, Position, Workflow,
};
use crate::domain::workflow::run::RunOutcome;
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
            agents: &self.agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
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
        let code = self
            .exit_codes
            .lock()
            .unwrap()
            .first()
            .copied()
            .unwrap_or(0);
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
                    question: "Go?".into(),
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
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::WaitingForUser);

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
