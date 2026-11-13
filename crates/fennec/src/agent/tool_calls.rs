//! Runs the model's tool calls and records their results. `ask_user` is
//! special: it pauses the turn, so the calls after it in the batch are not
//! run, and the turn ends waiting for the human. `propose_plan` and
//! `submit_report` end the turn the same way: the plan or report is
//! recorded, the calls after it in the batch are not run, and the turn ends
//! complete.

use super::own_tools::{self, Handled};
use super::rule_check;
use super::turn::Turn;
use crate::agent::ports::RuleCheck;
use crate::domain::agent::{AgentDefinition, WriteScope};
use crate::domain::event::{Event, ToolCall, ToolResult};
use crate::domain::mcp::META_TOOLS;
use crate::domain::named_paths::{human_texts, named_by_human};
use crate::domain::test_files;
use std::sync::atomic::Ordering;

/// `INTERRUPTED_ERROR`: a call the human stopped before it ran.
pub(super) const INTERRUPTED_ERROR: &str = "Tool execution was interrupted by user";
/// A call queued after an `ask_user` that paused the turn.
const NOT_RUN_AFTER_ASK: &str = "Not run: the turn paused for the human (a question or an approval). Call it again after the answer if it is still needed.";

/// A default read ask can be skipped only when the human already named the path.
fn named_by_the_human(turn: &Turn, check: &RuleCheck) -> Result<bool, String> {
    let Some(path) = rule_check::named_read(check) else {
        return Ok(false);
    };

    let events = turn.log.events(turn.session_id)?;
    let texts = human_texts(&events);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);

    Ok(named_by_human(path, &texts, home.as_deref()))
}

pub(super) enum Flow {
    Continue,
    WaitingForUser,
    /// `propose_plan` recorded the plan, or `submit_report` the report:
    /// the turn ends complete.
    Done,
}

pub(super) fn record(
    turn: &mut Turn,
    message_id: &str,
    call: &ToolCall,
    result: ToolResult,
) -> Result<(), String> {
    turn.append(Event::ToolResult {
        message_id: message_id.to_string(),
        tool_call_id: call.id.clone(),
        result,
    })
}

/// Runs one call, streaming its output to the log; `approved` is the human's
/// "once" for a call fennec paused.
pub(super) fn run_tool(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    call: &ToolCall,
    approved: bool,
) -> Result<ToolResult, String> {
    let tools = turn.tools;
    let stop = turn.stop;
    let mut log_error = None;
    let mut on_output = |stream, chunk: &str| {
        if log_error.is_none() {
            log_error = turn
                .append(Event::ToolOutput {
                    message_id: message_id.into(),
                    tool_call_id: call.id.clone(),
                    stream,
                    content: chunk.into(),
                })
                .err();
        }
    };
    let (name, args, access) = (&call.name, &call.arguments, agent.commands);
    if let Some(refusal) = scope_refusal(agent, call) {
        return Ok(ToolResult::failure(refusal));
    }
    let result = if approved {
        tools.execute_approved(name, args, access, stop, &mut on_output)
    } else {
        tools.execute(name, args, access, stop, &mut on_output)
    };
    match log_error {
        Some(e) => Err(e),
        None => Ok(result),
    }
}

/// The refusal for a file write outside an agent's `writes` scope, if any.
fn scope_refusal(agent: &AgentDefinition, call: &ToolCall) -> Option<String> {
    if agent.writes != WriteScope::Tests {
        return None;
    }
    if !matches!(call.name.as_str(), "write_file" | "edit_file") {
        return None;
    }
    let path = call
        .arguments
        .get("path")
        .and_then(|value| value.as_str())?;
    let path = path.strip_prefix("./").unwrap_or(path);
    if test_files::is_test_path(path) {
        None
    } else {
        Some(format!(
            "{path} is not a test file: this agent writes only the tests; the code comes after the human validates them."
        ))
    }
}

/// A Toolbox call: the human's rules first (the model's call never reaches a refused tool), then the run.
fn run_checked(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    call: &ToolCall,
) -> Result<Handled, String> {
    let tools = turn.tools;
    let check = tools.check_rules(&call.name, &call.arguments);
    if !turn.autonomous
        && let rules::Decision::Ask(rule) = check.decision
    {
        if rule.is_none() && named_by_the_human(turn, &check)? {
            let r = run_tool(turn, agent, message_id, call, true)?;
            return Ok(Handled::Result(r));
        }

        turn.append(rule_check::approval(&call.id, rule, &check))?;
        return Ok(Handled::Waiting);
    }
    if let Some(rule) = rule_check::refusal(&check) {
        turn.append(rule_check::event(&call.id, rule, &check.subject))?;
        return Ok(Handled::Result(ToolResult::failure(rule.refusal())));
    }
    let r = run_tool(turn, agent, message_id, call, false)?;
    Ok(Handled::Result(r))
}

/// Runs the round's tool calls, sequential for now.
pub(super) fn execute_tools(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    calls: &[ToolCall],
) -> Result<Flow, String> {
    let mut waiting = false;
    let mut done = false;
    // MCP is a human-configured capability, not part of the agent's fixed
    // allowed list: allow `mcp_tools` and `mcp_call` here when it is offered.
    let offers_mcp = turn.tools.offers_mcp();
    for call in calls {
        // The human pressed stop: a `write_file` queued after the stopped
        // command must not run, so record a failure without running it.
        if turn.stop.load(Ordering::Relaxed) {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(INTERRUPTED_ERROR.into()),
            )?;
            continue;
        }
        // An `ask_user` earlier in this batch paused the turn, or a
        // `propose_plan` / `submit_report` ended it.
        if waiting || done {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(NOT_RUN_AFTER_ASK.into()),
            )?;
            continue;
        }
        if let Some(pe) = &call.parse_error {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(format!(
                    "Failed to parse tool call arguments: {pe}. Please ensure your JSON function call arguments are valid."
                )),
            )?;
            continue;
        }
        // The agent's allow-list check: the mode is
        // enforced here, at execution, so plan stays read-only.
        if !agent.allows(&call.name) && !(offers_mcp && META_TOOLS.contains(&call.name.as_str())) {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(agent.denied(&call.name)),
            )?;
            continue;
        }
        let handled = match own_tools::handle(turn, agent, message_id, call)? {
            Some(handled) => handled,
            None => run_checked(turn, agent, message_id, call)?,
        };
        match handled {
            Handled::Result(result) => record(turn, message_id, call, result)?,
            Handled::Waiting => waiting = true,
            Handled::Done(result) => {
                record(turn, message_id, call, result)?;
                done = true;
            }
        }
    }
    Ok(if waiting {
        Flow::WaitingForUser
    } else if done {
        Flow::Done
    } else {
        Flow::Continue
    })
}

#[cfg(test)]
mod tests;
