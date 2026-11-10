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
mod tests {
    use super::*;
    use crate::agent::ports::{Completion, Delta, Request};
    use crate::agent::round::CONTINUE_AFTER_STREAM_ERROR_PROMPT;
    use crate::agent::test_support::{FakeToolbox, MemoryLog, Scripted, builder, plan, reply};
    use crate::domain::conversation::fold_messages;
    use crate::domain::event::ToolCall;
    use crate::tools::LocalTools;
    use serde_json::{Value, json};
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// Owns a turn's dependencies so a test can run several turns on the
    /// same session and inspect the log between them.
    struct Harness<'a> {
        log: MemoryLog,
        model: Box<dyn Model>,
        tools: LocalTools,
        stop: &'a AtomicBool,
        workdir: PathBuf,
        on_event: Box<dyn FnMut(&Event) + 'a>,
        agents: Vec<AgentDefinition>,
    }

    impl<'a> Harness<'a> {
        fn new(model: Box<dyn Model>, workdir: PathBuf, stop: &'a AtomicBool) -> Self {
            Harness {
                log: MemoryLog::default(),
                model,
                tools: LocalTools::new(workdir.clone()),
                stop,
                workdir,
                on_event: Box::new(|_: &Event| {}),
                agents: builder(),
            }
        }

        fn turn(&mut self) -> Turn<'_> {
            Turn {
                model: &*self.model,
                log: &self.log,
                tools: &self.tools,
                session_id: "s",
                workdir: &self.workdir,
                instructions: "",
                skills: &[],
                always_on: &[],
                agents: &self.agents,
                max_tokens: 100,
                context_window: 1000,
                compact_at: 0.0,
                reasoning_effort: None,
                session_effort: None,
                autonomous: false,
                lang: crate::domain::lang::Lang::Fr,
                preflight_sent_back: false,
                stop: self.stop,
                on_event: &mut self.on_event,
                children: None,
            }
        }
    }

    fn run(model: &dyn Model, workdir: &Path) -> (DoneReason, Vec<Event>) {
        run_with_rules(model, workdir, rules::Rules::default())
    }

    fn run_with_rules(
        model: &dyn Model,
        workdir: &Path,
        rules: rules::Rules,
    ) -> (DoneReason, Vec<Event>) {
        let log = MemoryLog::default();
        let tools = LocalTools::new(workdir.to_path_buf()).with_rules(rules);
        let stop = AtomicBool::new(false);
        let mut on_event = |_: &Event| {};
        let agents = builder();
        let mut turn = Turn {
            model,
            log: &log,
            tools: &tools,
            session_id: "s",
            workdir,
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            session_effort: None,
            autonomous: false,
            lang: crate::domain::lang::Lang::Fr,
            preflight_sent_back: false,
            stop: &stop,
            on_event: &mut on_event,
            children: None,
        };
        let reason = run_turn(&mut turn, "go", &[]).unwrap();
        (reason, log.0.into_inner())
    }

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fennec-turn-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_command_matching_a_rule_is_refused_and_logged() {
        let dir = scratch_dir();
        std::fs::create_dir_all(dir.join("target")).unwrap();
        let call = ToolCall {
            id: "c1".into(),
            name: "run_command".into(),
            arguments: json!({"command": "rm -fr target"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let rules = rules::Rules::new(vec![rules::Rule {
            id: 1,
            scope: rules::Scope::Global,
            target: rules::Target::parse(rules::TargetKind::Command, "rm -rf", None).unwrap(),
            action: rules::Action::Block,
            note: None,
            origin: rules::Origin::Human,
        }]);
        let (_, events) = run_with_rules(&model, &dir, rules);
        assert!(events.iter().any(
            |e| matches!(e, Event::RuleBlocked { subject, .. } if subject == "rm -fr target")
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            Event::ToolResult { result, .. } if !result.success && result.error.as_deref().is_some_and(|m| m.contains("rm -rf"))
        )));
        assert!(dir.join("target").exists(), "the command never ran");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_final_chat_done_carries_the_loop_duration() {
        let dir = scratch_dir();
        let model = Scripted(RefCell::new(vec![reply("hi", vec![])]));
        let (_, events) = run(&model, &dir);
        assert!(matches!(
            events.iter().rfind(|e| matches!(e, Event::ChatDone { .. })),
            Some(Event::ChatDone {
                duration_ms: Some(_),
                ..
            })
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn tool_call_round_then_final_answer() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: json!({"path": "hello.txt", "content": "hi"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let (reason, events) = run(&model, &dir);
        assert_eq!(reason, DoneReason::Complete);
        assert_eq!(
            std::fs::read_to_string(dir.join("hello.txt")).unwrap(),
            "hi"
        );
        let results: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::ToolResult { result, .. } => Some(result.success),
                _ => None,
            })
            .collect();
        assert_eq!(results, [true]);
        let msgs = fold_messages(&events);
        assert_eq!(msgs.last().unwrap()["content"], "done");
        assert_eq!(msgs[3]["role"], "tool");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn queued_message_is_drained_between_tool_rounds() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: json!({"path": "hello.txt", "content": "hi"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut harness = Harness::new(Box::new(model), dir.clone(), &stop);
        let queue_id = new_id();
        harness
            .log
            .append(
                "s",
                &Event::QueueAdded {
                    queue_id: queue_id.clone(),
                    mode: crate::domain::event::QueueMode::Asap,
                    content: "queued".into(),
                    queued_at: 1,
                },
            )
            .unwrap();

        let reason = run_turn(&mut harness.turn(), "prompt", &[]).unwrap();
        let events = harness.log.events("s").unwrap();
        assert_eq!(reason, DoneReason::Complete);

        let tool_result = events
            .iter()
            .position(|e| matches!(e, Event::ToolResult { .. }))
            .unwrap();
        let queued_start = events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    Event::MessageStart {
                        role: Role::User,
                        content: Some(content),
                        ..
                    } if content == "queued"
                )
            })
            .unwrap();
        let drained = events
            .iter()
            .position(|e| matches!(e, Event::QueueDrained { queue_id: id } if id == &queue_id))
            .unwrap();
        // The next round's answer comes after the queued message.
        assert!(events[drained..].iter().any(|e| matches!(
            e,
            Event::MessageStart {
                role: Role::Assistant,
                ..
            }
        )));

        assert!(tool_result < queued_start);
        assert!(queued_start < drained);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_loop_compacts_past_the_threshold_then_goes_on() {
        let call = ToolCall {
            id: "c1".into(),
            name: "run_command".into(),
            arguments: json!({}),
            parse_error: Some("bad".into()),
            raw_arguments: None,
        };
        // A 1000-token window leaves no headroom: any measured context fills it.
        let full = Completion {
            prompt_tokens: 500,
            ..reply("", vec![call]).unwrap()
        };
        let model = Scripted(RefCell::new(vec![
            Ok(full),
            reply("the summary", vec![]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        let mut turn = h.turn();
        turn.compact_at = 0.85;
        assert_eq!(
            run_turn(&mut turn, "go", &[]).unwrap(),
            DoneReason::Complete
        );
        let events = h.log.0.into_inner();
        let compacted = events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    Event::ContextCompacted {
                        before_tokens: 500,
                        ..
                    }
                )
            })
            .expect("context.compacted");
        // The round after the cut sees only the resume message.
        let messages = fold_messages(&events[..compacted + 2]);
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0]["content"]
                .as_str()
                .unwrap()
                .contains("the summary")
        );
    }

    #[test]
    fn unparsable_arguments_are_reported_to_the_model_not_run() {
        let call = ToolCall {
            id: "c1".into(),
            name: "run_command".into(),
            arguments: json!({}),
            parse_error: Some("bad".into()),
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("ok", vec![]),
        ]));
        let (_, events) = run(&model, Path::new("/tmp"));
        assert!(events.iter().any(|e| matches!(e,
            Event::ToolResult { result, .. } if result.error.as_deref().unwrap().starts_with("Failed to parse"))));
    }

    #[test]
    fn mid_stream_failure_keeps_partial_and_asks_to_continue() {
        struct FailsMidStream(RefCell<u32>);
        impl Model for FailsMidStream {
            fn name(&self) -> &str {
                "fails-mid-stream"
            }

            fn stream(&self, _: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
                *self.0.borrow_mut() += 1;
                on(Delta::Text("part".into()));
                if *self.0.borrow() == 1 {
                    return Err("reset".into());
                }
                reply("part", vec![])
            }
        }
        let (reason, events) = run(&FailsMidStream(RefCell::new(0)), Path::new("/tmp"));
        assert_eq!(reason, DoneReason::Complete);
        let agent = builder().into_iter().next().unwrap();
        let msgs = fold_messages(&events);
        let contents: Vec<_> = msgs
            .iter()
            .map(|m| m["content"].as_str().unwrap())
            .collect();
        assert_eq!(
            contents,
            [
                agent.reminder().as_str(),
                "go",
                "part",
                CONTINUE_AFTER_STREAM_ERROR_PROMPT,
                "part"
            ]
        );
    }

    #[test]
    fn a_failed_request_is_reported_before_it_is_retried() {
        let model = Scripted(RefCell::new(vec![
            Err("connection refused".into()),
            reply("done", vec![]),
        ]));
        let (reason, events) = run(&model, Path::new("/tmp"));
        assert_eq!(reason, DoneReason::Complete);
        assert!(events.iter().any(|e| matches!(e,
            Event::ChatError { error, recoverable: true }
                if error == "connection refused (retry 1/3 in 1s)")));
    }

    /// Answers at once and keeps the system prompt of each call.
    #[derive(Default)]
    struct Recording(RefCell<Vec<String>>);

    impl Model for Recording {
        fn name(&self) -> &str {
            "recording"
        }

        fn stream(&self, req: &Request, _: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
            self.0.borrow_mut().push(req.system_prompt.to_string());
            reply("done", vec![])
        }
    }

    #[test]
    fn the_system_prompt_is_frozen_for_the_session() {
        let log = MemoryLog::default();
        log.append(
            "s",
            &Event::TurnSnapshot {
                cached_system_prompt: "frozen".into(),
                cached_tools: Some(json!([])),
                mcp_tool_names: Vec::new(),
            },
        )
        .unwrap();
        let model = Recording::default();
        let workdir = std::env::temp_dir();
        let tools = LocalTools::new(workdir.clone());
        let stop = AtomicBool::new(false);
        let mut on_event = |_: &Event| {};
        let agents = builder();
        let mut turn = Turn {
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
            session_effort: None,
            autonomous: false,
            lang: crate::domain::lang::Lang::Fr,
            preflight_sent_back: false,
            stop: &stop,
            on_event: &mut on_event,
            children: None,
        };
        run_turn(&mut turn, "one", &[]).unwrap();
        run_turn(&mut turn, "two", &[]).unwrap();
        assert_eq!(*model.0.borrow(), ["frozen", "frozen"]);
        let snapshots = log
            .0
            .borrow()
            .iter()
            .filter(|e| matches!(e, Event::TurnSnapshot { .. }))
            .count();
        assert_eq!(snapshots, 1);
        let stats = log.0.borrow().iter().find_map(|e| match e {
            Event::MessageDone { stats, .. } => stats.clone(),
            _ => None,
        });
        assert_eq!(stats.map(|s| s.model), Some("recording".into()));
    }

    #[test]
    fn ask_user_ends_the_turn_and_the_answer_is_the_next_turns_input() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "ask_user".into(),
            arguments: json!({"question": "Which color?"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);
        let events = h.log.0.borrow().clone();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::ChatAskUser { call_id, .. } if call_id == "c1"))
        );
        assert!(
            !events.iter().any(
                |e| matches!(e, Event::ToolResult { tool_call_id, .. } if tool_call_id == "c1")
            )
        );
        let reason2 = run_turn(&mut h.turn(), "blue", &[]).unwrap();
        assert_eq!(reason2, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        // The answer is a tool result, not a user message.
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.output.as_deref() == Some("blue"))));
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::MessageStart { role: Role::User, content: Some(c), .. } if c == "blue")));
        let msgs = fold_messages(&events);
        let tool_idx = msgs
            .iter()
            .position(|m| m["role"] == "tool" && m["tool_call_id"] == "c1")
            .unwrap();
        assert_eq!(msgs[tool_idx]["content"], "blue");
        assert_eq!(msgs[tool_idx - 1]["role"], "assistant");
        assert_eq!(msgs[tool_idx - 1]["tool_calls"][0]["id"], "c1");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn while_the_human_is_away_ask_user_and_propose_criteria_answer_themselves() {
        let dir = scratch_dir();
        let ask = ToolCall {
            id: "c1".into(),
            name: "ask_user".into(),
            arguments: json!({"question": "Which color?"}),
            parse_error: None,
            raw_arguments: None,
        };
        let propose = ToolCall {
            id: "c2".into(),
            name: "propose_criteria".into(),
            arguments: json!({"criteria":[{"description":"builds","check":"true"}],"reason":"r"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![ask, propose]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let mut turn = h.turn();
        turn.autonomous = true;
        assert_eq!(
            run_turn(&mut turn, "go", &[]).unwrap(),
            DoneReason::Complete
        );
        let events = h.log.0.borrow().clone();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::ChatAskUser { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::CriteriaProposed { .. }))
        );
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.success)));
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c2" && !result.success)));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn propose_criteria_accepts_the_proposed_list() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "propose_criteria".into(),
            arguments: json!({"criteria":[{"description":"builds","check":"true"}],"reason":"the old check cannot run"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);
        let events = h.log.0.borrow().clone();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::CriteriaProposed { call_id, .. } if call_id == "c1"))
        );
        assert!(events.iter().any(|e| matches!(e, Event::ChatAskUser { call_id, question, .. } if call_id == "c1" && question.contains("builds"))));

        let reason2 = run_turn(&mut h.turn(), "Accept", &[]).unwrap();
        assert_eq!(reason2, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        let set_idx = events
            .iter()
            .position(|e| matches!(e, Event::CriteriaSet { .. }))
            .unwrap();
        let result_idx = events
            .iter()
            .position(
                |e| matches!(e, Event::ToolResult { tool_call_id, .. } if tool_call_id == "c1"),
            )
            .unwrap();
        assert!(set_idx < result_idx);
        assert!(events.iter().any(|e| matches!(e, Event::CriteriaSet { criteria } if criteria.len() == 1 && criteria[0].id == "C1" && criteria[0].description == "builds" && criteria[0].check.as_deref() == Some("true"))));
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.output.as_deref() == Some("Accepted: the criteria are replaced."))));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn propose_criteria_refusal_keeps_the_criteria() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "propose_criteria".into(),
            arguments: json!({"criteria":[{"description":"builds","check":"true"}],"reason":"the old check cannot run"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);

        let reason2 = run_turn(&mut h.turn(), "Refuse", &[]).unwrap();
        assert_eq!(reason2, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::CriteriaSet { .. }))
        );
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.output.as_deref() == Some("Refuse"))));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn propose_criteria_noop_does_not_ask() {
        use crate::domain::criteria::Criterion;
        use crate::domain::workflow::definition::{
            Edge, EndOutcome, Node, NodeKind, Position, Workflow,
        };

        let dir = scratch_dir();
        let workflow = Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![
                Node {
                    id: "start".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::Start,
                },
                Node {
                    id: "end".into(),
                    name: None,
                    position: Position::default(),
                    kind: NodeKind::End {
                        outcome: EndOutcome::Done,
                    },
                },
            ],
            edges: vec![Edge {
                from: "start".into(),
                port: "next".into(),
                to: "end".into(),
                max_passes: None,
                pause: false,
            }],
        };
        let call = ToolCall {
            id: "c1".into(),
            name: "propose_criteria".into(),
            arguments: json!({"criteria":[{"description":"builds","check":"true"}],"reason":"unchanged"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        h.log.0.borrow_mut().extend([
            Event::WorkflowStarted {
                run_id: "r".into(),
                workflow,
                task: "t".into(),
                step_by_step: false,
                started_at: None,
            },
            Event::CriteriaSet {
                criteria: vec![Criterion {
                    id: "C1".into(),
                    description: "builds".into(),
                    check: Some("true".into()),
                }],
            },
        ]);

        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::ChatAskUser { call_id, .. } if call_id == "c1"))
        );
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.output.as_deref() == Some("The criteria are unchanged; nothing to accept."))));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn record_decision_is_logged_and_the_turn_goes_on() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "d1".into(),
            name: "record_decision".into(),
            arguments: json!({"title":"Storage","choice":"SQLite","reason":"one file","alternatives":["Postgres"]}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(events.iter().any(|e| matches!(e, Event::DecisionRecorded { call_id, title, .. } if call_id == "d1" && title == "Storage")));
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "d1" && result.output.as_deref() == Some("Decision recorded."))));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_tool_after_ask_user_in_the_same_batch_is_not_run() {
        let dir = scratch_dir();
        let ask = ToolCall {
            id: "c1".into(),
            name: "ask_user".into(),
            arguments: json!({"question": "Proceed?"}),
            parse_error: None,
            raw_arguments: None,
        };
        let read = ToolCall {
            id: "c2".into(),
            name: "read_file".into(),
            arguments: json!({"path": "x.txt"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![reply("", vec![ask, read])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);
        let events = h.log.0.borrow().clone();
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c2" && result.error.as_deref().unwrap().starts_with("Not run"))));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_stopped_batch_records_interrupted_results_without_running() {
        let dir = scratch_dir();
        let write = ToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: json!({"path": "hello.txt", "content": "hi"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![write]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        // Stop the turn as soon as the model's tool call is recorded, so the
        // batch sees it before running.
        h.on_event = Box::new(|e: &Event| {
            if matches!(e, Event::ToolCall { .. }) {
                stop.store(true, Ordering::Relaxed);
            }
        });
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::Stopped);
        let events = h.log.0.borrow().clone();
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.error.as_deref().unwrap() == INTERRUPTED_ERROR)));
        assert!(!dir.join("hello.txt").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_crashed_turns_open_call_is_closed_before_the_next_prompt() {
        let dir = scratch_dir();
        let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        // A previous turn recorded a tool call and died before its result.
        h.log
            .append(
                "s",
                &Event::ToolCall {
                    message_id: "m".into(),
                    tool_call: ToolCall {
                        id: "c1".into(),
                        name: "write_file".into(),
                        arguments: json!({}),
                        parse_error: None,
                        raw_arguments: None,
                    },
                },
            )
            .unwrap();
        let reason = run_turn(&mut h.turn(), "hi", &[]).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        // The interrupted result is appended before the user message "hi".
        let result_idx = events
            .iter()
            .position(
                |e| matches!(e, Event::ToolResult { tool_call_id, .. } if tool_call_id == "c1"),
            )
            .unwrap();
        let user_idx = events
            .iter()
            .position(|e| matches!(e, Event::MessageStart { role: Role::User, content: Some(c), .. } if c == "hi"))
            .unwrap();
        assert!(result_idx < user_idx);
        assert!(matches!(
            &events[result_idx],
            Event::ToolResult { result, .. } if result.error.as_deref() == Some(INTERRUPTED_ERROR)
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn ask_user_without_a_question_fails_and_the_loop_continues() {
        let call = ToolCall {
            id: "c1".into(),
            name: "ask_user".into(),
            arguments: json!({}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::ChatAskUser { .. }))
        );
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.error.is_some())));
    }

    #[test]
    fn plan_mode_refuses_write_file() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: json!({"path": "hello.txt", "content": "hi"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        h.agents = vec![plan()];
        let reason = run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(events.iter().any(|e| matches!(e,
            Event::ToolResult { tool_call_id, result, .. }
                if tool_call_id == "c1"
                    && result.error.as_deref() == Some(
                        "Tool 'write_file' is not available in 'plan' mode. Available: ask_user, load_skill, read_file"
                    )
        )));
        assert!(!dir.join("hello.txt").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_first_turn_appends_the_agent_reminder_before_the_prompt() {
        let agent = builder().into_iter().next().unwrap();
        let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        run_turn(&mut h.turn(), "go", &[]).unwrap();
        let events = h.log.0.borrow().clone();
        let reminder_idx = events
            .iter()
            .position(|e| {
                matches!(e, Event::MessageStart {
                content: Some(c),
                is_system_generated: true,
                metadata: Some(MessageMetadata {
                    source: MetadataSource::Agent,
                    name,
                    kind: ReminderKind::Definition,
                    ..
                }),
                ..
            } if c == &agent.reminder() && name == &agent.name)
            })
            .unwrap();
        let prompt_idx = events
            .iter()
            .position(|e| {
                matches!(e, Event::MessageStart {
                role: Role::User,
                content: Some(c),
                is_system_generated: false,
                metadata: None,
                ..
            } if c == "go")
            })
            .unwrap();
        assert!(reminder_idx < prompt_idx);
        assert!(matches!(
            &events[reminder_idx + 1],
            Event::MessageDone { .. }
        ));
        assert!(matches!(
            &events[reminder_idx + 2],
            Event::MessageStart {
                role: Role::User,
                content: Some(c),
                ..
            } if c == "go"
        ));
    }

    #[test]
    fn a_second_turn_in_the_same_mode_appends_no_reminder() {
        let model = Scripted(RefCell::new(vec![
            reply("one", vec![]),
            reply("two", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        run_turn(&mut h.turn(), "one", &[]).unwrap();
        run_turn(&mut h.turn(), "two", &[]).unwrap();
        let events = h.log.0.borrow().clone();
        let reminders = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::MessageStart {
                        is_system_generated: true,
                        metadata: Some(MessageMetadata {
                            source: MetadataSource::Agent,
                            ..
                        }),
                        ..
                    }
                )
            })
            .count();
        assert_eq!(reminders, 1);
    }

    #[test]
    fn a_mode_change_appends_the_new_agents_reminder_before_the_prompt() {
        let plan_agent = plan();
        let mut agents = vec![plan_agent.clone()];
        agents.extend(builder());
        let model = Scripted(RefCell::new(vec![
            reply("one", vec![]),
            reply("two", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        h.agents = agents;
        h.log
            .append(
                "s",
                &Event::ModeChanged {
                    mode: "builder".into(),
                    auto: false,
                    reason: None,
                },
            )
            .unwrap();
        run_turn(&mut h.turn(), "one", &[]).unwrap();
        h.log
            .append(
                "s",
                &Event::ModeChanged {
                    mode: "plan".into(),
                    auto: false,
                    reason: None,
                },
            )
            .unwrap();
        run_turn(&mut h.turn(), "two", &[]).unwrap();
        let events = h.log.0.borrow().clone();
        let reminder_idx = events
            .iter()
            .position(|e| {
                matches!(e, Event::MessageStart {
                content: Some(c),
                is_system_generated: true,
                metadata: Some(MessageMetadata {
                    source: MetadataSource::Agent,
                    name,
                    kind: ReminderKind::Definition,
                    ..
                }),
                ..
            } if c == &plan_agent.reminder() && name == "Plan")
            })
            .unwrap();
        let prompt_idx = events
            .iter()
            .position(|e| {
                matches!(e, Event::MessageStart {
                role: Role::User,
                content: Some(c),
                is_system_generated: false,
                metadata: None,
                ..
            } if c == "two")
            })
            .unwrap();
        assert!(reminder_idx < prompt_idx);
    }

    /// Answers at once and keeps the tool definitions of each request.
    /// Shares what it saw with the test, which keeps a handle after the
    /// harness takes the model.
    #[derive(Default, Clone)]
    struct RecordingTools(std::rc::Rc<RefCell<Vec<Value>>>);

    impl Model for RecordingTools {
        fn name(&self) -> &str {
            "recording-tools"
        }

        fn stream(&self, req: &Request, _: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
            self.0.borrow_mut().push(req.tools.clone());
            reply("done", vec![])
        }
    }

    #[test]
    fn tool_definitions_are_frozen_with_the_prompt() {
        let stop = AtomicBool::new(false);
        let snapshot = |tools: Option<Value>| Event::TurnSnapshot {
            cached_system_prompt: "frozen".into(),
            cached_tools: tools,
            mcp_tool_names: Vec::new(),
        };
        let snapshots = |log: &MemoryLog| {
            log.0
                .borrow()
                .iter()
                .filter(|e| matches!(e, Event::TurnSnapshot { .. }))
                .count()
        };

        // A frozen list is sent as is, whatever the toolbox has now.
        let model = RecordingTools::default();
        let mut h = Harness::new(Box::new(model.clone()), PathBuf::from("/tmp"), &stop);
        h.log
            .append("s", &snapshot(Some(json!([{"name": "old"}]))))
            .unwrap();
        run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(*model.0.borrow(), [json!([{"name": "old"}])]);
        assert_eq!(snapshots(&h.log), 1);

        // A session from before tools were frozen freezes them once.
        let model = RecordingTools::default();
        let mut h = Harness::new(Box::new(model.clone()), PathBuf::from("/tmp"), &stop);
        h.log.append("s", &snapshot(None)).unwrap();
        run_turn(&mut h.turn(), "one", &[]).unwrap();
        run_turn(&mut h.turn(), "two", &[]).unwrap();
        let seen = model.0.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], seen[1]);
        assert_eq!(snapshots(&h.log), 2);
    }

    #[test]
    fn an_mcp_tool_set_change_refreezes_the_snapshot_once() {
        use crate::domain::mcp::{McpServer, McpTool, McpTransport};

        let stop = AtomicBool::new(false);
        let snapshots = |log: &MemoryLog| {
            log.0
                .borrow()
                .iter()
                .filter(|e| matches!(e, Event::TurnSnapshot { .. }))
                .count()
        };
        let prompts = |log: &MemoryLog| {
            log.0
                .borrow()
                .iter()
                .filter_map(|e| match e {
                    Event::TurnSnapshot {
                        cached_system_prompt,
                        ..
                    } => Some(cached_system_prompt.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let server = |tools: &[&str]| {
            let mut server = McpServer {
                name: "fs".into(),
                transport: McpTransport::Stdio,
                command: Some("x".into()),
                ..Default::default()
            };
            server.cached_tools = tools
                .iter()
                .map(|tool| McpTool {
                    annotations: None,
                    name: (*tool).into(),
                    description: None,
                    input_schema: json!({ "type": "object" }),
                })
                .collect();
            server
        };

        let model = RecordingTools::default();
        let mut h = Harness::new(Box::new(model.clone()), PathBuf::from("/tmp"), &stop);
        h.tools = LocalTools::new(PathBuf::from("/tmp")).with_mcp(vec![server(&["read", "write"])]);
        run_turn(&mut h.turn(), "one", &[]).unwrap();
        assert_eq!(snapshots(&h.log), 1);
        let first_tools = model.0.borrow()[0].clone();

        // The set is unchanged: no re-freeze, same tools sent.
        run_turn(&mut h.turn(), "two", &[]).unwrap();
        assert_eq!(snapshots(&h.log), 1);
        assert_eq!(model.0.borrow()[1], first_tools);

        // A tool is added: one re-freeze, and the prompt stays byte-identical.
        h.tools = LocalTools::new(PathBuf::from("/tmp"))
            .with_mcp(vec![server(&["read", "write", "delete"])]);
        run_turn(&mut h.turn(), "three", &[]).unwrap();
        assert_eq!(snapshots(&h.log), 2);
        assert_ne!(model.0.borrow()[2], first_tools);
        let prompts = prompts(&h.log);
        assert_eq!(prompts.len(), 2);
        assert_eq!(prompts[0], prompts[1]);

        // The set is stable again: no further re-freeze.
        run_turn(&mut h.turn(), "four", &[]).unwrap();
        assert_eq!(snapshots(&h.log), 2);
    }

    /// Records the chat messages of each request and answers from a scripted
    /// model.
    struct RecordingMessages {
        seen: std::rc::Rc<RefCell<Vec<Vec<Value>>>>,
        inner: Box<dyn Model>,
    }

    impl Model for RecordingMessages {
        fn name(&self) -> &str {
            self.inner.name()
        }

        fn stream(&self, req: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
            self.seen.borrow_mut().push(req.messages.to_vec());
            self.inner.stream(req, on)
        }
    }

    #[test]
    fn replaying_a_message_resends_it_without_the_dropped_answer() {
        let seen = std::rc::Rc::new(RefCell::new(Vec::new()));
        let model = RecordingMessages {
            seen: seen.clone(),
            inner: Box::new(Scripted(RefCell::new(vec![
                reply("first", vec![]),
                reply("again", vec![]),
            ]))),
        };
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        run_turn(&mut h.turn(), "go", &[]).unwrap();

        let events = h.log.0.borrow().clone();
        let prompt_id = events
            .iter()
            .find_map(|e| match e {
                Event::MessageStart {
                    message_id,
                    role: Role::User,
                    content: Some(c),
                    is_system_generated: false,
                    ..
                } if c == "go" => Some(message_id.clone()),
                _ => None,
            })
            .unwrap();
        replay_turn(&mut h.turn(), &prompt_id).unwrap();

        let requests = seen.borrow();
        assert_eq!(requests.len(), 2);
        let contents: Vec<String> = requests[1]
            .iter()
            .filter_map(|m| m["content"].as_str().map(str::to_string))
            .collect();
        assert_eq!(contents.iter().filter(|c| *c == "go").count(), 1);
        assert!(!contents.iter().any(|c| *c == "first"));

        let events = h.log.0.borrow().clone();
        // The dropped answer stays in the log.
        assert!(events.iter().any(|e| matches!(
            e,
            Event::MessageDelta { content, .. } if content == "first"
        )));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::SessionRewound { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn replaying_an_unknown_message_fails() {
        let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        let err = replay_turn(&mut h.turn(), "missing").unwrap_err();
        assert_eq!(
            err,
            "only a message you typed in the current window can be replayed"
        );
    }

    /// Fails its first four requests before any content, then answers.
    struct FailsThenAnswers(RefCell<u32>);

    impl Model for FailsThenAnswers {
        fn name(&self) -> &str {
            "fails-then-answers"
        }

        fn stream(&self, _: &Request, _: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
            if *self.0.borrow() < 4 {
                *self.0.borrow_mut() += 1;
                return Err("LLM HTTP 400".into());
            }
            reply("done", vec![])
        }
    }

    #[test]
    fn a_turn_ended_in_error_is_retried_with_a_continuation_prompt() {
        let model = FailsThenAnswers(RefCell::new(0));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        assert_eq!(
            run_turn(&mut h.turn(), "go", &[]).unwrap(),
            DoneReason::Error
        );
        assert_eq!(retry_turn(&mut h.turn()).unwrap(), DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        let prompts: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                Event::MessageStart {
                    role: Role::User,
                    content: Some(c),
                    is_system_generated: true,
                    ..
                } => Some(c.clone()),
                _ => None,
            })
            .collect();
        assert!(
            prompts
                .iter()
                .any(|p| p.contains("The last turn ended in an error: LLM HTTP 400"))
        );
    }

    #[test]
    fn retry_is_refused_when_the_last_turn_did_not_fail() {
        let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        run_turn(&mut h.turn(), "go", &[]).unwrap();
        assert_eq!(
            retry_turn(&mut h.turn()).unwrap_err(),
            "the last turn did not end in an error"
        );
    }

    #[test]
    fn retry_prompt_carries_the_last_error() {
        let events = vec![
            Event::ChatError {
                error: "boom".into(),
                recoverable: false,
            },
            Event::ChatDone {
                message_id: "m".into(),
                reason: DoneReason::Error,
                duration_ms: None,
            },
        ];
        let prompt = retry_prompt(&events).unwrap();
        assert!(prompt.contains("boom"));
        assert!(prompt.contains(CONTINUE_AFTER_STREAM_ERROR_PROMPT));
        assert_eq!(retry_prompt(&[]).unwrap_err(), "no turn to retry");
    }

    #[test]
    fn a_prompt_with_an_attachment_sends_text_and_image_parts() {
        let model = Scripted(RefCell::new(vec![reply("done", vec![])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        let attachment = Attachment {
            id: "a".into(),
            filename: "a.png".into(),
            mime_type: "image/png".into(),
            size: 1,
            data: "data:image/png;base64,AAAA".into(),
        };
        run_turn(&mut h.turn(), "look", &[attachment]).unwrap();
        let events = h.log.0.borrow().clone();
        let msgs = fold_messages(&events);
        let user = msgs
            .iter()
            .find(|m| m["role"] == "user" && m["content"].as_array().is_some())
            .unwrap();
        assert_eq!(user["content"][0], json!({"type": "text", "text": "look"}));
        assert_eq!(
            user["content"][1],
            json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}})
        );
    }

    #[test]
    fn the_tool_definitions_are_identical_in_plan_and_builder_modes() {
        let plan_model = RecordingTools::default();
        let builder_model = RecordingTools::default();
        let stop = AtomicBool::new(false);

        // One session's agents in both modes: suggest_agent's definition lists them.
        let agents = [builder(), vec![plan()]].concat();
        let mut plan_h = Harness::new(Box::new(plan_model.clone()), PathBuf::from("/tmp"), &stop);
        plan_h.agents = agents.clone();
        run_turn(&mut plan_h.turn(), "go", &[]).unwrap();

        let mut builder_h = Harness::new(
            Box::new(builder_model.clone()),
            PathBuf::from("/tmp"),
            &stop,
        );
        builder_h.agents = agents;
        builder_h.log.0.borrow_mut().push(Event::ModeChanged {
            mode: "builder".into(),
            auto: false,
            reason: None,
        });
        run_turn(&mut builder_h.turn(), "go", &[]).unwrap();

        assert_eq!(*plan_model.0.borrow(), *builder_model.0.borrow());
    }

    fn run_with_fake_tools(
        model: Box<dyn Model>,
        agents: Vec<AgentDefinition>,
    ) -> (DoneReason, Vec<Event>) {
        let log = MemoryLog::default();
        let tools = FakeToolbox;
        let stop = AtomicBool::new(false);
        let mut on_event = |_: &Event| {};
        let mut turn = Turn {
            model: &*model,
            log: &log,
            tools: &tools,
            session_id: "s",
            workdir: Path::new("/tmp"),
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            session_effort: None,
            autonomous: false,
            lang: crate::domain::lang::Lang::Fr,
            preflight_sent_back: false,
            stop: &stop,
            on_event: &mut on_event,
            children: None,
        };
        let reason = run_turn(&mut turn, "go", &[]).unwrap();
        (reason, log.0.into_inner())
    }

    #[test]
    fn a_plan_with_a_check_that_passes_already_is_sent_back_once() {
        let plan_text = "## Done when\n- [ ] builds — check: `true`\n";
        let model = Box::new(Scripted(RefCell::new(vec![
            reply(plan_text, vec![]),
            reply(plan_text, vec![]),
        ])));
        let (reason, events) = run_with_fake_tools(model, vec![plan()]);
        assert_eq!(reason, DoneReason::Complete);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::CriteriaPreflight { .. }))
                .count(),
            2
        );
        assert!(events.iter().any(|e| matches!(
            e,
            Event::MessageStart {
                role: Role::User,
                is_system_generated: true,
                content: Some(c),
                ..
            } if c.contains("passes already")
        )));
    }

    #[test]
    fn a_builder_plan_with_a_check_is_not_preflighted() {
        let plan_text = "## Done when\n- [ ] builds — check: `true`\n";
        let model = Box::new(Scripted(RefCell::new(vec![reply(plan_text, vec![])])));
        let (_, events) = run_with_fake_tools(model, builder());
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::CriteriaPreflight { .. }))
        );
    }
}
