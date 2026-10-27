//! The workflow engine: walks a run through its graph (docs/workflows.md).
//! The path is decided by domain/workflow/run.rs; this file runs the nodes
//! and records everything in the session's log.

mod criteria;
mod fork;
mod nodes;
mod outcome;

#[cfg(test)]
mod tests;

use super::ports::{ChildSessions, Commands};
use super::turn::{self, Turn};
use crate::domain::attachment::Attachment;
use crate::domain::compaction::files_changed;
use crate::domain::conversation::last_assistant_text;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;
use crate::domain::workflow::definition::{AgentContext, NodeKind, Workflow};
use crate::domain::workflow::run::{self as run, Next, RunOutcome, RunState, Via};
use crate::domain::workflow::stats::{RunStats, run_stats};
use crate::domain::workflow::validate;
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
        } => start_run(turn, &workflow, &task, step_by_step)?,
        WorkflowJob::Answer { choice, comment } => answer_human(turn, &choice, &comment)?,
        WorkflowJob::Continue { comment } => continue_paused(turn, &comment)?,
        WorkflowJob::AgentInput {
            prompt,
            attachments,
        } => {
            return agent_input(turn, env, &prompt, &attachments);
        }
        WorkflowJob::Resume => resume_interrupted(turn)?,
        WorkflowJob::Stop => return stop_run(turn),
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

fn start_run(
    turn: &mut Turn,
    workflow: &Workflow,
    task: &str,
    step_by_step: bool,
) -> Result<Cursor, String> {
    let issues = validate::validate(workflow, turn.agents);
    if !issues.is_empty() {
        return Err(issues
            .iter()
            .map(|i| i.message.clone())
            .collect::<Vec<_>>()
            .join("\n"));
    }

    let events = turn.log.events(turn.session_id)?;
    if let Some(state) = run::current_run(&events)
        && state.is_active()
    {
        return Err("a workflow is already running in this session".into());
    }

    let run_id = new_id();
    turn.append(Event::WorkflowStarted {
        run_id: run_id.clone(),
        workflow: workflow.clone(),
        task: task.to_string(),
        step_by_step,
    })?;

    let start_node = workflow.start().ok_or("no start node")?;
    turn.append(Event::WorkflowNodeStarted {
        run_id: run_id.clone(),
        node_id: start_node.id.clone(),
        pass: 1,
        via: None,
        child_session_id: None,
    })?;
    turn.append(Event::WorkflowNodeFinished {
        run_id,
        node_id: start_node.id.clone(),
        port: "next".into(),
        output: task.to_string(),
        exit_code: None,
        comment: None,
        duration_ms: None,
    })?;

    Ok(Cursor::After {
        from: start_node.id.clone(),
        port: "next".into(),
    })
}

fn answer_human(turn: &mut Turn, choice: &str, comment: &str) -> Result<Cursor, String> {
    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active workflow run")?;

    let open = state.open.first().ok_or("no open node to answer")?;
    let node = state
        .workflow
        .node(&open.node_id)
        .ok_or_else(|| format!("{}: unknown node", open.node_id))?;

    let NodeKind::Human { choices, .. } = &node.kind else {
        return Err("the open node is not a human node".into());
    };

    if !choices.iter().any(|c| c == choice) {
        return Err(format!("'{choice}' is not a choice of this node"));
    }

    let comment = if comment.is_empty() {
        None
    } else {
        Some(comment.to_string())
    };
    turn.append(Event::WorkflowNodeFinished {
        run_id: state.run_id.clone(),
        node_id: open.node_id.clone(),
        port: choice.to_string(),
        output: choice.to_string(),
        exit_code: None,
        comment,
        duration_ms: None,
    })?;

    Ok(Cursor::After {
        from: open.node_id.clone(),
        port: choice.to_string(),
    })
}

fn continue_paused(turn: &mut Turn, comment: &str) -> Result<Cursor, String> {
    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active workflow run")?;

    let pause = state.paused.as_ref().ok_or("the run is not paused")?;

    turn.append(Event::WorkflowResumed {
        run_id: state.run_id.clone(),
        comment: comment.to_string(),
    })?;

    Ok(Cursor::Enter {
        via: Via {
            from: pause.from.clone(),
            port: pause.port.clone(),
        },
        to: pause.to.clone(),
    })
}

fn resume_interrupted(turn: &mut Turn) -> Result<Cursor, String> {
    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active workflow run")?;

    if state.paused.is_some() {
        return Err("the run is paused, not interrupted".into());
    }

    let open = state.open.first().ok_or("no open node to resume")?;

    turn.append(Event::WorkflowResumed {
        run_id: state.run_id.clone(),
        comment: String::new(),
    })?;

    let via = events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::WorkflowNodeStarted { node_id, via, .. } if node_id == &open.node_id => {
                via.clone()
            }
            _ => None,
        })
        .unwrap_or_else(|| Via {
            from: state
                .workflow
                .start()
                .map(|n| n.id.clone())
                .unwrap_or_default(),
            port: "next".into(),
        });

    Ok(Cursor::Enter {
        via,
        to: open.node_id.clone(),
    })
}

fn stop_run(turn: &mut Turn) -> Result<DoneReason, String> {
    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active workflow run")?;
    finish_run(
        turn,
        &state,
        RunOutcome::Stopped,
        Some("stopped by the human".into()),
    )
}

fn agent_input(
    turn: &mut Turn,
    env: &WorkflowEnv,
    prompt: &str,
    attachments: &[Attachment],
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
    let reason = turn::answer_or_prompt(turn, prompt, attachments)?;
    let events = turn.log.events(turn.session_id)?;
    let text = last_assistant_text(&events[before..]);
    let label = node.label();

    let end = nodes::agent_end(label, outputs, reason, text, |reminder| {
        session_send(turn, agent, reminder)
    })?;
    let step = record(turn, &state, &node.id, end, None)?;

    match step {
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
    let pass = state.node_passes.get(node_id).copied().unwrap_or(0) + 1;
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
    if matches!(node.kind, NodeKind::Human { .. }) {
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
    let text = last_assistant_text(&events[before..]);
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
    let text = last_assistant_text(&events[before..]);
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
        nodes::NodeEnd::AgentAsked => Ok(Step::Exit(DoneReason::WaitingForUser)),
    }
}

/// The facts of the run that ends now, read back from the log: its own
/// events, its child sessions' and its criteria as last recorded.
fn finished_run_stats(turn: &Turn) -> Result<RunStats, String> {
    let events = turn.log.events(turn.session_id)?;
    let run_events = run::current_run_events(&events);
    let mut child_ids: Vec<&String> = Vec::new();
    for event in run_events {
        if let Event::WorkflowNodeStarted {
            child_session_id: Some(child),
            ..
        } = event
            && !child_ids.contains(&child)
        {
            child_ids.push(child);
        }
    }
    // A child log that cannot be read is skipped: the run must still finish.
    let children: Vec<Vec<Event>> = child_ids
        .iter()
        .filter_map(|id| turn.log.events(id).ok())
        .collect();
    let criteria = run::current_run(&events)
        .map(|state| state.criteria)
        .unwrap_or_default();
    let files = files_changed(run_events);
    Ok(run_stats(run_events, &children, &criteria, files.len()))
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
        })?;
    }
    Ok(reason)
}
