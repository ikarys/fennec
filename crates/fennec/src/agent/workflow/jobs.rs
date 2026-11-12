//! What the human asks of a run before the walk goes on: start, answer, continue, resume, stop.

use super::criteria;
use super::{Cursor, finish_run};
use crate::agent::turn::Turn;
use crate::domain::attachment::Attachment;
use crate::domain::clock::now_ms;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;
use crate::domain::workflow::definition::{NodeKind, Workflow};
use crate::domain::workflow::run::{self as run, RunOutcome, Via};
use crate::domain::workflow::validate;

pub(super) fn start_run(
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

pub(super) fn start_from_plan(
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

    // A task readied from its sheet starts in an empty session: nothing above
    // holds the plan the session-context nodes ("Execute the plan above") read.
    let empty = !turn
        .log
        .events(turn.session_id)?
        .iter()
        .any(|event| matches!(event, Event::MessageStart { .. }));
    if empty {
        turn.append_user(
            &format!("{task}\n\n{plan}"),
            false,
            None,
            attachments.to_vec(),
        )?;
        start_run(turn, workflow, task, false, &[])?;
    } else {
        start_run(turn, workflow, task, false, attachments)?;
    }
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

pub(super) fn answer_human(turn: &mut Turn, choice: &str, comment: &str) -> Result<Cursor, String> {
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

    // The kernel, not the model, decides which tests are protected: at the
    // human's click, the test files written so far are frozen for the session.
    if choices.first().is_some_and(|first| first == choice)
        && fed_by_tests_agent(&state.workflow, &open.node_id)
    {
        turn.append(Event::TestsApproved {
            run_id: state.run_id.clone(),
            files: crate::domain::test_files::written(&events),
        })?;
    }

    Ok(Cursor::After {
        from: open.node_id.clone(),
        port: choice.to_string(),
    })
}

/// Whether `node_id` is fed by the `done` edge of the `tests` agent node: the
/// human node that validates the tests written before the code.
fn fed_by_tests_agent(workflow: &Workflow, node_id: &str) -> bool {
    workflow.nodes.iter().any(|node| {
        matches!(&node.kind, NodeKind::Agent { agent, .. } if agent == "tests")
            && workflow
                .edge(&node.id, "done")
                .is_some_and(|edge| edge.to == node_id)
    })
}

pub(super) fn continue_paused(turn: &mut Turn, comment: &str) -> Result<Cursor, String> {
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

pub(super) fn resume_interrupted(turn: &mut Turn) -> Result<Cursor, String> {
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
            .unwrap_or_else(|| state.default_via());
        (open.node_id.clone(), via)
    } else {
        // A run that ended in error: re-enter the node that was open when it
        // ended, on a new pass.
        match state.finished {
            Some((RunOutcome::Error, _)) => {
                let (node_id, via) = run::open_at_finish(&events, &state.run_id)
                    .ok_or("the run ended in error but no node can be re-run")?;
                if state.workflow.in_branch(&node_id) {
                    return Err(
                        "the failed node is in a branch, it cannot be relaunched alone".into(),
                    );
                }
                (node_id, via.unwrap_or_else(|| state.default_via()))
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

pub(super) fn stop_run(turn: &mut Turn) -> Result<DoneReason, String> {
    let events = turn.log.events(turn.session_id)?;
    let state = run::current_run(&events).ok_or("no active workflow run")?;
    finish_run(
        turn,
        &state,
        RunOutcome::Stopped,
        Some("stopped by the human".into()),
    )
}
