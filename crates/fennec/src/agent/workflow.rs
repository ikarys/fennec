//! The workflow engine: walks a run through its graph (docs/devs/workflows.md).
//! The path is decided by domain/workflow/run.rs; this file runs the nodes
//! and records everything in the session's log.

mod criteria;
mod fork;
mod jobs;
mod nodes;
mod outcome;

#[cfg(test)]
mod tests;

use super::ports::{ChildSessions, Commands};
use super::turn::{self, Turn};
use crate::domain::attachment::Attachment;
use crate::domain::clock::now_ms;
use crate::domain::compaction::files_changed;
use crate::domain::conversation::node_output;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;
use crate::domain::workflow::definition::{AgentContext, NodeKind, Workflow};
use crate::domain::workflow::run::{self as run, Next, RunOutcome, RunState, Via};
use crate::domain::workflow::stats::{RunStats, run_stats};
use crate::domain::workflow::template::render;
use nodes::NodeInput;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub struct WorkflowEnv<'a> {
    pub children: &'a dyn ChildSessions,
    pub commands: &'a dyn Commands,
}

/// What the human asks of a session's workflow.
#[derive(Clone)]
pub enum WorkflowJob {
    Start {
        workflow: Workflow,
        task: String,
        step_by_step: bool,
        /// The composer's images, appended to the session before the run starts.
        attachments: Vec<Attachment>,
    },
    /// A run that starts from a plan the human already approved (the plan
    /// card's button): the plan node and the approval are recorded, not run.
    StartFromPlan {
        workflow: Workflow,
        task: String,
        plan: String,
        attachments: Vec<Attachment>,
    },
    /// The choice at a human node, with an optional comment.
    Answer { choice: String, comment: String },
    /// Past a pause.
    Continue { comment: String },
    /// The answer to an ask_user inside a session-context agent node.
    AgentInput {
        prompt: String,
        attachments: Vec<Attachment>,
    },
    /// Re-runs the node an interrupted run was running.
    Resume,
    /// Ends a run that is waiting or interrupted.
    Stop,
}

/// Where the walk continues from.
enum Cursor {
    After { from: String, port: String },
    Enter { via: Via, to: String },
}

/// One step of the walk: continue from a cursor, or end the turn.
enum Step {
    Go(Cursor),
    Exit(DoneReason),
}

pub fn run_workflow(
    turn: &mut Turn,
    env: &WorkflowEnv,
    job: WorkflowJob,
) -> Result<DoneReason, String> {
    turn::with_running(turn, |turn| walk(turn, env, job))
}

fn walk(turn: &mut Turn, env: &WorkflowEnv, job: WorkflowJob) -> Result<DoneReason, String> {
    let cursor = match job {
        WorkflowJob::Start {
            workflow,
            task,
            step_by_step,
            attachments,
        } => jobs::start_run(turn, &workflow, &task, step_by_step, &attachments)?,
        WorkflowJob::StartFromPlan {
            workflow,
            task,
            plan,
            attachments,
        } => jobs::start_from_plan(turn, &workflow, &task, &plan, &attachments)?,
        WorkflowJob::Answer { choice, comment } => jobs::answer_human(turn, &choice, &comment)?,
        WorkflowJob::Continue { comment } => jobs::continue_paused(turn, &comment)?,
        WorkflowJob::AgentInput {
            prompt,
            attachments,
        } => {
            return continue_agent_node(turn, env, |turn| {
                turn::answer_or_prompt(turn, &prompt, &attachments)
            });
        }
        WorkflowJob::Resume => jobs::resume_interrupted(turn)?,
        WorkflowJob::Stop => return jobs::stop_run(turn),
    };
    walk_from(turn, env, cursor)
}

fn walk_from(turn: &mut Turn, env: &WorkflowEnv, mut cursor: Cursor) -> Result<DoneReason, String> {
    loop {
        let events = turn.log.events(turn.session_id)?;
        let state = run::current_run(&events).ok_or("no active workflow run")?;

        if turn.stop.load(Ordering::Relaxed) {
            return finish_run(turn, &state, RunOutcome::Stopped, Some("stopped".into()));
        }

        let next = match &cursor {
            Cursor::After { from, port } => run::next_after(&state, from, port, false),
            Cursor::Enter { via, to } => run::enter(&state, via.clone(), to, false),
        };

        match next {
            Next::Run { node_id, via } => match run_node(turn, env, &state, &node_id, via)? {
                Step::Go(c) => cursor = c,
                Step::Exit(reason) => return end_turn(turn, reason),
            },
            Next::Pause(p) => {
                turn.append(Event::WorkflowPaused {
                    run_id: state.run_id.clone(),
                    from: p.from.clone(),
                    port: p.port.clone(),
                    to: p.to.clone(),
                })?;
                return end_turn(turn, DoneReason::WaitingForUser);
            }
            Next::Fork {
                parallel,
                entries,
                join,
            } => {
                let fork = fork::Fork {
                    parallel: &parallel,
                    entries: &entries,
                    join: &join,
                };
                match fork::run_fork(turn, env, &state, fork)? {
                    Step::Go(c) => cursor = c,
                    Step::Exit(reason) => return end_turn(turn, reason),
                }
            }
            Next::ReachJoin { .. } => {
                return Err("a join was reached outside a branch".into());
            }
            Next::Finish { outcome, reason } => {
                return finish_run(turn, &state, outcome, reason);
            }
        }
    }
}

/// The human's answer to an open question inside a session-context agent
/// node: the continuation is the node's work, so it finishes the node and
/// walks on. Outside such a node it just runs the answer's own loop.
pub fn answer_in_node(
    turn: &mut Turn,
    env: &WorkflowEnv,
    step: impl FnOnce(&mut Turn) -> Result<DoneReason, String>,
) -> Result<DoneReason, String> {
    turn::with_running(turn, |turn| {
        let events = turn.log.events(turn.session_id)?;
        let in_node = run::current_run(&events).is_some_and(|state| {
            state
                .open
                .first()
                .and_then(|open| state.workflow.node(&open.node_id))
                .is_some_and(|node| {
                    matches!(
                        node.kind,
                        NodeKind::Agent {
                            context: AgentContext::Session,
                            ..
                        }
                    )
                })
        });
        if in_node {
            continue_agent_node(turn, env, step)
        } else {
            step(turn)
        }
    })
}

fn continue_agent_node(
    turn: &mut Turn,
    env: &WorkflowEnv,
    step: impl FnOnce(&mut Turn) -> Result<DoneReason, String>,
) -> Result<DoneReason, String> {
    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active workflow run")?;

    let open = state.open.first().ok_or("no open node")?;
    let node = state
        .workflow
        .node(&open.node_id)
        .ok_or_else(|| format!("{}: unknown node", open.node_id))?;

    let NodeKind::Agent {
        agent,
        context: AgentContext::Session,
        outputs,
        ..
    } = &node.kind
    else {
        return Err("the open node is not a session-context agent node".into());
    };

    let before = events.len();
    let reason = step(turn)?;
    let events = turn.log.events(turn.session_id)?;
    let text = node_output(&events[before..]);
    let label = node.label();

    let end = nodes::agent_end(label, outputs, reason, text, |reminder| {
        session_send(turn, agent, reminder)
    })?;
    let next = record(turn, &state, &node.id, end, None)?;

    match next {
        Step::Go(cursor) => walk_from(turn, env, cursor),
        Step::Exit(reason) => end_turn(turn, reason),
    }
}

fn run_node(
    turn: &mut Turn,
    env: &WorkflowEnv,
    state: &RunState,
    node_id: &str,
    via: Via,
) -> Result<Step, String> {
    let node = state
        .workflow
        .node(node_id)
        .ok_or_else(|| format!("{node_id}: unknown node"))?;
    // An interrupted node resumes the pass it was started on, not a new one.
    let pass = state
        .interrupted
        .iter()
        .find(|open| open.node_id == node_id)
        .map(|open| open.pass)
        .unwrap_or_else(|| state.node_passes.get(node_id).copied().unwrap_or(0) + 1);
    // A fresh agent's child is created first, so its start names it.
    let child = match &node.kind {
        NodeKind::Agent {
            context: AgentContext::Fresh,
            ..
        } => Some(
            env.children
                .create(turn.session_id, &nodes::child_title(state, node))?,
        ),
        _ => None,
    };
    let started = Instant::now();
    append_node_started(turn, state, node_id, pass, &via, child.as_deref())?;
    if let NodeKind::Human { question, choices } = &node.kind {
        let (state, files) = folded_after_start(turn)?;
        turn.append(Event::WorkflowHumanAsked {
            run_id: state.run_id.clone(),
            node_id: node_id.to_string(),
            question: render(
                question,
                &|name| run::variable(&state, node_id, &files, name),
                false,
            ),
        })?;
        // Night mode: the kernel, never the model, takes the forward choice
        // and records it as a decision the human reviews in the morning.
        if turn.autonomous {
            return jobs::answer_by_night(turn, node, choices).map(Step::Go);
        }
        return Ok(Step::Exit(DoneReason::WaitingForUser));
    }

    let (state, files) = folded_after_start(turn)?;
    let input = NodeInput {
        state: &state,
        node,
        pass,
        files: &files,
        stop: turn.stop,
    };
    let (agent_id, verdicts) = match &node.kind {
        NodeKind::Agent {
            agent, verdicts, ..
        } => (agent.clone(), *verdicts),
        _ => (String::new(), false),
    };
    let end = match (&node.kind, child.as_deref()) {
        (NodeKind::Agent { agent, outputs, .. }, Some(child)) => {
            nodes::fresh_agent(env, &input, agent, outputs, child)?
        }
        (NodeKind::Agent { agent, outputs, .. }, None) => {
            session_agent(turn, &input, agent, outputs)?
        }
        (NodeKind::Criteria { plan }, _) => criteria::run(turn, env, &input, plan)?,
        (NodeKind::Shell { .. }, _) => nodes::shell(env, &input),
        (NodeKind::Condition { .. }, _) => nodes::condition(&input),
        _ => return Err(format!("{node_id}: cannot run as a node")),
    };
    if verdicts && let nodes::NodeEnd::Finished { output, .. } = &end {
        criteria::record_verdicts(turn, &state, &agent_id, output)?;
    }
    record(
        turn,
        &state,
        node_id,
        end,
        Some(started.elapsed().as_millis() as u64),
    )
}

fn session_agent(
    turn: &mut Turn,
    input: &NodeInput,
    agent: &str,
    outputs: &[String],
) -> Result<nodes::NodeEnd, String> {
    let prompt = nodes::agent_prompt(input, outputs);
    let before = turn.log.events(turn.session_id)?.len();
    let reason = turn::run_agent_node(turn, agent, &prompt)?;
    let events = turn.log.events(turn.session_id)?;
    let text = node_output(&events[before..]);
    let label = input.node.label();

    nodes::agent_end(label, outputs, reason, text, |reminder| {
        session_send(turn, agent, reminder)
    })
}

fn session_send(
    turn: &mut Turn,
    agent: &str,
    prompt: &str,
) -> Result<(DoneReason, String), String> {
    let before = turn.log.events(turn.session_id)?.len();
    let reason = turn::run_agent_node(turn, agent, prompt)?;
    let events = turn.log.events(turn.session_id)?;
    let text = node_output(&events[before..]);
    Ok((reason, text))
}

fn folded_after_start(turn: &mut Turn) -> Result<(RunState, Vec<String>), String> {
    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active run")?;
    let files = files_changed(run::current_run_events(&events));
    Ok((state, files))
}

fn append_node_started(
    turn: &mut Turn,
    state: &RunState,
    node_id: &str,
    pass: u32,
    via: &Via,
    child_session_id: Option<&str>,
) -> Result<(), String> {
    turn.append(Event::WorkflowNodeStarted {
        run_id: state.run_id.clone(),
        node_id: node_id.to_string(),
        pass,
        via: Some(via.clone()),
        child_session_id: child_session_id.map(|s| s.to_string()),
        started_at: Some(now_ms()),
    })
}

fn record(
    turn: &mut Turn,
    state: &RunState,
    node_id: &str,
    end: nodes::NodeEnd,
    duration_ms: Option<u64>,
) -> Result<Step, String> {
    match end {
        nodes::NodeEnd::Finished {
            port,
            output,
            exit_code,
        } => {
            turn.append(Event::WorkflowNodeFinished {
                run_id: state.run_id.clone(),
                node_id: node_id.to_string(),
                port: port.clone(),
                output,
                exit_code,
                comment: None,
                duration_ms,
            })?;
            Ok(Step::Go(Cursor::After {
                from: node_id.to_string(),
                port,
            }))
        }
        nodes::NodeEnd::EndRun { outcome, reason } => {
            finish_run(turn, state, outcome, reason).map(Step::Exit)
        }
        // A model error leaves the node open; the run exits interrupted so the
        // human can resume it or stop it.
        nodes::NodeEnd::Halted { .. } => Ok(Step::Exit(DoneReason::Error)),
        nodes::NodeEnd::AgentAsked => Ok(Step::Exit(DoneReason::WaitingForUser)),
    }
}

/// The facts of the run that ends now, read back from the log: its own
/// events and its criteria as last recorded.
fn finished_run_stats(turn: &Turn) -> Result<RunStats, String> {
    let events = turn.log.events(turn.session_id)?;
    let run_events = run::current_run_events(&events);
    let criteria = run::current_run(&events)
        .map(|state| state.criteria)
        .unwrap_or_default();
    let files = files_changed(run_events);
    let mut stats = run_stats(run_events, &criteria, files.len());
    stats.finished_at = Some(now_ms());
    Ok(stats)
}

fn finish_run(
    turn: &mut Turn,
    state: &RunState,
    outcome: RunOutcome,
    reason: Option<String>,
) -> Result<DoneReason, String> {
    let stats = finished_run_stats(turn)?;

    turn.append(Event::WorkflowFinished {
        run_id: state.run_id.clone(),
        outcome,
        reason,
        stats: Some(stats),
    })?;
    end_turn(turn, map_outcome(outcome))
}

fn map_outcome(outcome: RunOutcome) -> DoneReason {
    match outcome {
        RunOutcome::Done | RunOutcome::Blocked => DoneReason::Complete,
        RunOutcome::Stopped => DoneReason::Stopped,
        RunOutcome::Error => DoneReason::Error,
    }
}

fn end_turn(turn: &mut Turn, reason: DoneReason) -> Result<DoneReason, String> {
    let events = turn.log.events(turn.session_id)?;
    if !matches!(events.last(), Some(Event::ChatDone { .. })) {
        turn.append(Event::ChatDone {
            message_id: new_id(),
            reason,
            duration_ms: None,
        })?;
    }
    Ok(reason)
}
