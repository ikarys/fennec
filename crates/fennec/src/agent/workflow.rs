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
use crate::domain::clock::now_ms;
use crate::domain::compaction::files_changed;
use crate::domain::conversation::last_assistant_text;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;
use crate::domain::workflow::definition::{AgentContext, NodeKind, Workflow};
use crate::domain::workflow::run::{self as run, Next, RunOutcome, RunState, Via};
use crate::domain::workflow::stats::{RunStats, run_stats};
use crate::domain::workflow::template::render;
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
        } => start_run(turn, &workflow, &task, step_by_step, &attachments)?,
        WorkflowJob::StartFromPlan {
            workflow,
            task,
            plan,
            attachments,
        } => start_from_plan(turn, &workflow, &task, &plan, &attachments)?,
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
    attachments: &[Attachment],
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

    // Session-context nodes see them in the conversation; fresh-context nodes do not.
    if !attachments.is_empty() {
        turn.append_user("", false, None, attachments.to_vec())?;
    }

    let run_id = new_id();
    turn.append(Event::WorkflowStarted {
        run_id: run_id.clone(),
        workflow: workflow.clone(),
        task: task.to_string(),
        step_by_step,
        started_at: Some(now_ms()),
    })?;

    let start_node = workflow.start().ok_or("no start node")?;
    turn.append(Event::WorkflowNodeStarted {
        run_id: run_id.clone(),
        node_id: start_node.id.clone(),
        pass: 1,
        via: None,
        child_session_id: None,
        started_at: Some(now_ms()),
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

fn start_from_plan(
    turn: &mut Turn,
    workflow: &Workflow,
    task: &str,
    plan: &str,
    attachments: &[Attachment],
) -> Result<Cursor, String> {
    // Checked before the run starts: a refusal leaves no half-recorded run.
    let plan_id = workflow
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            NodeKind::Criteria { plan } => Some(plan.clone()),
            _ => None,
        })
        .ok_or("the workflow has no criteria node naming its plan")?;
    let start_id = workflow.start().ok_or("no start node")?.id.clone();
    if workflow.edge(&start_id, "next").map(|edge| &edge.to) != Some(&plan_id) {
        return Err("the workflow does not start with its plan node".into());
    }

    start_run(turn, workflow, task, false, attachments)?;
    let events = turn.log.events(turn.session_id)?;
    let run_id = run::current_run(&events)
        .ok_or("no active workflow run")?
        .run_id;
    let via = |from: &str, port: &str| Via {
        from: from.to_string(),
        port: port.to_string(),
    };
    record_given(
        turn,
        &run_id,
        &plan_id,
        via(&start_id, "next"),
        "done",
        plan,
        None,
    )?;

    // The card's button is the human's approval: the human node right after
    // the plan is answered with its first choice.
    if let Some(human) = workflow
        .edge(&plan_id, "done")
        .and_then(|edge| workflow.node(&edge.to))
        && let NodeKind::Human { choices, .. } = &human.kind
        && let Some(choice) = choices.first()
    {
        let comment = Some("approved from the plan card".to_string());
        record_given(
            turn,
            &run_id,
            &human.id,
            via(&plan_id, "done"),
            choice,
            choice,
            comment,
        )?;
        // The recorded approval is the human's: it records the plan's
        // criteria too, so the builder works against a real contract.
        let events = turn.log.events(turn.session_id)?;
        if let Some(state) = run::current_run(&events) {
            criteria::record_from_plan(turn, &state)?;
        }
        return Ok(Cursor::After {
            from: human.id.clone(),
            port: choice.clone(),
        });
    }
    Ok(Cursor::After {
        from: plan_id,
        port: "done".into(),
    })
}

/// Records a node as run and finished with an outcome the human gave, not
/// one fennec computed.
fn record_given(
    turn: &mut Turn,
    run_id: &str,
    node_id: &str,
    via: Via,
    port: &str,
    output: &str,
    comment: Option<String>,
) -> Result<(), String> {
    turn.append(Event::WorkflowNodeStarted {
        run_id: run_id.to_string(),
        node_id: node_id.to_string(),
        pass: 1,
        via: Some(via),
        child_session_id: None,
        started_at: Some(now_ms()),
    })?;
    turn.append(Event::WorkflowNodeFinished {
        run_id: run_id.to_string(),
        node_id: node_id.to_string(),
        port: port.to_string(),
        output: output.to_string(),
        exit_code: None,
        comment,
        duration_ms: None,
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

    // The approval of the plan is what records its criteria: the builder then
    // works against a real contract, not an empty one.
    let approved = choices.first().is_some_and(|first| first == choice)
        && state.workflow.nodes.iter().any(|plan| {
            state
                .workflow
                .edge(&plan.id, "done")
                .is_some_and(|edge| edge.to == open.node_id)
        });
    if approved {
        criteria::record_from_plan(turn, &state)?;
    }

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

    let (node_id, via) = if let Some(open) = state.open.first() {
        // An interrupted run: re-enter the open node on its own pass.
        let via = events
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::WorkflowNodeStarted { node_id, via, .. } if node_id == &open.node_id => {
                    via.clone()
                }
                _ => None,
            })
            .unwrap_or_else(|| default_via(&state));
        (open.node_id.clone(), via)
    } else {
        // A run that ended in error: re-enter the node that was open when it
        // ended, on a new pass.
        match state.finished {
            Some((RunOutcome::Error, _)) => {
                let (node_id, via) = open_at_finish(&events, &state.run_id)
                    .ok_or("the run ended in error but no node can be re-run")?;
                if in_branch(&state.workflow, &node_id) {
                    return Err(
                        "the failed node is in a branch, it cannot be relaunched alone".into(),
                    );
                }
                (node_id, via.unwrap_or_else(|| default_via(&state)))
            }
            Some(_) => {
                return Err("the run is finished, it can only be relaunched on an error".into());
            }
            None => return Err("no open node to resume".into()),
        }
    };

    turn.append(Event::WorkflowResumed {
        run_id: state.run_id.clone(),
        comment: String::new(),
    })?;

    Ok(Cursor::Enter { via, to: node_id })
}

fn default_via(state: &RunState) -> Via {
    Via {
        from: state
            .workflow
            .start()
            .map(|n| n.id.clone())
            .unwrap_or_default(),
        port: "next".into(),
    }
}

/// The node that was open when a finished run ended: from the run's start on,
/// the last node started with no finish after it, and the via that entered it.
fn open_at_finish(events: &[Event], run_id: &str) -> Option<(String, Option<Via>)> {
    let start = events
        .iter()
        .rposition(|e| matches!(e, Event::WorkflowStarted { run_id: r, .. } if r == run_id))?;
    let mut open: Option<(String, Option<Via>)> = None;
    for event in &events[start + 1..] {
        match event {
            Event::WorkflowNodeStarted {
                run_id: r,
                node_id,
                via,
                ..
            } if r == run_id => {
                open = Some((node_id.clone(), via.clone()));
            }
            Event::WorkflowNodeFinished {
                run_id: r, node_id, ..
            } if r == run_id && open.as_ref().is_some_and(|(id, _)| id == node_id) => {
                open = None;
            }
            _ => {}
        }
    }
    open
}

/// A node that runs inside a parallel's fork region cannot be relaunched alone.
fn in_branch(workflow: &Workflow, node_id: &str) -> bool {
    workflow
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Parallel))
        .any(|p| workflow.branch_region(&p.id).contains(node_id))
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
    if let NodeKind::Human { question, .. } = &node.kind {
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
