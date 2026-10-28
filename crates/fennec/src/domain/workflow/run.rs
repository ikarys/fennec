//! The run state folded from workflow events and the deterministic next step.

use super::definition::{BRANCH, ConditionTest, EXHAUSTED, EndOutcome, Node, NodeKind, Workflow};
use super::template;
use crate::domain::criteria::{Criterion, CriterionState, CriterionStatus, render_for_prompt};
use crate::domain::event::{DoneReason, Event};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunOutcome {
    Done,
    Blocked,
    Stopped,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Via {
    pub from: String,
    pub port: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeResult {
    pub port: String,
    pub output: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pause {
    pub from: String,
    pub port: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenNode {
    pub node_id: String,
    pub pass: u32,
    pub child_session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunState {
    pub run_id: String,
    pub workflow: Workflow,
    pub task: String,
    pub step_by_step: bool,
    pub results: HashMap<String, NodeResult>,
    pub node_passes: HashMap<String, u32>,
    pub edge_passes: HashMap<(String, String), u32>,
    pub steps: u32,
    pub open: Vec<OpenNode>,
    /// The nodes a resume found open (interrupted): their next start re-runs
    /// the same pass and is not a new pass on the edge.
    pub interrupted: Vec<OpenNode>,
    pub paused: Option<Pause>,
    pub comment: String,
    pub finished: Option<(RunOutcome, Option<String>)>,
    pub criteria: Vec<CriterionState>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Next {
    Run {
        node_id: String,
        via: Via,
    },
    Pause(Pause),
    Fork {
        parallel: String,
        entries: Vec<(Via, String)>,
        join: String,
    },
    ReachJoin {
        join: String,
    },
    Finish {
        outcome: RunOutcome,
        reason: Option<String>,
    },
}

impl RunState {
    pub fn new(run_id: String, workflow: Workflow, task: String, step_by_step: bool) -> RunState {
        RunState {
            run_id,
            workflow,
            task,
            step_by_step,
            results: HashMap::new(),
            node_passes: HashMap::new(),
            edge_passes: HashMap::new(),
            steps: 0,
            open: Vec::new(),
            interrupted: Vec::new(),
            paused: None,
            comment: String::new(),
            finished: None,
            criteria: Vec::new(),
        }
    }

    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::WorkflowNodeStarted {
                run_id,
                node_id,
                pass,
                via,
                child_session_id,
            } if run_id == &self.run_id => {
                let resumed = self
                    .interrupted
                    .iter()
                    .position(|open| open.node_id == *node_id && open.pass == *pass);
                if let Some(index) = resumed {
                    // A resume re-enters the node on the pass it was interrupted on.
                    self.interrupted.remove(index);
                } else {
                    self.steps += 1;
                    if let Some(via) = via {
                        let key = (via.from.clone(), via.port.clone());
                        *self.edge_passes.entry(key).or_insert(0) += 1;
                    }
                }
                self.node_passes.insert(node_id.clone(), *pass);
                self.open.push(OpenNode {
                    node_id: node_id.clone(),
                    pass: *pass,
                    child_session_id: child_session_id.clone(),
                });
                self.paused = None;
            }
            Event::WorkflowNodeFinished {
                run_id,
                node_id,
                port,
                output,
                exit_code,
                comment,
                ..
            } if run_id == &self.run_id => {
                self.open.retain(|open| &open.node_id != node_id);
                self.results.insert(
                    node_id.clone(),
                    NodeResult {
                        port: port.clone(),
                        output: output.clone(),
                        exit_code: *exit_code,
                    },
                );

                if let Some(comment) = comment
                    && !comment.is_empty()
                {
                    self.comment = comment.clone();
                }
            }
            Event::WorkflowPaused {
                run_id,
                from,
                port,
                to,
            } if run_id == &self.run_id => {
                self.paused = Some(Pause {
                    from: from.clone(),
                    port: port.clone(),
                    to: to.clone(),
                });
            }
            Event::WorkflowResumed { run_id, comment } if run_id == &self.run_id => {
                // A resumed interrupted run starts its node afresh (a pause has no
                // open node): remembered so that start keeps the node's pass.
                self.interrupted = std::mem::take(&mut self.open);
                self.paused = None;
                if !comment.is_empty() {
                    self.comment = comment.clone();
                }
            }
            Event::WorkflowFinished {
                run_id,
                outcome,
                reason,
                ..
            } if run_id == &self.run_id => {
                self.finished = Some((*outcome, reason.clone()));
            }
            Event::CriteriaSet { criteria } => {
                self.criteria = criteria
                    .iter()
                    .map(|criterion| CriterionState {
                        criterion: criterion.clone(),
                        status: CriterionStatus::Pending,
                    })
                    .collect();
            }
            Event::CriterionUpdated {
                criterion_id,
                status,
            } => {
                if let Some(state) = self
                    .criteria
                    .iter_mut()
                    .find(|state| state.criterion.id == *criterion_id)
                {
                    state.status = status.clone();
                }
            }
            _ => {}
        }
    }

    pub fn is_active(&self) -> bool {
        self.finished.is_none()
    }
}

/// The current run's criteria, without their status: the contract a
/// proposal is compared with; none outside a run.
pub fn criteria_in_force(events: &[Event]) -> Vec<Criterion> {
    current_run(events)
        .map(|state| state.criteria.into_iter().map(|c| c.criterion).collect())
        .unwrap_or_default()
}

/// The log from the latest `workflow.started` on: what the current run did,
/// so `{{filesChanged}}` does not list an earlier run's files.
pub fn current_run_events(events: &[Event]) -> &[Event] {
    let start = events
        .iter()
        .rposition(|event| matches!(event, Event::WorkflowStarted { .. }))
        .unwrap_or(0);
    &events[start..]
}

pub fn current_run(events: &[Event]) -> Option<RunState> {
    let start_index = events
        .iter()
        .rposition(|event| matches!(event, Event::WorkflowStarted { .. }))?;

    let Event::WorkflowStarted {
        run_id,
        workflow,
        task,
        step_by_step,
    } = &events[start_index]
    else {
        return None;
    };

    let mut state = RunState::new(
        run_id.clone(),
        workflow.clone(),
        task.clone(),
        *step_by_step,
    );

    for event in &events[start_index + 1..] {
        state.apply(event);
    }

    Some(state)
}

/// Where an active run waits for the human.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waiting {
    /// A human node is open: the human picks a choice.
    Human,
    /// A paused edge: the human continues or stops.
    Pause,
    /// An agent node is open and its turn ended waiting for the user.
    AgentQuestion,
    /// The run is not running (interrupted): the human resumes or stops.
    Interrupted,
}

/// Where `state` waits, `None` when it is not active.
pub fn waiting(state: &RunState, events: &[Event]) -> Option<Waiting> {
    if !state.is_active() {
        return None;
    }
    if state.paused.is_some() {
        return Some(Waiting::Pause);
    }
    let open = |is: fn(&NodeKind) -> bool| {
        state.open.iter().any(|open| {
            state
                .workflow
                .node(&open.node_id)
                .is_some_and(|n| is(&n.kind))
        })
    };
    if open(NodeKind::is_human) {
        return Some(Waiting::Human);
    }
    let last_done = events.iter().rev().find_map(|event| match event {
        Event::ChatDone { reason, .. } => Some(*reason),
        _ => None,
    });
    if open(|kind| matches!(kind, NodeKind::Agent { .. }))
        && last_done == Some(DoneReason::WaitingForUser)
    {
        return Some(Waiting::AgentQuestion);
    }
    Some(Waiting::Interrupted)
}

pub fn next_after(state: &RunState, from: &str, port: &str, in_branch: bool) -> Next {
    if state.steps >= state.workflow.max_steps {
        return Next::Finish {
            outcome: RunOutcome::Blocked,
            reason: Some(format!(
                "the run reached maxSteps ({})",
                state.workflow.max_steps
            )),
        };
    }

    let Some(edge) = state.workflow.edge(from, port) else {
        return Next::Finish {
            outcome: RunOutcome::Blocked,
            reason: Some(format!("{from}: port {port} is not wired")),
        };
    };

    // A capped edge that reached its cap gives way to the `exhausted` port,
    // which is then the edge taken (and counted).
    let (edge, port) = match edge.max_passes {
        Some(max_passes)
            if state
                .edge_passes
                .get(&(from.to_string(), port.to_string()))
                .copied()
                .unwrap_or(0)
                >= max_passes =>
        {
            match state.workflow.edge(from, EXHAUSTED) {
                Some(exhausted) => (exhausted, EXHAUSTED),
                None => {
                    return Next::Finish {
                        outcome: RunOutcome::Blocked,
                        reason: Some(format!(
                            "{from} → {}: maxPasses ({max_passes}) reached",
                            edge.to
                        )),
                    };
                }
            }
        }
        _ => (edge, port),
    };

    if !in_branch && (edge.pause || state.step_by_step) {
        return Next::Pause(Pause {
            from: from.to_string(),
            port: port.to_string(),
            to: edge.to.clone(),
        });
    }

    enter(
        state,
        Via {
            from: from.to_string(),
            port: port.to_string(),
        },
        &edge.to,
        in_branch,
    )
}

pub fn enter(state: &RunState, via: Via, to: &str, in_branch: bool) -> Next {
    let Some(node) = state.workflow.node(to) else {
        return Next::Finish {
            outcome: RunOutcome::Error,
            reason: Some(format!("{to}: unknown node")),
        };
    };

    match &node.kind {
        NodeKind::End { outcome } => Next::Finish {
            outcome: match outcome {
                EndOutcome::Done => RunOutcome::Done,
                EndOutcome::Blocked => RunOutcome::Blocked,
            },
            reason: match outcome {
                EndOutcome::Done => None,
                EndOutcome::Blocked => Some(format!("reached the end \"{}\"", node.label())),
            },
        },
        NodeKind::Parallel => {
            let entries = state
                .workflow
                .edges_from(to, BRANCH)
                .map(|edge| {
                    (
                        Via {
                            from: to.to_string(),
                            port: BRANCH.to_string(),
                        },
                        edge.to.clone(),
                    )
                })
                .collect();

            match state.workflow.join_of(to) {
                Some(join) => Next::Fork {
                    parallel: to.to_string(),
                    entries,
                    join,
                },
                None => Next::Finish {
                    outcome: RunOutcome::Error,
                    reason: Some(format!("{to}: no join")),
                },
            }
        }
        NodeKind::Join => {
            if in_branch {
                Next::ReachJoin {
                    join: to.to_string(),
                }
            } else {
                Next::Finish {
                    outcome: RunOutcome::Error,
                    reason: Some(format!("{to}: join reached outside a branch")),
                }
            }
        }
        _ => Next::Run {
            node_id: to.to_string(),
            via,
        },
    }
}

pub fn variable(
    state: &RunState,
    node_id: &str,
    files_changed: &[String],
    name: &str,
) -> Option<String> {
    match name {
        "task" => Some(state.task.clone()),
        "filesChanged" => Some(files_changed.join("\n")),
        "pass" => state.node_passes.get(node_id).map(|pass| pass.to_string()),
        "comment" => Some(state.comment.clone()),
        "criteria" => Some(render_for_prompt(&state.criteria)),
        _ if name.starts_with("nodes.") => {
            let rest = &name["nodes.".len()..];
            let (id, field) = rest.split_once('.')?;
            let node = state.workflow.node(id)?;
            let result = state.results.get(id)?;

            match field {
                "output" => Some(result.output.clone()),
                "port" => Some(result.port.clone()),
                "exitCode" => {
                    if matches!(node.kind, NodeKind::Shell { .. }) {
                        result.exit_code.map(|code| code.to_string())
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn agent_prompt(state: &RunState, node: &Node, pass: u32, files_changed: &[String]) -> String {
    let NodeKind::Agent {
        prompt,
        retry_prompt,
        ..
    } = &node.kind
    else {
        return String::new();
    };

    let base = if pass >= 2 {
        retry_prompt.clone().unwrap_or_else(|| prompt.clone())
    } else {
        prompt.clone()
    };

    let rendered = template::render(
        &base,
        // The node being prompted is on this pass, whether or not its start
        // is folded yet.
        &|name| match name {
            "pass" => Some(pass.to_string()),
            _ => variable(state, &node.id, files_changed, name),
        },
        false,
    );

    if state.comment.is_empty() {
        rendered
    } else {
        format!("{rendered}\n\nHuman comment: {}", state.comment)
    }
}

pub fn evaluate(test: ConditionTest, value: &str, against: Option<&str>) -> bool {
    match test {
        crate::domain::workflow::definition::ConditionTest::Equals => {
            against.is_some_and(|against| value.trim() == against.trim())
        }
        crate::domain::workflow::definition::ConditionTest::Contains => {
            against.is_some_and(|against| value.contains(against))
        }
        crate::domain::workflow::definition::ConditionTest::Empty => value.trim().is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workflow::definition::{
        AgentContext, Edge, EndOutcome, Node, NodeKind, Position, Workflow,
    };

    fn simple_workflow() -> Workflow {
        Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 2,
            nodes: vec![
                Node {
                    id: "start".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Start,
                },
                Node {
                    id: "a".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Agent {
                        agent: "plan".into(),
                        prompt: "{{task}}".into(),
                        retry_prompt: Some("retry {{pass}}".into()),
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

    fn state(workflow: Workflow) -> RunState {
        RunState::new("r".into(), workflow, "task".into(), false)
    }

    #[test]
    fn resumed_clears_open_nodes_and_remembers_them() {
        let mut state = state(simple_workflow());
        state.apply(&Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "a".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
        });
        assert_eq!(state.open.len(), 1);
        state.apply(&Event::WorkflowResumed {
            run_id: "r".into(),
            comment: String::new(),
        });
        assert!(state.open.is_empty());
        assert_eq!(state.interrupted.len(), 1);
        assert!(state.paused.is_none());
    }

    #[test]
    fn resuming_an_interrupted_node_keeps_its_pass() {
        let mut state = state(simple_workflow());
        let started = || Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "a".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
        };
        state.apply(&started());
        state.apply(&Event::WorkflowResumed {
            run_id: "r".into(),
            comment: String::new(),
        });
        state.apply(&started());

        assert_eq!(state.open.len(), 1);
        assert!(state.interrupted.is_empty());
        assert_eq!(state.node_passes.get("a"), Some(&1));
        assert_eq!(state.steps, 1);
        assert_eq!(
            state
                .edge_passes
                .get(&("start".to_string(), "next".to_string())),
            Some(&1)
        );
    }

    #[test]
    fn apply_folds_a_small_run() {
        let mut state = state(simple_workflow());

        state.apply(&Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "a".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
        });
        state.apply(&Event::WorkflowNodeFinished {
            run_id: "r".into(),
            node_id: "a".into(),
            port: "done".into(),
            output: "out".into(),
            exit_code: None,
            comment: None,
            duration_ms: None,
            stats: None,
        });
        state.apply(&Event::WorkflowPaused {
            run_id: "r".into(),
            from: "a".into(),
            port: "done".into(),
            to: "end".into(),
        });
        state.apply(&Event::WorkflowResumed {
            run_id: "r".into(),
            comment: "note".into(),
        });
        state.apply(&Event::WorkflowFinished {
            run_id: "r".into(),
            outcome: RunOutcome::Done,
            reason: None,
            stats: None,
        });

        assert_eq!(state.steps, 1);
        assert_eq!(state.node_passes.get("a"), Some(&1));
        assert_eq!(
            state
                .edge_passes
                .get(&("start".to_string(), "next".to_string())),
            Some(&1)
        );
        assert_eq!(
            state.results.get("a"),
            Some(&NodeResult {
                port: "done".into(),
                output: "out".into(),
                exit_code: None,
            })
        );
        assert_eq!(state.comment, "note");
        assert_eq!(state.finished, Some((RunOutcome::Done, None)));
        assert!(state.open.is_empty());
        assert!(state.paused.is_none());
        assert!(!state.is_active());
    }

    #[test]
    fn apply_folds_criteria_events() {
        let mut state = state(simple_workflow());

        state.apply(&Event::CriteriaSet {
            criteria: vec![crate::domain::criteria::Criterion {
                id: "C1".into(),
                description: "d".into(),
                check: None,
            }],
        });
        assert_eq!(state.criteria.len(), 1);
        assert!(matches!(state.criteria[0].status, CriterionStatus::Pending));

        state.apply(&Event::CriterionUpdated {
            criterion_id: "C1".into(),
            status: CriterionStatus::Passed {
                provenance: crate::domain::criteria::Provenance::Indicative {
                    agent: "a".into(),
                    reason: "r".into(),
                },
            },
        });
        assert!(matches!(
            state.criteria[0].status,
            CriterionStatus::Passed { .. }
        ));
    }

    #[test]
    fn variable_renders_criteria() {
        let mut state = state(simple_workflow());
        state.apply(&Event::CriteriaSet {
            criteria: vec![crate::domain::criteria::Criterion {
                id: "C1".into(),
                description: "d".into(),
                check: None,
            }],
        });

        assert_eq!(
            variable(&state, "a", &[], "criteria"),
            Some("C1: d — no check, to judge: pending".into())
        );
    }

    #[test]
    fn current_run_events_start_at_the_last_run() {
        let started = |run_id: &str| Event::WorkflowStarted {
            run_id: run_id.into(),
            workflow: simple_workflow(),
            task: String::new(),
            step_by_step: false,
        };
        let events = vec![
            started("old"),
            Event::RunningChanged { is_running: true },
            started("new"),
        ];
        assert_eq!(current_run_events(&events).len(), 1);
        assert_eq!(current_run_events(&events[..2]).len(), 2);
    }

    #[test]
    fn current_run_picks_the_last_run() {
        let events = vec![
            Event::WorkflowStarted {
                run_id: "old".into(),
                workflow: simple_workflow(),
                task: "old".into(),
                step_by_step: false,
            },
            Event::WorkflowStarted {
                run_id: "new".into(),
                workflow: simple_workflow(),
                task: "new".into(),
                step_by_step: true,
            },
        ];

        let run = current_run(&events).unwrap();
        assert_eq!(run.run_id, "new");
        assert_eq!(run.task, "new");
        assert!(run.step_by_step);
    }

    #[test]
    fn next_after_plain_edge_runs() {
        let state = state(simple_workflow());
        let next = next_after(&state, "start", "next", false);

        assert_eq!(
            next,
            Next::Run {
                node_id: "a".into(),
                via: Via {
                    from: "start".into(),
                    port: "next".into(),
                },
            }
        );
    }

    #[test]
    fn next_after_pause_edge_pauses() {
        let mut workflow = simple_workflow();
        workflow.edges[1].pause = true;
        let state = state(workflow);

        assert_eq!(
            next_after(&state, "a", "done", false),
            Next::Pause(Pause {
                from: "a".into(),
                port: "done".into(),
                to: "end".into(),
            })
        );
    }

    #[test]
    fn step_by_step_pauses_outside_branches_only() {
        let mut state = state(simple_workflow());
        state.step_by_step = true;

        assert!(matches!(
            next_after(&state, "start", "next", false),
            Next::Pause(_)
        ));
        assert!(matches!(
            next_after(&state, "start", "next", true),
            Next::Run { .. }
        ));
    }

    #[test]
    fn cap_reached_takes_exhausted_edge() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                Node {
                    id: "s".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Shell {
                        command: "cmd".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                },
                Node {
                    id: "b".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Agent {
                        agent: "plan".into(),
                        prompt: "p".into(),
                        retry_prompt: None,
                        outputs: vec!["done".into()],
                        context: AgentContext::Session,
                        verdicts: false,
                    },
                },
            ],
            edges: vec![
                Edge {
                    from: "s".into(),
                    port: "failure".into(),
                    to: "b".into(),
                    max_passes: Some(1),
                    pause: false,
                },
                Edge {
                    from: "s".into(),
                    port: EXHAUSTED.into(),
                    to: "b".into(),
                    max_passes: None,
                    pause: false,
                },
            ],
        };

        let mut state = state(workflow);
        state
            .edge_passes
            .insert(("s".to_string(), "failure".to_string()), 1);

        assert_eq!(
            next_after(&state, "s", "failure", false),
            Next::Run {
                node_id: "b".into(),
                via: Via {
                    from: "s".into(),
                    port: EXHAUSTED.into(),
                },
            }
        );
    }

    #[test]
    fn cap_reached_without_exhausted_finishes_blocked() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                Node {
                    id: "s".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Shell {
                        command: "cmd".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                },
                Node {
                    id: "b".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Agent {
                        agent: "plan".into(),
                        prompt: "p".into(),
                        retry_prompt: None,
                        outputs: vec!["done".into()],
                        context: AgentContext::Session,
                        verdicts: false,
                    },
                },
            ],
            edges: vec![Edge {
                from: "s".into(),
                port: "failure".into(),
                to: "b".into(),
                max_passes: Some(1),
                pause: false,
            }],
        };

        let mut state = state(workflow);
        state
            .edge_passes
            .insert(("s".to_string(), "failure".to_string()), 1);

        assert_eq!(
            next_after(&state, "s", "failure", false),
            Next::Finish {
                outcome: RunOutcome::Blocked,
                reason: Some("s → b: maxPasses (1) reached".into()),
            }
        );
    }

    #[test]
    fn max_steps_finishes_blocked() {
        let mut state = state(simple_workflow());
        state.steps = 2;

        assert_eq!(
            next_after(&state, "start", "next", false),
            Next::Finish {
                outcome: RunOutcome::Blocked,
                reason: Some("the run reached maxSteps (2)".into()),
            }
        );
    }

    #[test]
    fn end_outcomes_finish() {
        let done = state(simple_workflow());

        assert_eq!(
            enter(
                &done,
                Via {
                    from: "a".into(),
                    port: "done".into()
                },
                "end",
                false
            ),
            Next::Finish {
                outcome: RunOutcome::Done,
                reason: None,
            }
        );

        let mut workflow = simple_workflow();
        if let Some(node) = workflow.nodes.iter_mut().find(|node| node.id == "end") {
            node.kind = NodeKind::End {
                outcome: EndOutcome::Blocked,
            };
        }
        let blocked = state(workflow);

        assert_eq!(
            enter(
                &blocked,
                Via {
                    from: "a".into(),
                    port: "done".into()
                },
                "end",
                false
            ),
            Next::Finish {
                outcome: RunOutcome::Blocked,
                reason: Some("reached the end \"end\"".into()),
            }
        );
    }

    #[test]
    fn parallel_forks_to_its_join() {
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                Node {
                    id: "p".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Parallel,
                },
                Node {
                    id: "a".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Shell {
                        command: "a".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                },
                Node {
                    id: "b".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Shell {
                        command: "b".into(),
                        timeout_secs: None,
                        success_codes: None,
                    },
                },
                Node {
                    id: "j".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Join,
                },
            ],
            edges: vec![
                Edge {
                    from: "p".into(),
                    port: BRANCH.into(),
                    to: "a".into(),
                    max_passes: None,
                    pause: false,
                },
                Edge {
                    from: "p".into(),
                    port: BRANCH.into(),
                    to: "b".into(),
                    max_passes: None,
                    pause: false,
                },
                Edge {
                    from: "a".into(),
                    port: "success".into(),
                    to: "j".into(),
                    max_passes: None,
                    pause: false,
                },
                Edge {
                    from: "b".into(),
                    port: "success".into(),
                    to: "j".into(),
                    max_passes: None,
                    pause: false,
                },
            ],
        };

        let state = state(workflow);
        assert_eq!(
            enter(
                &state,
                Via {
                    from: "start".into(),
                    port: "next".into()
                },
                "p",
                false
            ),
            Next::Fork {
                parallel: "p".into(),
                entries: vec![
                    (
                        Via {
                            from: "p".into(),
                            port: BRANCH.into(),
                        },
                        "a".into()
                    ),
                    (
                        Via {
                            from: "p".into(),
                            port: BRANCH.into(),
                        },
                        "b".into()
                    ),
                ],
                join: "j".into(),
            }
        );
    }

    #[test]
    fn join_in_branch_reaches_join() {
        let state = state(simple_workflow());
        assert_eq!(
            enter(
                &state,
                Via {
                    from: "a".into(),
                    port: "done".into()
                },
                "end",
                true
            ),
            Next::Finish {
                outcome: RunOutcome::Done,
                reason: None,
            }
        );
    }

    #[test]
    fn variable_resolves_run_values() {
        let mut state = state(simple_workflow());
        state.node_passes.insert("a".to_string(), 3);
        state.results.insert(
            "a".to_string(),
            NodeResult {
                port: "done".into(),
                output: "out".into(),
                exit_code: Some(0),
            },
        );

        assert_eq!(variable(&state, "a", &[], "task"), Some("task".into()));
        assert_eq!(variable(&state, "a", &[], "pass"), Some("3".into()));
        assert_eq!(
            variable(&state, "a", &[], "nodes.a.output"),
            Some("out".into())
        );
        assert_eq!(
            variable(&state, "a", &[], "nodes.a.port"),
            Some("done".into())
        );
    }

    #[test]
    fn agent_prompt_uses_retry_and_comment() {
        let mut state = state(simple_workflow());
        state.comment = "look".into();

        let node = state.workflow.node("a").unwrap();
        assert_eq!(
            agent_prompt(&state, node, 2, &[]),
            "retry 2\n\nHuman comment: look"
        );
    }

    #[test]
    fn evaluate_conditions() {
        use crate::domain::workflow::definition::ConditionTest;

        assert!(evaluate(ConditionTest::Equals, "  a ", Some("a")));
        assert!(!evaluate(ConditionTest::Equals, "a", Some("b")));
        assert!(evaluate(ConditionTest::Contains, "hello", Some("ell")));
        assert!(evaluate(ConditionTest::Empty, "   ", None));
    }

    fn human_workflow() -> Workflow {
        let mut workflow = simple_workflow();
        workflow.nodes[1].kind = NodeKind::Human {
            question: "Go?".into(),
            choices: vec!["Go".into()],
        };
        workflow
    }

    #[test]
    fn waiting_is_none_when_not_active() {
        let mut state = state(simple_workflow());
        state.apply(&Event::WorkflowFinished {
            run_id: "r".into(),
            outcome: RunOutcome::Done,
            reason: None,
            stats: None,
        });
        assert_eq!(waiting(&state, &[]), None);
    }

    #[test]
    fn waiting_is_pause_when_paused() {
        let mut state = state(simple_workflow());
        state.apply(&Event::WorkflowPaused {
            run_id: "r".into(),
            from: "a".into(),
            port: "done".into(),
            to: "end".into(),
        });
        assert_eq!(waiting(&state, &[]), Some(Waiting::Pause));
    }

    #[test]
    fn waiting_is_human_when_a_human_node_is_open() {
        let mut state = state(human_workflow());
        state.apply(&Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "a".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
        });
        assert_eq!(waiting(&state, &[]), Some(Waiting::Human));
    }

    #[test]
    fn waiting_is_agent_question_when_the_turn_waits_for_the_user() {
        let mut state = state(simple_workflow());
        state.apply(&Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "a".into(),
            pass: 1,
            via: Some(Via {
                from: "start".into(),
                port: "next".into(),
            }),
            child_session_id: None,
        });
        let events = vec![Event::ChatDone {
            message_id: "m".into(),
            reason: DoneReason::WaitingForUser,
            duration_ms: None,
        }];
        assert_eq!(waiting(&state, &events), Some(Waiting::AgentQuestion));
    }

    #[test]
    fn waiting_is_interrupted_when_nothing_is_open() {
        let state = state(simple_workflow());
        assert_eq!(waiting(&state, &[]), Some(Waiting::Interrupted));
    }
}
