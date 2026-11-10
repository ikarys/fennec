//! The multi-turn loop. Each round streams the model (`round`), then runs
//! its tool calls, until it answers without one.
//!
//! Agents/modes: the turn resolves one top-level agent from the log's last
//! `mode.changed`, injects its reminder when the current window lacks it,
//! and enforces its tool allow-list at execution.
//!
//! `ask_user`: the turn ends with `waiting_for_user` and the answer arrives
//! as the next turn's input, so a pending question survives a restart.
//!
//! Compaction: at a threshold the loop asks the model for a summary and
//! opens a new window from it (`compaction`), and `compact_session` runs it
//! on demand.
//!
//! Tool calls run one at a time; sub-agents are not a tool yet.

use super::compaction::{self, Compacted};
use super::ports::{ChildSessions, EventLog, Model, Toolbox};
use super::preflight;
use super::round::{self, CONTINUE_AFTER_STREAM_ERROR_PROMPT, Round, RoundPrompt};
use super::snapshot::round_prompt_parts;
use super::tool_calls::{Flow, INTERRUPTED_ERROR, execute_tools};
use crate::domain::agent::{AgentDefinition, DEFAULT_COLOR, last_reminder, resolve};
use crate::domain::attachment::Attachment;
use crate::domain::compaction::{context_tokens, should_compact};
use crate::domain::criteria::{accepted_proposal, parse_done_when};
use crate::domain::event::{
    DoneReason, Event, MessageMetadata, MetadataSource, ReminderKind, Role, ToolResult,
};
use crate::domain::id::new_id;
use crate::domain::open_calls;
use crate::domain::preferences::ReasoningEffort;
use crate::domain::replay::replayable;
use crate::domain::skill::Skill;
use crate::domain::workdir_notice;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const CONTINUE_AFTER_TRUNCATION_PROMPT: &str =
    "Continue your previous response exactly where you left off.";
const MAX_TRUNCATION_RETRIES: u32 = 3;
/// The loop runs until aborted; a cap also keeps a confused model from
/// running forever when nobody watches.
const MAX_ROUNDS: u32 = 200;

/// One session's turn and everything it runs against.
pub struct Turn<'a> {
    pub model: &'a dyn Model,
    pub log: &'a dyn EventLog,
    pub tools: &'a dyn Toolbox,
    pub session_id: &'a str,
    pub workdir: &'a Path,
    /// The custom instructions, assembled by the adapter. Only a
    /// session's first turn uses them: the system prompt is frozen then.
    pub instructions: &'a str,
    /// `skills`: listed in the system prompt, frozen with it.
    pub skills: &'a [Skill],
    /// Skill ids whose bodies the system prompt carries; frozen with it.
    pub always_on: &'a [String],
    /// The agents the session can run as; the log's last `mode.changed`
    /// picks one.
    pub agents: &'a [AgentDefinition],
    pub max_tokens: u32,
    /// The model's context window, reported in `context.state`.
    pub context_window: u64,
    /// The compaction threshold as a fraction of the window; 0 disables it.
    pub compact_at: f64,
    /// `reasoningEffort`; `None` sends no `reasoning_effort` field.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// The effort the human chose for the session; wins over the agent's.
    pub session_effort: Option<ReasoningEffort>,
    /// The human is away (the ideas queue's night mode): `ask_user` and
    /// `propose_criteria` answer themselves instead of pausing the turn.
    pub autonomous: bool,
    /// The language the human reads fennec in: the questions the loop builds follow it.
    pub lang: crate::domain::lang::Lang,
    /// The plan's preflight feedback was already sent back in this turn: it
    /// is sent once, shared by the turn-end text path and the `propose_plan`
    /// tool.
    pub preflight_sent_back: bool,
    /// Set by the human to stop the turn: checked between rounds, while
    /// streaming and while a command runs.
    pub stop: &'a AtomicBool,
    /// Called for every event right after it is stored (the CLI prints them).
    pub on_event: &'a mut dyn FnMut(&Event),
    /// Where `delegate` runs its agents; None in the CLI and inside a
    /// delegated agent's own session, which cannot delegate further.
    pub children: Option<&'a dyn ChildSessions>,
}

impl Turn<'_> {
    pub(super) fn append(&mut self, event: Event) -> Result<(), String> {
        self.log.append(self.session_id, &event)?;
        (self.on_event)(&event);
        Ok(())
    }

    /// A message fennec writes in the human's place (a notice, a feedback, a continuation).
    pub(super) fn append_system(&mut self, content: &str) -> Result<(), String> {
        self.append_user(content, true, None, Vec::new())
    }

    pub(super) fn append_user(
        &mut self,
        content: &str,
        system_generated: bool,
        metadata: Option<MessageMetadata>,
        attachments: Vec<Attachment>,
    ) -> Result<(), String> {
        let message_id = new_id();
        self.append(Event::MessageStart {
            message_id: message_id.clone(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: system_generated,
            metadata,
            attachments,
        })?;
        self.append(Event::MessageDone {
            message_id,
            stats: None,
            partial: false,
        })
    }
}

/// Appends the user's prompt and runs the loop to completion. Returns the
/// `chat.done` reason.
pub fn run_turn(
    turn: &mut Turn,
    prompt: &str,
    attachments: &[Attachment],
) -> Result<DoneReason, String> {
    with_running(turn, |turn| answer_or_prompt(turn, prompt, attachments))
}

/// The human's replay: cut the model's view back to before `message_id`
/// and send that message again, with its images.
pub fn replay_turn(turn: &mut Turn, message_id: &str) -> Result<DoneReason, String> {
    with_running(turn, |turn| {
        let events = turn.log.events(turn.session_id)?;
        let (content, attachments) = replayable(&events, message_id).ok_or_else(|| {
            "only a message you typed in the current window can be replayed".to_string()
        })?;
        turn.append(Event::SessionRewound {
            message_id: message_id.to_string(),
        })?;
        answer_or_prompt(turn, &content, &attachments)
    })
}

/// The human's retry of a turn that ended in an error: a system-generated
/// continuation prompt re-runs the loop from where it stopped.
pub fn retry_turn(turn: &mut Turn) -> Result<DoneReason, String> {
    with_running(turn, |turn| {
        let events = turn.log.events(turn.session_id)?;
        if !open_calls::open_calls(&events).is_empty() {
            return Err("answer the pending question or send a message first".into());
        }
        let prompt = retry_prompt(&events)?;
        let agent = resolve(turn.agents, &events).ok_or("no top-level agent")?;
        ensure_agent_reminder(turn, agent)?;
        turn.append_system(&prompt)?;
        run_top_level_agent_loop(turn, agent)
    })
}

/// The continuation prompt of a retry, with the last error when there is one.
fn retry_prompt(events: &[Event]) -> Result<String, String> {
    let last_done = events
        .iter()
        .rfind(|e| matches!(e, Event::ChatDone { .. }))
        .and_then(|e| match e {
            Event::ChatDone { reason, .. } => Some(*reason),
            _ => None,
        })
        .ok_or("no turn to retry")?;
    if last_done != DoneReason::Error {
        return Err("the last turn did not end in an error".into());
    }
    let error = events.iter().rev().find_map(|e| match e {
        Event::ChatError { error, .. } => Some(error.clone()),
        _ => None,
    });
    Ok(match error {
        Some(error) => format!(
            "The last turn ended in an error: {error}\n{CONTINUE_AFTER_STREAM_ERROR_PROMPT}"
        ),
        None => CONTINUE_AFTER_STREAM_ERROR_PROMPT.to_string(),
    })
}

pub(super) fn answer_or_prompt(
    turn: &mut Turn,
    prompt: &str,
    attachments: &[Attachment],
) -> Result<DoneReason, String> {
    let events = turn.log.events(turn.session_id)?;
    let agents = turn.agents;
    let agent = resolve(agents, &events).ok_or_else(|| "no top-level agent".to_string())?;
    // Close tool calls left open by a previous turn before writing the
    // prompt: the model API rejects an assistant tool call without its
    // tool message. When the open call awaits an answer, the human's
    // input *is* that answer, so no user message is appended on top of it.
    let answered = close_open_calls(turn, prompt)?;
    ensure_workdir_notice(turn)?;
    ensure_agent_reminder(turn, agent)?;
    if !answered {
        turn.append_user(prompt, false, None, attachments.to_vec())?;
    } else if !attachments.is_empty() {
        // A tool result only carries text from the human, so the images
        // ride a user message of their own.
        turn.append_user("", false, None, attachments.to_vec())?;
    }
    run_top_level_agent_loop(turn, agent)
}

/// The human's manual compaction: summarise the history and open a new
/// window, then end the turn. Refuses while a question is pending.
pub fn compact_session(turn: &mut Turn) -> Result<DoneReason, String> {
    with_running(turn, |turn| {
        let events = turn.log.events(turn.session_id)?;
        if !open_calls::open_calls(&events).is_empty() {
            return Err("answer the pending question or send a message first".into());
        }
        // The summary round runs as the session's agent; the session's effort
        // wins over the agent's.
        let agent_effort = resolve(turn.agents, &events).and_then(|agent| agent.reasoning_effort);
        let (system_prompt, tool_defs) = round_prompt_parts(turn)?;
        let prompt = RoundPrompt {
            system_prompt: &system_prompt,
            tools: &tool_defs,
            max_tokens: turn.max_tokens,
            reasoning_effort: turn
                .session_effort
                .or(agent_effort)
                .or(turn.reasoning_effort),
        };
        match compaction::compact(turn, &prompt)? {
            Compacted::Done | Compacted::Skipped => {
                finish(turn, new_id(), DoneReason::Complete, None)
            }
            Compacted::Ended(reason) => finish(turn, new_id(), reason, None),
        }
    })
}

/// Wraps a turn: mark it running, record a `chat.error` on failure, then
/// mark it not running. Shared by `run_turn` and `compact_session`.
pub(super) fn with_running(
    turn: &mut Turn,
    work: impl FnOnce(&mut Turn) -> Result<DoneReason, String>,
) -> Result<DoneReason, String> {
    turn.append(Event::RunningChanged { is_running: true })?;
    let result = work(turn);
    if let Err(e) = &result {
        // Best effort: the store itself may be what failed.
        let _ = turn.append(Event::ChatError {
            error: e.clone(),
            recoverable: false,
        });
    }
    turn.append(Event::RunningChanged { is_running: false })?;
    result
}

/// The system prompt and tool definitions a round is sent with, built once
/// so the loop and a manual compaction use an identical cached prefix.
/// Closes the open calls of a previous turn. The open call that awaits an
/// answer gets the human's input as its result; every other open call gets
/// the interrupted error. Returns true when an answer was given, so the
/// caller does not append a user message on top of it.
fn close_open_calls(turn: &mut Turn, prompt: &str) -> Result<bool, String> {
    let events = turn.log.events(turn.session_id)?;
    let open = open_calls::open_calls(&events);
    let mut answered = false;
    for call in open {
        let result = if call.awaits_answer && !answered {
            answered = true;
            // The human's accept is what applies a proposal, never the agent.
            if let Some(criteria) = accepted_proposal(&events, &call.call_id, prompt) {
                turn.append(Event::CriteriaSet { criteria })?;
                ToolResult::success("Accepted: the criteria are replaced.".into(), false)
            } else {
                ToolResult::success(prompt.to_string(), false)
            }
        } else {
            ToolResult::failure(INTERRUPTED_ERROR.into())
        };
        turn.append(Event::ToolResult {
            message_id: call.message_id,
            tool_call_id: call.call_id,
            result,
        })?;
    }
    Ok(answered)
}

/// Tells the model its working directory moved (a workflow opened the
/// session's worktree after the prompt was frozen): the prompt only grows.
fn ensure_workdir_notice(turn: &mut Turn) -> Result<(), String> {
    match workdir_notice::needed(&turn.log.events(turn.session_id)?, turn.workdir) {
        Some(notice) => turn.append_system(&notice),
        None => Ok(()),
    }
}

/// Injects the agent's definition when the last reminder in the model's
/// current window is another agent's, or there is none: at the first turn,
/// after a mode switch, after a compaction. Nothing is re-sent otherwise.
fn ensure_agent_reminder(turn: &mut Turn, agent: &AgentDefinition) -> Result<(), String> {
    if last_reminder(&turn.log.events(turn.session_id)?) == Some(agent.name.as_str()) {
        return Ok(());
    }

    let metadata = MessageMetadata {
        source: MetadataSource::Agent,
        name: agent.name.clone(),
        color: agent
            .color
            .clone()
            .unwrap_or_else(|| DEFAULT_COLOR.to_string()),
        kind: ReminderKind::Definition,
    };
    turn.append_user(&agent.reminder(), true, Some(metadata), Vec::new())
}

pub(super) fn run_top_level_agent_loop(
    turn: &mut Turn,
    agent: &AgentDefinition,
) -> Result<DoneReason, String> {
    let started = Instant::now();
    let (system_prompt, tool_defs) = round_prompt_parts(turn)?;
    let mut max_tokens = turn.max_tokens;
    let mut truncation_retries = 0;
    // Guards against compacting again before a real round has measured the
    // new window.
    let mut compacted_last_round = false;

    for _ in 0..MAX_ROUNDS {
        let message_id = new_id();
        if turn.stop.load(Ordering::Relaxed) {
            return finish(
                turn,
                message_id,
                DoneReason::Stopped,
                Some(started.elapsed()),
            );
        }
        let prompt = RoundPrompt {
            system_prompt: &system_prompt,
            tools: &tool_defs,
            max_tokens,
            reasoning_effort: turn
                .session_effort
                .or(agent.reasoning_effort)
                .or(turn.reasoning_effort),
        };
        if !compacted_last_round
            && should_compact(
                context_tokens(&turn.log.window(turn.session_id)?),
                turn.context_window,
                turn.compact_at,
            )
        {
            match compaction::compact(turn, &prompt)? {
                Compacted::Ended(reason) => {
                    return finish(turn, message_id, reason, Some(started.elapsed()));
                }
                Compacted::Done => ensure_agent_reminder(turn, agent)?,
                Compacted::Skipped => {}
            }
            compacted_last_round = true;
            continue;
        }
        compacted_last_round = false;
        let (answer, stats) = match round::stream(turn, &message_id, &prompt)? {
            Round::Answered(answer, stats) => (answer, stats),
            Round::Interrupted => continue,
            Round::Ended(reason) => {
                return finish(turn, message_id, reason, Some(started.elapsed()));
            }
        };
        turn.append(Event::ContextState {
            current_tokens: answer.prompt_tokens + answer.completion_tokens,
            max_tokens: turn.context_window,
            compact_at: Some(turn.compact_at),
        })?;

        if answer.finish_reason == "length" && answer.tool_calls.is_empty() {
            let give_up = truncation_retries >= MAX_TRUNCATION_RETRIES;
            turn.append(Event::MessageDone {
                message_id: message_id.clone(),
                stats: Some(stats),
                partial: give_up,
            })?;
            if give_up {
                return finish(
                    turn,
                    message_id,
                    DoneReason::Truncated,
                    Some(started.elapsed()),
                );
            }
            truncation_retries += 1;
            max_tokens = max_tokens.saturating_mul(3) / 2;
            turn.append_system(CONTINUE_AFTER_TRUNCATION_PROMPT)?;
            continue;
        }

        for call in &answer.tool_calls {
            turn.append(Event::ToolCall {
                message_id: message_id.clone(),
                tool_call: call.clone(),
            })?;
        }
        turn.append(Event::MessageDone {
            message_id: message_id.clone(),
            stats: Some(stats),
            partial: false,
        })?;
        if answer.tool_calls.is_empty() {
            // The old free-text path: the plan is the message itself; the
            // `propose_plan` tool passes its criteria straight to the
            // preflight.
            let criteria = parse_done_when(&answer.content);
            if let Some(feedback) = preflight::run(turn, agent, &message_id, &criteria)?
                && !turn.preflight_sent_back
            {
                turn.preflight_sent_back = true;
                turn.append_system(&feedback)?;
                continue;
            }
            return finish(
                turn,
                message_id,
                DoneReason::Complete,
                Some(started.elapsed()),
            );
        }
        match execute_tools(turn, agent, &message_id, &answer.tool_calls)? {
            Flow::WaitingForUser => {
                return finish(
                    turn,
                    message_id,
                    DoneReason::WaitingForUser,
                    Some(started.elapsed()),
                );
            }
            // `propose_plan` recorded the plan, or `submit_report` the
            // report: the human reviews it.
            Flow::Done => {
                return finish(
                    turn,
                    message_id,
                    DoneReason::Complete,
                    Some(started.elapsed()),
                );
            }
            Flow::Continue => {
                // A message typed while the turn ran is dropped in here, after
                // the tools: append-only, so the prompt cache prefix is kept.
                super::queue::drain(turn)?;
            }
        }
    }

    turn.append(Event::ChatError {
        error: format!("stopped after {MAX_ROUNDS} rounds"),
        recoverable: false,
    })?;
    finish(turn, new_id(), DoneReason::Stopped, Some(started.elapsed()))
}

/// Records how the turn ended.
fn finish(
    turn: &mut Turn,
    message_id: String,
    reason: DoneReason,
    duration: Option<Duration>,
) -> Result<DoneReason, String> {
    turn.append(Event::ChatDone {
        message_id,
        reason,
        duration_ms: duration.map(|d| d.as_millis() as u64),
    })?;
    turn.model.release(turn.session_id);
    Ok(reason)
}

/// A fresh agent node in its child session, marked running like a human's
/// turn (the child's page shows "terminé" whenever the last `running.changed`
/// is false). Not for a session-context node: the walk already holds the
/// session's mark, and a nested one would clear it mid-run.
pub fn run_child_agent_node(
    turn: &mut Turn,
    agent_id: &str,
    prompt: &str,
) -> Result<DoneReason, String> {
    with_running(turn, |turn| run_agent_node(turn, agent_id, prompt))
}

/// Runs `agent` on `prompt` in this session, as a workflow node: switches
/// the mode when needed (as the human would), injects the agent's reminder,
/// appends the prompt as a system-generated user message, runs the loop.
pub fn run_agent_node(turn: &mut Turn, agent_id: &str, prompt: &str) -> Result<DoneReason, String> {
    let events = turn.log.events(turn.session_id)?;
    let agent = turn
        .agents
        .iter()
        .find(|a| a.id == agent_id)
        .ok_or_else(|| format!("unknown agent: {agent_id}"))?;

    if resolve(turn.agents, &events).is_none_or(|a| a.id != agent_id) {
        turn.append(Event::ModeChanged {
            mode: agent_id.to_string(),
            auto: true,
            reason: Some("workflow".into()),
        })?;
    }

    ensure_workdir_notice(turn)?;
    ensure_agent_reminder(turn, agent)?;
    turn.append_system(prompt)?;
    run_top_level_agent_loop(turn, agent)
}

#[cfg(test)]
mod tests;
