//! The parallel part of the workflow engine: runs branches in threads and
//! merges their events into the parent log.

use super::nodes::{self, NodeEnd};
use super::{Cursor, Step, WorkflowEnv};
use crate::agent::turn::Turn;
use crate::domain::compaction::files_changed;
use crate::domain::event::Event;
use crate::domain::workflow::definition::{BRANCH, NodeKind};
use crate::domain::workflow::run::{self as run, Next, RunOutcome, RunState, Via};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};

pub(super) struct Fork<'a> {
    pub parallel: &'a str,
    pub entries: &'a [(Via, String)],
    pub join: &'a str,
}

pub(super) fn run_fork(
    turn: &mut Turn,
    env: &WorkflowEnv,
    state: &RunState,
    fork: Fork<'_>,
) -> Result<Step, String> {
    let pass = state.node_passes.get(fork.parallel).copied().unwrap_or(0) + 1;
    turn.append(Event::WorkflowNodeStarted {
        run_id: state.run_id.clone(),
        node_id: fork.parallel.to_string(),
        pass,
        via: None,
        child_session_id: None,
    })?;
    turn.append(Event::WorkflowNodeFinished {
        run_id: state.run_id.clone(),
        node_id: fork.parallel.to_string(),
        port: BRANCH.into(),
        output: String::new(),
        exit_code: None,
        comment: None,
    })?;

    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active run")?;
    let files = files_changed(run::current_run_events(&events));
    let parent_session_id = turn.session_id.to_string();
    let stop = turn.stop;

    let (tx, rx) = mpsc::channel();
    let mut append_error = None;
    let results: Vec<BranchResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = fork
            .entries
            .iter()
            .map(|(via, entry)| {
                let tx = tx.clone();
                let branch = Branch {
                    env,
                    state: state.clone(),
                    files: &files,
                    parent_session_id: &parent_session_id,
                    stop,
                    via: via.clone(),
                    entry: entry.clone(),
                };
                scope.spawn(move || walk_branch(branch, tx))
            })
            .collect();
        drop(tx);
        // Keep draining after a failed append: the branches still run and
        // would block on nothing, but their events must not be half-recorded
        // silently either; the error is returned once they are done.
        for event in rx {
            if append_error.is_none()
                && let Err(error) = turn.append(event)
            {
                append_error = Some(error);
            }
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(failure(RunOutcome::Error, "a branch panicked")))
            })
            .collect()
    });
    if let Some(error) = append_error {
        return Err(error);
    }

    if let Some((outcome, reason)) = results.iter().find_map(|r| r.as_ref().err()) {
        return super::finish_run(turn, &state, *outcome, reason.clone()).map(Step::Exit);
    }

    let join_pass = state.node_passes.get(fork.join).copied().unwrap_or(0) + 1;
    turn.append(Event::WorkflowNodeStarted {
        run_id: state.run_id.clone(),
        node_id: fork.join.to_string(),
        pass: join_pass,
        via: None,
        child_session_id: None,
    })?;
    let join_output = join_output(&state, fork.entries, &results);
    turn.append(Event::WorkflowNodeFinished {
        run_id: state.run_id.clone(),
        node_id: fork.join.to_string(),
        port: "next".into(),
        output: join_output,
        exit_code: None,
        comment: None,
    })?;

    Ok(Step::Go(Cursor::After {
        from: fork.join.to_string(),
        port: "next".into(),
    }))
}

struct Branch<'a> {
    env: &'a WorkflowEnv<'a>,
    state: RunState,
    files: &'a [String],
    parent_session_id: &'a str,
    stop: &'a AtomicBool,
    via: Via,
    entry: String,
}

fn walk_branch(mut branch: Branch<'_>, tx: Sender<Event>) -> BranchResult {
    let mut last: Option<(String, String)> = None;
    let mut last_output = String::new();

    loop {
        if branch.stop.load(Ordering::Relaxed) {
            return Err(failure(RunOutcome::Stopped, "stopped"));
        }

        let next = match &last {
            None => run::enter(&branch.state, branch.via.clone(), &branch.entry, true),
            Some((node_id, port)) => run::next_after(&branch.state, node_id, port, true),
        };

        match next {
            Next::Run { node_id, via } => {
                let end = run_branch_node(&mut branch, &node_id, &via, &tx)?;
                let NodeEnd::Finished { port, output, .. } = end else {
                    return Err(branch_failure(end));
                };
                last_output = output;
                last = Some((node_id, port));
            }
            Next::ReachJoin { .. } => return Ok(last_output),
            Next::Finish { outcome, reason } => return Err((outcome, reason)),
            // The validation forbids both; a run frozen before it changed may not.
            Next::Pause(_) => return Err(failure(RunOutcome::Error, "a branch cannot pause")),
            Next::Fork { .. } => {
                return Err(failure(
                    RunOutcome::Error,
                    "nested parallel nodes are not supported",
                ));
            }
        }
    }
}

fn run_branch_node(
    branch: &mut Branch<'_>,
    node_id: &str,
    via: &Via,
    tx: &Sender<Event>,
) -> Result<NodeEnd, Failure> {
    // An owned node: the state it comes from is updated while it runs.
    let node = branch
        .state
        .workflow
        .node(node_id)
        .cloned()
        .ok_or_else(|| failure(RunOutcome::Error, &format!("{node_id}: unknown node")))?;
    let pass = branch.state.node_passes.get(node_id).copied().unwrap_or(0) + 1;
    let child_session_id = match node.kind {
        NodeKind::Agent { .. } => {
            let title = nodes::child_title(&branch.state, &node);
            let child = branch.env.children.create(branch.parent_session_id, &title);
            Some(child.map_err(|e| failure(RunOutcome::Error, &e))?)
        }
        _ => None,
    };
    let started = Event::WorkflowNodeStarted {
        run_id: branch.state.run_id.clone(),
        node_id: node_id.to_string(),
        pass,
        via: Some(via.clone()),
        child_session_id: child_session_id.clone(),
    };
    emit(&mut branch.state, tx, started);

    let input = nodes::NodeInput {
        state: &branch.state,
        node: &node,
        pass,
        files: branch.files,
        stop: branch.stop,
    };
    let end = match (&node.kind, &child_session_id) {
        (NodeKind::Agent { agent, outputs, .. }, Some(child)) => {
            nodes::fresh_agent(branch.env, &input, agent, outputs, child)
                .map_err(|e| failure(RunOutcome::Error, &e))?
        }
        (NodeKind::Shell { .. }, _) => nodes::shell(branch.env, &input),
        (NodeKind::Condition { .. }, _) => nodes::condition(&input),
        _ => {
            let reason = format!("{node_id}: cannot run in a branch");
            return Err(failure(RunOutcome::Error, &reason));
        }
    };

    if let NodeEnd::Finished {
        port,
        output,
        exit_code,
    } = &end
    {
        let finished = Event::WorkflowNodeFinished {
            run_id: branch.state.run_id.clone(),
            node_id: node_id.to_string(),
            port: port.clone(),
            output: output.clone(),
            exit_code: *exit_code,
            comment: None,
        };
        emit(&mut branch.state, tx, finished);
    }
    Ok(end)
}

/// Why a branch ended the run.
type Failure = (RunOutcome, Option<String>);
type BranchResult = Result<String, Failure>;

fn failure(outcome: RunOutcome, reason: &str) -> Failure {
    (outcome, Some(reason.to_string()))
}

fn emit(state: &mut RunState, tx: &Sender<Event>, event: Event) {
    state.apply(&event);
    let _ = tx.send(event);
}

fn branch_failure(end: NodeEnd) -> Failure {
    match end {
        NodeEnd::EndRun { outcome, reason } => (outcome, reason),
        NodeEnd::AgentAsked => (
            RunOutcome::Blocked,
            Some("a branch agent asked a question".into()),
        ),
        NodeEnd::Finished { .. } => (
            RunOutcome::Error,
            Some("a branch node finished without reaching the join".into()),
        ),
    }
}

fn join_output(state: &RunState, entries: &[(Via, String)], results: &[BranchResult]) -> String {
    let mut parts = Vec::new();
    for ((_, entry), result) in entries.iter().zip(results) {
        let Ok(output) = result else {
            continue;
        };
        let label = state
            .workflow
            .node(entry)
            .map(|n| n.label().to_string())
            .unwrap_or_else(|| entry.clone());
        parts.push(format!("### {label}\n\n{output}"));
    }
    parts.join("\n\n")
}
