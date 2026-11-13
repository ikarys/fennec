//! The session's frozen prompt: the system prompt and the tool definitions,
//! snapshotted at the first turn so the prompt only grows.

use super::ask_user;
use super::delegate;
use super::prompt::system_prompt;
use super::propose_criteria;
use super::propose_plan;
use super::propose_task;
use super::record_decision;
use super::request_waiver;
use super::submit_report;
use super::suggest_agent;
use super::turn::Turn;
use crate::domain::conversation::frozen_prompt;
use crate::domain::event::Event;
use crate::domain::mcp::META_TOOLS;
use serde_json::Value;

/// The system prompt, tools included: frozen at the session's first turn
/// and read back, so a new day or a new build (a new tool, a reworded
/// description) does not change the prompt's first bytes and void the whole
/// session's prompt cache.
pub(super) fn round_prompt_parts(turn: &mut Turn) -> Result<(String, Value), String> {
    let events = turn.log.events_of(turn.session_id, &["turn.snapshot"])?;
    let (prompt, tools) = frozen_prompt(&events);
    if let (Some(prompt), Some(tools)) = (prompt, tools) {
        // MCP was configured or removed since the last freeze, or the session
        // still has the per-tool MCP definitions of older builds: re-freeze
        // the tools, keeping the prompt byte-identical. One deliberate cache
        // miss; turning a server on or off is told by the catalog notice.
        if declares_mcp(tools) != turn.tools.offers_mcp() || froze_legacy_mcp(&events) {
            let new_tools = tool_definitions(turn);
            turn.append(Event::TurnSnapshot {
                cached_system_prompt: prompt.to_string(),
                cached_tools: Some(new_tools.clone()),
                mcp_tool_names: Vec::new(),
            })?;
            return Ok((prompt.to_string(), new_tools));
        }
        return Ok((prompt.to_string(), tools.clone()));
    }

    // A session from before the tools were frozen keeps its prompt and
    // freezes the tools now: one cache miss, then none.
    let prompt = prompt.map(str::to_string).unwrap_or_else(|| {
        system_prompt(
            turn.workdir,
            turn.model.name(),
            turn.instructions,
            turn.skills,
            turn.always_on,
            turn.style,
        )
    });
    let tools = tools.cloned().unwrap_or_else(|| tool_definitions(turn));
    turn.append(Event::TurnSnapshot {
        cached_system_prompt: prompt.clone(),
        cached_tools: Some(tools.clone()),
        mcp_tool_names: Vec::new(),
    })?;
    Ok((prompt, tools))
}

/// Whether the frozen tools declare the lazy MCP tools.
fn declares_mcp(tools: &Value) -> bool {
    tools.as_array().is_some_and(|tools| {
        tools
            .iter()
            .any(|tool| tool["function"]["name"] == META_TOOLS[1])
    })
}

/// Whether the last freeze holds the per-tool MCP definitions of older builds.
fn froze_legacy_mcp(events: &[Event]) -> bool {
    events.iter().rev().find_map(|event| match event {
        Event::TurnSnapshot { mcp_tool_names, .. } => Some(!mcp_tool_names.is_empty()),
        _ => None,
    }) == Some(true)
}

/// The loop owns `ask_user`, `request_waiver`, `propose_criteria`,
/// `record_decision`, `propose_task`, `propose_plan`, `submit_report` and
/// `suggest_agent` and `delegate`;
/// they are not host tools, so their definitions are added to the toolbox's.
/// `suggest_agent` and `delegate` list the session's agents as of the first turn;
/// definitions are frozen then.
fn tool_definitions(turn: &Turn) -> Value {
    let mut tool_defs = turn.tools.definitions();
    if let Some(arr) = tool_defs.as_array_mut() {
        arr.push(ask_user::definition());
        arr.push(request_waiver::definition());
        arr.push(propose_criteria::definition());
        arr.push(record_decision::definition());
        arr.push(propose_task::definition());
        arr.push(propose_plan::definition());
        arr.push(submit_report::definition());
        arr.push(suggest_agent::definition(turn.agents));
        arr.push(delegate::definition(turn.agents));
    }
    tool_defs
}
