//! Runs the model's tool calls and records their results. `ask_user` is
//! special: it pauses the turn, so the calls after it in the batch are not
//! run, and the turn ends waiting for the human.

use super::ask_user;
use super::propose_criteria;
use super::propose_task;
use super::record_decision;
use super::request_waiver;
use super::rule_check;
use super::suggest_agent;
use super::turn::Turn;
use crate::domain::agent::AgentDefinition;
use crate::domain::criteria;
use crate::domain::event::{Event, ToolCall, ToolResult};
use crate::domain::workflow::run;
use std::sync::atomic::Ordering;

/// openfox `INTERRUPTED_ERROR`: a call the human stopped before it ran.
pub(super) const INTERRUPTED_ERROR: &str = "Tool execution was interrupted by user";
/// A call queued after an `ask_user` that paused the turn.
/// `ask_user`'s result while the human is away (`Turn::autonomous`).
const AWAY_ANSWER: &str = "The human is away (autonomous run) and cannot answer. Decide yourself, choosing the safest option that stays within the approved plan, record the question and your choice with record_decision, and continue.";
/// `propose_criteria`'s result while the human is away.
const AWAY_CRITERIA: &str = "The human is away (autonomous run): the criteria stay as approved. Record your proposed change with record_decision for the human to review, and continue against the current criteria.";
/// `request_waiver`'s result while the human is away.
const AWAY_WAIVER: &str = "The human is away (autonomous run): a protected file cannot be waived. Say in your report what should change there and why, and continue.";
const NOT_RUN_AFTER_ASK: &str = "Not run: the turn paused for the human (a question or an approval). Call it again after the answer if it is still needed.";

pub(super) enum Flow {
    Continue,
    WaitingForUser,
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

/// Port of openfox `executeTools`, sequential for now.
pub(super) fn execute_tools(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    calls: &[ToolCall],
) -> Result<Flow, String> {
    let mut waiting = false;
    // MCP tools are a human-configured capability, not part of the agent's
    // fixed allowed list: allow them here so a `{server}_{tool}` call runs.
    let mcp_tools = turn.tools.mcp_tool_names();
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
        if !agent.allows(&call.name) && !mcp_tools.contains(&call.name) {
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
                // Nobody would answer before morning: the agent decides.
                Ok(_) if turn.autonomous => record(
                    turn,
                    message_id,
                    call,
                    ToolResult::success(AWAY_ANSWER.into(), false),
                )?,
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
        if call.name == request_waiver::NAME {
            match request_waiver::parse(&call.arguments) {
                // The model reads the failure and the loop goes on.
                Err(e) => record(turn, message_id, call, ToolResult::failure(e))?,
                // A waiver is the human's decision; while they are away it is refused.
                Ok(_) if turn.autonomous => record(
                    turn,
                    message_id,
                    call,
                    ToolResult::failure(AWAY_WAIVER.into()),
                )?,
                // No result is recorded: the human's answer will be the result.
                Ok((path, reason)) => {
                    turn.append(Event::WaiverRequested {
                        call_id: call.id.clone(),
                        path,
                        reason,
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
                // The contract is the human's: it never changes while they are away.
                Ok(_) if turn.autonomous => record(
                    turn,
                    message_id,
                    call,
                    ToolResult::failure(AWAY_CRITERIA.into()),
                )?,
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
        if call.name == propose_task::NAME {
            // Not blocking: the proposal is recorded and the turn goes on.
            let result = match propose_task::parse(&call.arguments) {
                Err(e) => ToolResult::failure(e),
                Ok(proposal) => {
                    turn.append(propose_task::event(&call.id, proposal))?;
                    ToolResult::success(propose_task::PROPOSED.into(), false)
                }
            };
            record(turn, message_id, call, result)?;
            continue;
        }

        if call.name == suggest_agent::NAME {
            // Not blocking: the suggestion is recorded and the turn goes on.
            let current = agent.id.clone();
            let result = match suggest_agent::parse(&call.arguments, turn.agents, &current) {
                Err(e) => ToolResult::failure(e),
                Ok(suggestion) => {
                    turn.append(suggest_agent::event(&call.id, suggestion))?;
                    ToolResult::success(suggest_agent::SUGGESTED.into(), false)
                }
            };
            record(turn, message_id, call, result)?;
            continue;
        }
        // The human's rules run before the tool: the model's call never reaches it.
        let tools = turn.tools;
        let check = tools.check_rules(&call.name, &call.arguments);
        if !turn.autonomous
            && let rules::Decision::Ask(rule) = check.decision
        {
            turn.append(rule_check::approval(&call.id, rule, &check))?;
            waiting = true; // no result: the human's answer will produce it
            continue;
        }
        if let Some(rule) = rule_check::refusal(&check) {
            turn.append(rule_check::event(&call.id, rule, &check.subject))?;
            record(turn, message_id, call, ToolResult::failure(rule.refusal()))?;
            continue;
        }
        let r = run_tool(turn, agent, message_id, call, false)?;
        record(turn, message_id, call, r)?;
    }
    Ok(if waiting {
        Flow::WaitingForUser
    } else {
        Flow::Continue
    })
}

#[cfg(test)]
mod tests {
    use super::execute_tools;
    use crate::agent::ports::Toolbox;
    use crate::agent::test_support::{MemoryLog, Scripted, builder};
    use crate::domain::agent::CommandAccess;
    use crate::domain::event::{Event, OutputStream, ToolCall, ToolResult};
    use serde_json::{Value, json};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::AtomicBool;

    /// A toolbox that offers one MCP tool and records every call it runs.
    struct McpToolbox {
        executed: Rc<RefCell<Vec<String>>>,
    }

    impl Toolbox for McpToolbox {
        fn definitions(&self) -> Value {
            json!([])
        }

        fn execute(
            &self,
            name: &str,
            _args: &Value,
            _access: CommandAccess,
            _stop: &AtomicBool,
            _on_output: &mut dyn FnMut(OutputStream, &str),
        ) -> ToolResult {
            self.executed.borrow_mut().push(name.to_string());
            ToolResult::success("mcp ran".into(), false)
        }

        fn mcp_tool_names(&self) -> Vec<String> {
            vec!["fs_echo".into()]
        }
    }

    #[test]
    fn an_mcp_tool_call_is_allowed_even_if_not_in_the_agents_list() {
        let executed = Rc::new(RefCell::new(Vec::new()));
        let tools = McpToolbox {
            executed: executed.clone(),
        };
        let log = MemoryLog::default();
        let model = Scripted(RefCell::new(vec![]));
        let stop = AtomicBool::new(false);
        let agents = builder();
        let workdir = PathBuf::from("/tmp");
        let mut on_event = |_: &Event| {};
        let mut turn = super::Turn {
            model: &model,
            log: &log,
            tools: &tools,
            session_id: "s",
            workdir: &workdir,
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            autonomous: false,
            stop: &stop,
            on_event: &mut on_event,
        };
        let agent = &agents[0];
        // The builder does not list `fs_echo`; the MCP set is what allows it.
        assert!(!agent.allows("fs_echo"));
        let mcp_call = ToolCall {
            id: "c1".into(),
            name: "fs_echo".into(),
            arguments: json!({}),
            parse_error: None,
            raw_arguments: None,
        };
        execute_tools(&mut turn, agent, "m1", &[mcp_call]).unwrap();
        assert_eq!(*executed.borrow(), vec!["fs_echo".to_string()]);

        // A non-MCP tool not in the list is still refused and never runs.
        let refused = ToolCall {
            id: "c2".into(),
            name: "web_search".into(),
            arguments: json!({}),
            parse_error: None,
            raw_arguments: None,
        };
        execute_tools(&mut turn, agent, "m2", &[refused]).unwrap();
        assert_eq!(*executed.borrow(), vec!["fs_echo".to_string()]);
        assert!(
            log.0
                .borrow()
                .iter()
                .any(|e| matches!(e, Event::ToolResult { result, .. }
                if !result.success
                    && result.error.as_deref().is_some_and(|m| m.contains("not available"))))
        );
    }
}
