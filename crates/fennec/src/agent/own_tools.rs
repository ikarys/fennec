//! The loop's own tools: calls fennec answers itself instead of the Toolbox (questions, waivers, criteria, decisions, tasks, plans, reports, agent suggestions, delegation).

use super::ask_user;
use super::delegate;
use super::preflight;
use super::propose_criteria;
use super::propose_plan;
use super::propose_task;
use super::record_decision;
use super::request_waiver;
use super::submit_report;
use super::suggest_agent;
use super::turn::Turn;
use crate::domain::agent::AgentDefinition;
use crate::domain::criteria;
use crate::domain::event::{Event, ToolCall, ToolResult};
use crate::domain::workflow::run;

/// `ask_user`'s result while the human is away (`Turn::autonomous`).
pub(super) const AWAY_ANSWER: &str = "The human is away (autonomous run) and cannot answer. Decide yourself, choosing the safest option that stays within the approved plan, record the question and your choice with record_decision, and continue.";
/// `propose_criteria`'s result while the human is away.
pub(super) const AWAY_CRITERIA: &str = "The human is away (autonomous run): the criteria stay as approved. Record your proposed change with record_decision for the human to review, and continue against the current criteria.";
/// `request_waiver`'s result while the human is away.
pub(super) const AWAY_WAIVER: &str = "The human is away (autonomous run): a protected file cannot be waived. Say in your report what should change there and why, and continue.";

/// What a call the loop handled itself leaves for the batch.
pub(super) enum Handled {
    /// Recorded as the call's result; the batch goes on.
    Result(ToolResult),
    /// The turn waits for the human; no result yet (their answer will be it).
    Waiting,
    /// Recorded as the call's result; the turn ends complete (plan or report recorded).
    Done(ToolResult),
}

/// The call's outcome when it is one of the loop's own tools, `None` otherwise.
pub(super) fn handle(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    call: &ToolCall,
) -> Result<Option<Handled>, String> {
    match call.name.as_str() {
        ask_user::NAME => Ok(Some(ask(turn, call)?)),
        request_waiver::NAME => Ok(Some(waiver(turn, call)?)),
        propose_criteria::NAME => Ok(Some(criteria(turn, call)?)),
        record_decision::NAME => Ok(Some(decision(turn, call)?)),
        propose_task::NAME => Ok(Some(task(turn, call)?)),
        propose_plan::NAME => Ok(Some(plan(turn, agent, message_id, call)?)),
        submit_report::NAME => Ok(Some(report(turn, call)?)),
        suggest_agent::NAME => Ok(Some(suggestion(turn, agent, call)?)),
        delegate::NAME => Ok(Some(delegation(turn, call)?)),
        _ => Ok(None),
    }
}

fn ask(turn: &mut Turn, call: &ToolCall) -> Result<Handled, String> {
    match ask_user::parse(&call.arguments) {
        // The model reads the failure and the loop goes on.
        Err(e) => Ok(Handled::Result(ToolResult::failure(e))),
        // Nobody would answer before morning: the agent decides.
        Ok(_) if turn.autonomous => Ok(Handled::Result(ToolResult::success(
            AWAY_ANSWER.into(),
            false,
        ))),
        // No result is recorded: the answer will be the result.
        Ok(q) => {
            turn.append(Event::ChatAskUser {
                call_id: call.id.clone(),
                question: q.question,
                ask_type: q.ask_type,
                options: q.options,
            })?;
            Ok(Handled::Waiting)
        }
    }
}

fn waiver(turn: &mut Turn, call: &ToolCall) -> Result<Handled, String> {
    match request_waiver::parse(&call.arguments) {
        // The model reads the failure and the loop goes on.
        Err(e) => Ok(Handled::Result(ToolResult::failure(e))),
        // A waiver is the human's decision; while they are away it is refused.
        Ok(_) if turn.autonomous => Ok(Handled::Result(ToolResult::failure(AWAY_WAIVER.into()))),
        // No result is recorded: the human's answer will be the result.
        Ok((path, reason)) => {
            turn.append(Event::WaiverRequested {
                call_id: call.id.clone(),
                path,
                reason,
            })?;
            Ok(Handled::Waiting)
        }
    }
}

fn criteria(turn: &mut Turn, call: &ToolCall) -> Result<Handled, String> {
    match propose_criteria::parse(&call.arguments) {
        // The model reads the failure and the loop goes on.
        Err(e) => Ok(Handled::Result(ToolResult::failure(e))),
        // The contract is the human's: it never changes while they are away.
        Ok(_) if turn.autonomous => Ok(Handled::Result(ToolResult::failure(AWAY_CRITERIA.into()))),
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
                Ok(Handled::Result(ToolResult::success(
                    "The criteria are unchanged; nothing to accept.".into(),
                    false,
                )))
            } else {
                for event in propose_criteria::events(&call.id, proposal, &in_force, turn.lang) {
                    turn.append(event)?;
                }
                Ok(Handled::Waiting)
            }
        }
    }
}

fn decision(turn: &mut Turn, call: &ToolCall) -> Result<Handled, String> {
    // Not blocking: the result is recorded and the turn goes on.
    let result = match record_decision::parse(&call.arguments) {
        Err(e) => ToolResult::failure(e),
        Ok(decision) => {
            turn.append(record_decision::event(&call.id, decision))?;
            ToolResult::success(record_decision::RECORDED.into(), false)
        }
    };
    Ok(Handled::Result(result))
}

fn task(turn: &mut Turn, call: &ToolCall) -> Result<Handled, String> {
    // Not blocking: the proposal is recorded and the turn goes on.
    let result = match propose_task::parse(&call.arguments) {
        Err(e) => ToolResult::failure(e),
        Ok(proposal) => {
            turn.append(propose_task::event(&call.id, proposal))?;
            ToolResult::success(propose_task::PROPOSED.into(), false)
        }
    };
    Ok(Handled::Result(result))
}

fn plan(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    call: &ToolCall,
) -> Result<Handled, String> {
    match propose_plan::parse(&call.arguments) {
        // The model reads the failure and the loop goes on.
        Err(e) => Ok(Handled::Result(ToolResult::failure(e))),
        Ok(plan) => {
            // The plan's checks run before the human sees the plan,
            // once per turn: sent back as this tool's error when one
            // cannot run or already passes, so the model fixes the
            // plan and calls the tool again.
            if !turn.preflight_sent_back
                && let Some(feedback) = preflight::run(turn, agent, message_id, &plan.criteria())?
            {
                turn.preflight_sent_back = true;
                Ok(Handled::Result(ToolResult::failure(feedback)))
            } else {
                // While the human is away the plan is still recorded: a
                // workflow's plan node must get its output.
                turn.append(propose_plan::event(&call.id, plan))?;
                Ok(Handled::Done(ToolResult::success(
                    propose_plan::RECORDED.into(),
                    false,
                )))
            }
        }
    }
}

fn report(turn: &mut Turn, call: &ToolCall) -> Result<Handled, String> {
    match submit_report::parse(&call.arguments) {
        // The model reads the failure and the loop goes on.
        Err(e) => Ok(Handled::Result(ToolResult::failure(e))),
        Ok(report) => {
            // The report is the turn's end: no preflight, no second
            // chance — the human reviews what the builder claims.
            turn.append(submit_report::event(&call.id, report))?;
            Ok(Handled::Done(ToolResult::success(
                submit_report::RECORDED.into(),
                false,
            )))
        }
    }
}

fn suggestion(
    turn: &mut Turn,
    agent: &AgentDefinition,
    call: &ToolCall,
) -> Result<Handled, String> {
    // Not blocking: the suggestion is recorded and the turn goes on.
    let current = agent.id.clone();
    let result = match suggest_agent::parse(&call.arguments, turn.agents, &current) {
        Err(e) => ToolResult::failure(e),
        Ok(suggestion) => {
            turn.append(suggest_agent::event(&call.id, suggestion))?;
            ToolResult::success(suggest_agent::SUGGESTED.into(), false)
        }
    };
    Ok(Handled::Result(result))
}

fn delegation(turn: &mut Turn, call: &ToolCall) -> Result<Handled, String> {
    // Blocking: the child runs to its turn's end, then the loop goes on.
    let result = delegate::run(turn, &call.arguments);
    Ok(Handled::Result(result))
}
