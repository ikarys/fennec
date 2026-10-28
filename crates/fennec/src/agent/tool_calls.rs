//! Runs the model's tool calls and records their results. `ask_user` is
//! special: it pauses the turn, so the calls after it in the batch are not
//! run, and the turn ends waiting for the human.

use super::ask_user;
use super::propose_criteria;
use super::record_decision;
use super::turn::Turn;
use crate::domain::agent::AgentDefinition;
use crate::domain::criteria;
use crate::domain::event::{Event, ToolCall, ToolResult};
use crate::domain::workflow::run;
use std::sync::atomic::Ordering;

/// openfox `INTERRUPTED_ERROR`: a call the human stopped before it ran.
pub(super) const INTERRUPTED_ERROR: &str = "Tool execution was interrupted by user";
/// A call queued after an `ask_user` that paused the turn.
const NOT_RUN_AFTER_ASK: &str =
    "Not run: the turn paused at ask_user. Call it again after the answer if it is still needed.";

pub(super) enum Flow {
    Continue,
    WaitingForUser,
}

fn record(
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

/// Port of openfox `executeTools`, sequential for now.
pub(super) fn execute_tools(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    calls: &[ToolCall],
) -> Result<Flow, String> {
    let mut waiting = false;
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
        // An `ask_user` earlier in this batch paused the turn.
        if waiting {
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
        // openfox `createRegistryFromTools` permission check: the mode is
        // enforced here, at execution, so plan stays read-only.
        if !agent.allows(&call.name) {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(agent.denied(&call.name)),
            )?;
            continue;
        }
        if call.name == ask_user::NAME {
            match ask_user::parse(&call.arguments) {
                // The model reads the failure and the loop goes on.
                Err(e) => record(turn, message_id, call, ToolResult::failure(e))?,
                // No result is recorded: the answer will be the result.
                Ok(q) => {
                    turn.append(Event::ChatAskUser {
                        call_id: call.id.clone(),
                        question: q.question,
                        ask_type: q.ask_type,
                        options: q.options,
                    })?;
                    waiting = true;
                }
            }
            continue;
        }
        if call.name == propose_criteria::NAME {
            match propose_criteria::parse(&call.arguments) {
                // The model reads the failure and the loop goes on.
                Err(e) => record(turn, message_id, call, ToolResult::failure(e))?,
                // The question shows the delta against the criteria in force;
                // a proposal that changes nothing is accepted without asking.
                Ok(proposal) => {
                    let in_force = run::criteria_in_force(&turn.log.events(turn.session_id)?);
                    if criteria::is_noop(&in_force, &proposal.criteria) {
                        turn.append(Event::CriteriaProposed {
                            call_id: call.id.clone(),
                            criteria: proposal.criteria.clone(),
                            reason: proposal.reason.clone(),
                        })?;
                        record(
                            turn,
                            message_id,
                            call,
                            ToolResult::success(
                                "The criteria are unchanged; nothing to accept.".into(),
                                false,
                            ),
                        )?;
                    } else {
                        for event in propose_criteria::events(&call.id, proposal, &in_force) {
                            turn.append(event)?;
                        }
                        waiting = true;
                    }
                }
            }
            continue;
        }
        if call.name == record_decision::NAME {
            // Not blocking: the result is recorded and the turn goes on.
            let result = match record_decision::parse(&call.arguments) {
                Err(e) => ToolResult::failure(e),
                Ok(decision) => {
                    turn.append(record_decision::event(&call.id, decision))?;
                    ToolResult::success(record_decision::RECORDED.into(), false)
                }
            };
            record(turn, message_id, call, result)?;
            continue;
        }
        let (tools, stop) = (turn.tools, turn.stop);
        let mut log_error = None;
        let r = tools.execute(
            &call.name,
            &call.arguments,
            agent.commands,
            stop,
            &mut |stream, chunk| {
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
            },
        );
        if let Some(e) = log_error {
            return Err(e);
        }
        record(turn, message_id, call, r)?;
    }
    Ok(if waiting {
        Flow::WaitingForUser
    } else {
        Flow::Continue
    })
}
