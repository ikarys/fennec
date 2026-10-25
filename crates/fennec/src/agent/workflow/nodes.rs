//! One node run, shared by the main walk and parallel branches.

use super::WorkflowEnv;
use super::outcome;
use crate::domain::event::DoneReason;
use crate::domain::workflow::definition::{DEFAULT_SHELL_TIMEOUT_SECS, Node, NodeKind};
use crate::domain::workflow::run::{self as run, RunOutcome};
use crate::domain::workflow::template::render;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// How a node run ended: its port and output, or the end of the run.
pub(super) enum NodeEnd {
    Finished {
        port: String,
        output: String,
        exit_code: Option<i32>,
    },
    /// The run ends here (no WorkflowNodeFinished is recorded).
    EndRun {
        outcome: RunOutcome,
        reason: Option<String>,
    },
    /// A session-context agent asked the human (ask_user): the turn ends waiting.
    AgentAsked,
}

/// What every node runner reads: the folded run (its start already applied),
/// the files changed in the session, the stop flag.
pub(super) struct NodeInput<'a> {
    pub state: &'a run::RunState,
    pub node: &'a Node,
    pub pass: u32,
    pub files: &'a [String],
    pub stop: &'a AtomicBool,
}

pub(super) fn shell(env: &WorkflowEnv, input: &NodeInput) -> NodeEnd {
    let NodeKind::Shell {
        command,
        timeout_secs,
        success_codes,
    } = &input.node.kind
    else {
        return NodeEnd::EndRun {
            outcome: RunOutcome::Error,
            reason: Some("not a shell node".into()),
        };
    };

    let cmd = render(
        command,
        &|name| run::variable(input.state, &input.node.id, input.files, name),
        true,
    );
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_SHELL_TIMEOUT_SECS));
    let result = env.commands.run(&cmd, timeout, input.stop);

    if result.stopped {
        return NodeEnd::EndRun {
            outcome: RunOutcome::Stopped,
            reason: None,
        };
    }

    let codes = success_codes.as_deref().unwrap_or(&[0]);
    let port = if result.exit_code.is_some_and(|c| codes.contains(&c)) {
        "success"
    } else {
        "failure"
    };

    NodeEnd::Finished {
        port: port.into(),
        output: outcome::last_8kib(&result.output),
        exit_code: result.exit_code,
    }
}

pub(super) fn condition(input: &NodeInput) -> NodeEnd {
    let NodeKind::Condition {
        value,
        test,
        against,
    } = &input.node.kind
    else {
        return NodeEnd::EndRun {
            outcome: RunOutcome::Error,
            reason: Some("not a condition node".into()),
        };
    };

    let rendered = render(
        value,
        &|name| run::variable(input.state, &input.node.id, input.files, name),
        false,
    );
    let result = run::evaluate(*test, &rendered, against.as_deref());
    let port = if result { "true" } else { "false" };

    NodeEnd::Finished {
        port: port.into(),
        output: rendered,
        exit_code: None,
    }
}

/// The prompt of an agent node: the domain's agent_prompt + the outcome instruction.
pub(super) fn agent_prompt(input: &NodeInput, outputs: &[String]) -> String {
    let mut prompt = run::agent_prompt(input.state, input.node, input.pass, input.files);
    prompt.push_str(&outcome::outcome_instruction(outputs));
    prompt
}

/// The title of the child session a node runs in.
pub(super) fn child_title(state: &run::RunState, node: &Node) -> String {
    format!("{} · {}", state.workflow.name, node.label())
}

/// Turns an agent's end into a NodeEnd; `ask_again` sends the outcome reminder
/// once to the same agent (same session or child) and returns its end.
pub(super) fn agent_end(
    label: &str,
    outputs: &[String],
    reason: DoneReason,
    text: String,
    ask_again: impl FnOnce(&str) -> Result<(DoneReason, String), String>,
) -> Result<NodeEnd, String> {
    if let Some(end) = unfinished(label, reason) {
        return Ok(end);
    }
    if let Some(end) = finished(outputs, &text) {
        return Ok(end);
    }
    let reminder = format!(
        "End your answer with one line: Outcome: <one of: {}>",
        outputs.join(", ")
    );
    let (reason, text) = ask_again(&reminder)?;
    if let Some(end) = unfinished(label, reason) {
        return Ok(end);
    }
    Ok(finished(outputs, &text).unwrap_or_else(|| NodeEnd::EndRun {
        outcome: RunOutcome::Blocked,
        reason: Some(format!(
            "\"{label}\" gave no outcome among: {}",
            outputs.join(", ")
        )),
    }))
}

/// The end of a turn that did not complete; None when it did.
fn unfinished(label: &str, reason: DoneReason) -> Option<NodeEnd> {
    match reason {
        DoneReason::Complete => None,
        DoneReason::WaitingForUser => Some(NodeEnd::AgentAsked),
        DoneReason::Stopped => Some(NodeEnd::EndRun {
            outcome: RunOutcome::Stopped,
            reason: None,
        }),
        DoneReason::Error | DoneReason::Truncated => Some(NodeEnd::EndRun {
            outcome: RunOutcome::Error,
            reason: Some(format!("\"{label}\" ended: {reason}")),
        }),
    }
}

/// The port and output of a completed agent: its only output, or the one its
/// outcome line names; None when it named none.
fn finished(outputs: &[String], text: &str) -> Option<NodeEnd> {
    let (port, output) = match outputs {
        [only] => (only.clone(), text.trim().to_string()),
        _ => (
            outcome::parse_outcome(text, outputs)?,
            outcome::strip_outcome_line(text),
        ),
    };
    Some(NodeEnd::Finished {
        port,
        output,
        exit_code: None,
    })
}

/// A fresh agent node: everything through env.children (used by the walk for
/// `context: fresh` and by every branch agent node).
pub(super) fn fresh_agent(
    env: &WorkflowEnv,
    input: &NodeInput,
    agent: &str,
    outputs: &[String],
    child_session_id: &str,
) -> Result<NodeEnd, String> {
    let prompt = agent_prompt(input, outputs);
    let run = env
        .children
        .send(child_session_id, agent, &prompt, input.stop)?;
    let label = input.node.label();

    match agent_end(label, outputs, run.reason, run.text, |reminder| {
        let run = env
            .children
            .send(child_session_id, agent, reminder, input.stop)?;
        Ok((run.reason, run.text))
    })? {
        NodeEnd::AgentAsked => Ok(NodeEnd::EndRun {
            outcome: RunOutcome::Blocked,
            reason: Some(format!("\"{label}\" asked a question in a child session")),
        }),
        end => Ok(end),
    }
}
