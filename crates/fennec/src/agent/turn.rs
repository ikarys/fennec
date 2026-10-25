//! Port of openfox `chat/agent-loop.ts`: the multi-turn loop. Each round
//! streams the model (`round`), then runs its tool calls, until it answers
//! without one.
//!
//! `ask_user` is ported: the turn ends with `waiting_for_user` and the
//! answer arrives as the next turn's input, so a pending question survives a
//! restart.
//!
//! Compaction is ported: at a threshold the loop asks the model for a
//! summary and opens a new window from it (`compaction`), and
//! `compact_session` runs it on demand.
//!
//! Not ported yet: auto-retry patterns, vision fallback, queue, pause gate,
//! sub-agents, return_value/step_done, parallel tool batches.

use super::ask_user;
use super::compaction::{self, Compacted};
use super::ports::{EventLog, Model, Toolbox};
use super::prompt::system_prompt;
use super::round::{self, Round, RoundPrompt};
use super::tool_calls::{Flow, INTERRUPTED_ERROR, execute_tools};
use crate::domain::compaction::{context_tokens, should_compact};
use crate::domain::event::{DoneReason, Event, Role, ToolResult};
use crate::domain::id::new_id;
use crate::domain::open_calls;
use crate::domain::preferences::ReasoningEffort;
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const CONTINUE_AFTER_TRUNCATION_PROMPT: &str =
    "Continue your previous response exactly where you left off.";
const MAX_TRUNCATION_RETRIES: u32 = 3;
/// openfox loops until aborted; a cap also keeps a confused model from
/// running forever when nobody watches.
const MAX_ROUNDS: u32 = 200;

/// One session's turn and everything it runs against.
pub struct Turn<'a> {
    pub model: &'a dyn Model,
    pub log: &'a dyn EventLog,
    pub tools: &'a dyn Toolbox,
    pub session_id: &'a str,
    pub workdir: &'a Path,
    pub max_tokens: u32,
    /// The model's context window, reported in `context.state`.
    pub context_window: u64,
    /// The compaction threshold as a fraction of the window; 0 disables it.
    pub compact_at: f64,
    /// openfox `reasoningEffort`; `None` sends no `reasoning_effort` field.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Set by the human to stop the turn: checked between rounds, while
    /// streaming and while a command runs.
    pub stop: &'a AtomicBool,
    /// Called for every event right after it is stored (the CLI prints them).
    pub on_event: &'a mut dyn FnMut(&Event),
}

impl Turn<'_> {
    pub(super) fn append(&mut self, event: Event) -> Result<(), String> {
        self.log.append(self.session_id, &event)?;
        (self.on_event)(&event);
        Ok(())
    }

    pub(super) fn append_user(
        &mut self,
        content: &str,
        system_generated: bool,
    ) -> Result<(), String> {
        let message_id = new_id();
        self.append(Event::MessageStart {
            message_id: message_id.clone(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: system_generated,
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
pub fn run_turn(turn: &mut Turn, prompt: &str) -> Result<DoneReason, String> {
    with_running(turn, |turn| {
        // Close tool calls left open by a previous turn before writing the
        // prompt: the model API rejects an assistant tool call without its
        // tool message. When the open call awaits an answer, the human's
        // input *is* that answer, so no user message is appended on top of it.
        let answered = close_open_calls(turn, prompt)?;
        if answered {
            run_top_level_agent_loop(turn)
        } else {
            turn.append_user(prompt, false)
                .and_then(|()| run_top_level_agent_loop(turn))
        }
    })
}

/// The human's manual compaction: summarise the history and open a new
/// window, then end the turn. Refuses while a question is pending.
pub fn compact_session(turn: &mut Turn) -> Result<DoneReason, String> {
    with_running(turn, |turn| {
        let events = turn.log.events(turn.session_id)?;
        if !open_calls::open_calls(&events).is_empty() {
            return Err("answer the pending question or send a message first".into());
        }
        let (system_prompt, tool_defs) = round_prompt_parts(turn)?;
        let prompt = RoundPrompt {
            system_prompt: &system_prompt,
            tools: &tool_defs,
            max_tokens: turn.max_tokens,
            reasoning_effort: turn.reasoning_effort,
        };
        match compaction::compact(turn, &prompt)? {
            Compacted::Done | Compacted::Skipped => finish(turn, new_id(), DoneReason::Complete),
            Compacted::Ended(reason) => finish(turn, new_id(), reason),
        }
    })
}

/// Wraps a turn: mark it running, record a `chat.error` on failure, then
/// mark it not running. Shared by `run_turn` and `compact_session`.
fn with_running(
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
fn round_prompt_parts(turn: &mut Turn) -> Result<(String, Value), String> {
    let system_prompt = cached_system_prompt(turn)?;
    // The loop owns `ask_user`; it is not a host tool, so its definition is
    // added to the toolbox's.
    let mut tool_defs = turn.tools.definitions();
    if let Some(arr) = tool_defs.as_array_mut() {
        arr.push(ask_user::definition());
    }
    Ok((system_prompt, tool_defs))
}

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
            ToolResult::success(prompt.to_string(), false)
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

/// openfox `cachedSystemPrompt`: built at the session's first turn, then
/// read back, so a new day or a new fennec build does not change the
/// prompt's first bytes and void the whole session's prompt cache.
fn cached_system_prompt(turn: &mut Turn) -> Result<String, String> {
    let cached = turn
        .log
        .events(turn.session_id)?
        .into_iter()
        .find_map(|e| match e {
            Event::TurnSnapshot {
                cached_system_prompt,
            } => Some(cached_system_prompt),
            _ => None,
        });
    if let Some(prompt) = cached {
        return Ok(prompt);
    }
    let prompt = system_prompt(turn.workdir, turn.model.name());
    turn.append(Event::TurnSnapshot {
        cached_system_prompt: prompt.clone(),
    })?;
    Ok(prompt)
}

fn run_top_level_agent_loop(turn: &mut Turn) -> Result<DoneReason, String> {
    let (system_prompt, tool_defs) = round_prompt_parts(turn)?;
    let mut max_tokens = turn.max_tokens;
    let mut truncation_retries = 0;
    // Guards against compacting again before a real round has measured the
    // new window.
    let mut compacted_last_round = false;

    for _ in 0..MAX_ROUNDS {
        let message_id = new_id();
        if turn.stop.load(Ordering::Relaxed) {
            return finish(turn, message_id, DoneReason::Stopped);
        }
        let prompt = RoundPrompt {
            system_prompt: &system_prompt,
            tools: &tool_defs,
            max_tokens,
            reasoning_effort: turn.reasoning_effort,
        };
        if !compacted_last_round
            && should_compact(
                context_tokens(&turn.log.events(turn.session_id)?),
                turn.context_window,
                turn.compact_at,
            )
        {
            match compaction::compact(turn, &prompt)? {
                Compacted::Ended(reason) => return finish(turn, message_id, reason),
                Compacted::Done | Compacted::Skipped => {}
            }
            compacted_last_round = true;
            continue;
        }
        compacted_last_round = false;
        let (answer, stats) = match round::stream(turn, &message_id, &prompt)? {
            Round::Answered(answer, stats) => (answer, stats),
            Round::Interrupted => continue,
            Round::Ended(reason) => return finish(turn, message_id, reason),
        };
        turn.append(Event::ContextState {
            current_tokens: answer.prompt_tokens + answer.completion_tokens,
            max_tokens: turn.context_window,
        })?;

        if answer.finish_reason == "length" && answer.tool_calls.is_empty() {
            let give_up = truncation_retries >= MAX_TRUNCATION_RETRIES;
            turn.append(Event::MessageDone {
                message_id: message_id.clone(),
                stats: Some(stats),
                partial: give_up,
            })?;
            if give_up {
                return finish(turn, message_id, DoneReason::Truncated);
            }
            truncation_retries += 1;
            max_tokens = max_tokens.saturating_mul(3) / 2;
            turn.append_user(CONTINUE_AFTER_TRUNCATION_PROMPT, true)?;
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
            return finish(turn, message_id, DoneReason::Complete);
        }
        match execute_tools(turn, &message_id, &answer.tool_calls)? {
            Flow::WaitingForUser => return finish(turn, message_id, DoneReason::WaitingForUser),
            Flow::Continue => {}
        }
    }

    turn.append(Event::ChatError {
        error: format!("stopped after {MAX_ROUNDS} rounds"),
        recoverable: false,
    })?;
    finish(turn, new_id(), DoneReason::Stopped)
}

/// Records how the turn ended.
fn finish(turn: &mut Turn, message_id: String, reason: DoneReason) -> Result<DoneReason, String> {
    turn.append(Event::ChatDone { message_id, reason })?;
    Ok(reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::{Completion, Delta, Request};
    use crate::agent::round::CONTINUE_AFTER_STREAM_ERROR_PROMPT;
    use crate::agent::test_support::{MemoryLog, Scripted, reply};
    use crate::domain::conversation::fold_messages;
    use crate::domain::event::ToolCall;
    use crate::tools::LocalTools;
    use serde_json::json;
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
            }
        }

        fn turn(&mut self) -> Turn<'_> {
            Turn {
                model: &*self.model,
                log: &self.log,
                tools: &self.tools,
                session_id: "s",
                workdir: &self.workdir,
                max_tokens: 100,
                context_window: 1000,
                compact_at: 0.0,
                reasoning_effort: None,
                stop: self.stop,
                on_event: &mut self.on_event,
            }
        }
    }

    fn run(model: &dyn Model, workdir: &Path) -> (DoneReason, Vec<Event>) {
        let log = MemoryLog::default();
        let tools = LocalTools::new(workdir.to_path_buf());
        let stop = AtomicBool::new(false);
        let mut on_event = |_: &Event| {};
        let mut turn = Turn {
            model,
            log: &log,
            tools: &tools,
            session_id: "s",
            workdir,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            stop: &stop,
            on_event: &mut on_event,
        };
        let reason = run_turn(&mut turn, "go").unwrap();
        (reason, log.0.into_inner())
    }

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fennec-turn-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tool_call_round_then_final_answer() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: json!({"path": "hello.txt", "content": "hi"}),
            parse_error: None,
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
        assert_eq!(msgs[2]["role"], "tool");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_loop_compacts_past_the_threshold_then_goes_on() {
        let call = ToolCall {
            id: "c1".into(),
            name: "run_command".into(),
            arguments: json!({}),
            parse_error: Some("bad".into()),
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
        assert_eq!(run_turn(&mut turn, "go").unwrap(), DoneReason::Complete);
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
        let msgs = fold_messages(&events);
        let contents: Vec<_> = msgs
            .iter()
            .map(|m| m["content"].as_str().unwrap())
            .collect();
        assert_eq!(
            contents,
            ["go", "part", CONTINUE_AFTER_STREAM_ERROR_PROMPT, "part"]
        );
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
            },
        )
        .unwrap();
        let model = Recording::default();
        let workdir = std::env::temp_dir();
        let tools = LocalTools::new(workdir.clone());
        let stop = AtomicBool::new(false);
        let mut on_event = |_: &Event| {};
        let mut turn = Turn {
            model: &model,
            log: &log,
            tools: &tools,
            session_id: "s",
            workdir: &workdir,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            stop: &stop,
            on_event: &mut on_event,
        };
        run_turn(&mut turn, "one").unwrap();
        run_turn(&mut turn, "two").unwrap();
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
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let reason = run_turn(&mut h.turn(), "go").unwrap();
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
        let reason2 = run_turn(&mut h.turn(), "blue").unwrap();
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
    fn a_tool_after_ask_user_in_the_same_batch_is_not_run() {
        let dir = scratch_dir();
        let ask = ToolCall {
            id: "c1".into(),
            name: "ask_user".into(),
            arguments: json!({"question": "Proceed?"}),
            parse_error: None,
        };
        let read = ToolCall {
            id: "c2".into(),
            name: "read_file".into(),
            arguments: json!({"path": "x.txt"}),
            parse_error: None,
        };
        let model = Scripted(RefCell::new(vec![reply("", vec![ask, read])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        let reason = run_turn(&mut h.turn(), "go").unwrap();
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
        let reason = run_turn(&mut h.turn(), "go").unwrap();
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
                    },
                },
            )
            .unwrap();
        let reason = run_turn(&mut h.turn(), "hi").unwrap();
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
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
        let reason = run_turn(&mut h.turn(), "go").unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::ChatAskUser { .. }))
        );
        assert!(events.iter().any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.error.is_some())));
    }
}
