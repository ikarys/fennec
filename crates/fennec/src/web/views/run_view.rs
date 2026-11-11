//! The server-side run view: the run the server serves pre-calculated, so the UI renders it as it is.

use crate::domain::event::Event;
use crate::domain::workflow::definition::{AgentContext, Edge, NodeKind, Workflow};
use crate::domain::workflow::run::RunOutcome;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A step's state, as the rail shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStepState {
    Running,
    Done,
    Ended,
    Interrupted,
}

/// One node run, in the order it started (a node run twice appears twice).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRun {
    pub node_id: String,
    pub label: String,
    #[serde(flatten)]
    pub kind: NodeKind,
    pub pass: u32,
    pub state: RunStepState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Rows of the same node compare equal across passes: a second pass is the
/// same step, not a different one.
impl PartialEq for NodeRun {
    fn eq(&self, other: &NodeRun) -> bool {
        self.node_id == other.node_id
            && self.label == other.label
            && self.kind == other.kind
            && self.state == other.state
            && self.port == other.port
            && self.child_session_id == other.child_session_id
            && self.started_at == other.started_at
            && self.duration_ms == other.duration_ms
            && self.output == other.output
    }
}

impl Eq for NodeRun {}

/// The paused edge: where the run waits for the human.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PausedEdge {
    pub from: String,
    pub port: String,
    pub to: String,
}

/// The finished run's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    pub outcome: RunOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The completion counts the rail's status line reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCounts {
    pub done: u32,
    pub open: u32,
    pub total: u32,
}

/// A step the run has not reached yet, on its nominal path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingStep {
    pub node_id: String,
    pub label: String,
}

/// Nodes started and not finished, in start order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpenNodes(Vec<String>);

impl OpenNodes {
    fn push(&mut self, node_id: String) {
        self.0.push(node_id);
    }
    fn contains(&self, node_id: &str) -> bool {
        self.0.iter().any(|open| open == node_id)
    }
    fn remove_last(&mut self, node_id: &str) {
        if let Some(at) = self.0.iter().rposition(|open| open == node_id) {
            self.0.remove(at);
        }
    }
    fn last(&self) -> Option<&str> {
        self.0.last().map(String::as_str)
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn clear(&mut self) {
        self.0.clear();
    }
}

impl PartialEq for OpenNodes {
    fn eq(&self, other: &OpenNodes) -> bool {
        self.0 == other.0
    }
}

/// The test contract compares the open nodes against a plain list of ids.
impl PartialEq<Vec<String>> for OpenNodes {
    fn eq(&self, other: &Vec<String>) -> bool {
        self.0 == *other
    }
}

/// The run, pre-calculated: steps, states, ports, rounds, upcoming steps and
/// completion counts, folded from the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub run_id: String,
    pub workflow: Workflow,
    pub task: String,
    pub step_by_step: bool,
    /// The human asked to pause at the next edge; cleared once paused.
    pub pause_requested: bool,
    pub runs: Vec<NodeRun>,
    pub open: OpenNodes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<PausedEdge>,
    /// Human nodes' questions with their {{…}} names filled in.
    pub questions: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished: Option<Finished>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_comment: Option<String>,
    pub rounds: Vec<Vec<NodeRun>>,
    pub upcoming: Vec<UpcomingStep>,
    pub counts: RunCounts,
}

/// The log from the latest `workflow.started` on, folded into the view the
/// run rail renders as it is; `None` when the log has no run.
pub fn run_view(events: &[Event]) -> Option<RunView> {
    let start = events
        .iter()
        .rposition(|event| matches!(event, Event::WorkflowStarted { .. }))?;
    let Event::WorkflowStarted {
        run_id,
        workflow,
        task,
        step_by_step,
        ..
    } = &events[start]
    else {
        return None;
    };

    let mut view = RunView {
        run_id: run_id.clone(),
        workflow: workflow.clone(),
        task: task.clone(),
        step_by_step: *step_by_step,
        pause_requested: false,
        runs: Vec::new(),
        open: OpenNodes(Vec::new()),
        paused: None,
        questions: HashMap::new(),
        finished: None,
        last_comment: None,
        rounds: Vec::new(),
        upcoming: Vec::new(),
        counts: RunCounts {
            done: 0,
            open: 0,
            total: 0,
        },
    };

    for event in &events[start + 1..] {
        apply(&mut view, event);
    }

    view.upcoming = upcoming(&view);
    view.rounds = rounds(&view.runs);
    let done = view
        .runs
        .iter()
        .filter(|run| matches!(run.state, RunStepState::Done | RunStepState::Ended))
        .count() as u32;
    let open = view.open.len() as u32;
    // Upcoming steps count in the total only for nodes not run yet: a loop's
    // second pass is already in done/open.
    let upcoming = view
        .upcoming
        .iter()
        .filter(|step| !view.runs.iter().any(|run| run.node_id == step.node_id))
        .count() as u32;
    view.counts = RunCounts {
        done,
        open,
        total: done + open + upcoming,
    };

    Some(view)
}

fn apply(view: &mut RunView, event: &Event) {
    match event {
        Event::WorkflowNodeStarted {
            run_id,
            node_id,
            pass,
            child_session_id,
            started_at,
            ..
        } if run_id == &view.run_id => {
            // A resume restarts an interrupted node on its own pass: same row.
            if let Some(existing) = view.runs.iter_mut().find(|candidate| {
                candidate.node_id == *node_id
                    && candidate.pass == *pass
                    && matches!(
                        candidate.state,
                        RunStepState::Running | RunStepState::Interrupted
                    )
            }) {
                existing.state = RunStepState::Running;
                if let Some(child) = child_session_id {
                    existing.child_session_id = Some(child.clone());
                }
                if let Some(at) = started_at {
                    existing.started_at = Some(*at);
                }
                // workflow.resumed emptied open; the restarted node is open again.
                if !view.open.contains(node_id) {
                    view.open.push(node_id.clone());
                }
                return;
            }
            let node = view.workflow.node(node_id);
            view.runs.push(NodeRun {
                node_id: node_id.clone(),
                label: node
                    .map(|node| node.label().to_string())
                    .unwrap_or_else(|| node_id.clone()),
                kind: default_agent_kind(),
                pass: *pass,
                state: RunStepState::Running,
                port: None,
                child_session_id: child_session_id.clone(),
                started_at: *started_at,
                duration_ms: None,
                output: None,
            });
            view.open.push(node_id.clone());
        }
        Event::WorkflowNodeFinished {
            run_id,
            node_id,
            port,
            output,
            duration_ms,
            ..
        } if run_id == &view.run_id => {
            // The last running row of the node is the one that finished.
            if let Some(at) = view.runs.iter().rposition(|candidate| {
                candidate.node_id == *node_id && candidate.state == RunStepState::Running
            }) {
                let candidate = &mut view.runs[at];
                candidate.state = RunStepState::Done;
                candidate.port = Some(port.clone());
                if let Some(ms) = duration_ms {
                    candidate.duration_ms = Some(*ms);
                }
                if !output.is_empty() {
                    candidate.output = Some(output.clone());
                }
            }
            view.open.remove_last(node_id);
        }
        Event::WorkflowHumanAsked {
            run_id,
            node_id,
            question,
        } if run_id == &view.run_id => {
            view.questions.insert(node_id.clone(), question.clone());
        }
        Event::WorkflowPauseRequested { run_id } if run_id == &view.run_id => {
            view.pause_requested = true;
        }
        Event::WorkflowPaused {
            run_id,
            from,
            port,
            to,
        } if run_id == &view.run_id => {
            view.pause_requested = false;
            view.paused = Some(PausedEdge {
                from: from.clone(),
                port: port.clone(),
                to: to.clone(),
            });
        }
        Event::WorkflowResumed { run_id, comment } if run_id == &view.run_id => {
            for node_run in &mut view.runs {
                if node_run.state == RunStepState::Running {
                    node_run.state = RunStepState::Interrupted;
                }
            }
            view.paused = None;
            view.open.clear();
            // A resume after a finish (a run relaunched on its error) is active
            // again: the next workflow.finished ends it.
            view.finished = None;
            view.last_comment = if comment.is_empty() {
                None
            } else {
                Some(comment.clone())
            };
        }
        Event::WorkflowFinished {
            run_id,
            outcome,
            reason,
            ..
        } if run_id == &view.run_id => {
            // A node still running when the run finishes is the one that ended it
            // (a criteria node blocking, a stop): its port is the run's outcome.
            let port = outcome_port(outcome);
            for node_run in &mut view.runs {
                if node_run.state == RunStepState::Running {
                    node_run.state = RunStepState::Ended;
                    node_run.port = Some(port.clone());
                }
            }
            view.open.clear();
            view.paused = None;
            view.pause_requested = false;
            view.finished = Some(Finished {
                outcome: *outcome,
                reason: reason.clone(),
            });
        }
        _ => {}
    }
}

/// The kind a node run is served with: the definition's kind is looked up by
/// the UI from `workflow`, the view carries the agent default.
fn default_agent_kind() -> NodeKind {
    NodeKind::Agent {
        agent: String::new(),
        prompt: String::new(),
        retry_prompt: None,
        outputs: vec!["done".into()],
        context: AgentContext::Session,
        verdicts: false,
    }
}

/// The run's outcome as a port name.
fn outcome_port(outcome: &RunOutcome) -> String {
    match outcome {
        RunOutcome::Done => "done".into(),
        RunOutcome::Blocked => "blocked".into(),
        RunOutcome::Stopped => "stopped".into(),
        RunOutcome::Error => "error".into(),
    }
}

fn port_tone(port: &str) -> PortTone {
    let normalized = port.trim().to_lowercase();
    if POSITIVE_PORTS.contains(&normalized.as_str()) {
        PortTone::Positive
    } else if NEGATIVE_PORTS.contains(&normalized.as_str()) {
        PortTone::Negative
    } else {
        PortTone::Other
    }
}

#[derive(PartialEq)]
enum PortTone {
    Positive,
    Negative,
    Other,
}

const POSITIVE_PORTS: [&str; 10] = [
    "next", "done", "pass", "success", "clean", "true", "go", "ok", "accept", "minor",
];
const NEGATIVE_PORTS: [&str; 8] = [
    "fail",
    "failure",
    "findings",
    "false",
    "exhausted",
    "blocked",
    "stop",
    "revise",
];

/// Split node runs, in the order they started, into rounds. A new round starts
/// at a node run whose nodeId already appeared in the current round, which is
/// how a loop's second pass of the same node is separated from its first.
fn rounds(runs: &[NodeRun]) -> Vec<Vec<NodeRun>> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for node_run in runs {
        if seen.contains(&node_run.node_id) {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            seen.clear();
        }
        current.push(node_run.clone());
        seen.insert(node_run.node_id.clone());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The nodes still to come after the current one, following at each node its
/// nominal edge: a positive port first, else the first edge whose port is not
/// negative, in the definition's order. Stops at an end node (not listed), at
/// a node already listed (a loop), or when nothing leads on. A finished run
/// has none.
fn upcoming(view: &RunView) -> Vec<UpcomingStep> {
    if view.finished.is_some() || view.workflow.nodes.is_empty() {
        return Vec::new();
    }

    let next_node = |node_id: &str| -> Option<String> {
        let outgoing: Vec<&Edge> = view
            .workflow
            .edges
            .iter()
            .filter(|edge| edge.from == node_id)
            .collect();
        outgoing
            .iter()
            .find(|edge| port_tone(&edge.port) == PortTone::Positive)
            .or_else(|| {
                outgoing
                    .iter()
                    .find(|edge| port_tone(&edge.port) != PortTone::Negative)
            })
            .map(|edge| edge.to.clone())
    };

    let Some(start) = view
        .paused
        .as_ref()
        .map(|paused| paused.to.clone())
        .or_else(|| view.open.last().map(str::to_string))
        .or_else(|| view.runs.last().map(|run| run.node_id.clone()))
    else {
        return Vec::new();
    };

    let mut steps = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if view.paused.is_some() {
        // The paused target is a step to come; terminal nodes are not.
        add_step(view, &mut seen, &mut steps, &start);
    } else {
        // The current node is already a row; only mark it so a loop does not relist it.
        seen.insert(start.clone());
    }

    let mut current = start;
    // One guard per node, so a malformed cycle cannot run the walk forever.
    for _ in 0..view.workflow.nodes.len() {
        let Some(next) = next_node(&current) else {
            break;
        };
        if seen.contains(&next) {
            break;
        }
        // An end node ends the walk without being listed.
        if !add_step(view, &mut seen, &mut steps, &next) {
            break;
        }
        current = next;
    }

    steps
}

/// List `node_id` as a step to come; `false` when it is a terminal node and
/// the walk must stop there.
fn add_step(
    view: &RunView,
    seen: &mut std::collections::HashSet<String>,
    steps: &mut Vec<UpcomingStep>,
    node_id: &str,
) -> bool {
    let node = view.workflow.node(node_id);
    if node.is_some_and(|node| matches!(node.kind, NodeKind::End { .. })) {
        return false;
    }
    if seen.insert(node_id.to_string()) {
        steps.push(UpcomingStep {
            node_id: node_id.to_string(),
            label: node
                .map(|node| node.label().to_string())
                .unwrap_or_else(|| node_id.to_string()),
        });
    }
    true
}

#[cfg(test)]
mod tests;
