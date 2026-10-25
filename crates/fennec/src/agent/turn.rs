//! Port of openfox `chat/agent-loop.ts`: the multi-turn loop. Each round
//! streams the model (`round`), then runs its tool calls, until it answers
//! without one.
//!
//! Not ported yet: compaction, auto-retry patterns, vision fallback, queue,
//! pause gate, sub-agents, return_value/step_done, parallel tool batches.

use super::ports::{EventLog, Model, Toolbox};
use super::prompt::system_prompt;
use super::round::{self, Round, RoundPrompt};
use super::tool_calls::execute_tools;
use crate::domain::event::{DoneReason, Event, Role};
use crate::domain::id::new_id;
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
    turn.append(Event::RunningChanged { is_running: true })?;
    let result = turn
        .append_user(prompt, false)
        .and_then(|()| run_top_level_agent_loop(turn));
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
    let system_prompt = cached_system_prompt(turn)?;
    let tool_defs = turn.tools.definitions();
    let mut max_tokens = turn.max_tokens;
    let mut truncation_retries = 0;

    for _ in 0..MAX_ROUNDS {
        let message_id = new_id();
        if turn.stop.load(Ordering::Relaxed) {
            return finish(turn, message_id, DoneReason::Stopped);
        }
        let prompt = RoundPrompt {
            system_prompt: &system_prompt,
            tools: &tool_defs,
            max_tokens,
        };
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
        execute_tools(turn, &message_id, &answer.tool_calls)?;
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
    use crate::domain::conversation::fold_messages;
    use crate::domain::event::ToolCall;
    use crate::tools::LocalTools;
    use serde_json::json;
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// Plays back one scripted reply per call, like openfox's mock LLM.
    struct Scripted(RefCell<Vec<Result<Completion, String>>>);

    impl Model for Scripted {
        fn name(&self) -> &str {
            "scripted"
        }

        fn stream(&self, _: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
            let next = self.0.borrow_mut().remove(0);
            if let Ok(c) = &next
                && !c.content.is_empty()
            {
                on(Delta::Text(c.content.clone()));
            }
            next
        }
    }

    /// An in-memory log: the loop's tests do not need SQLite.
    #[derive(Default)]
    struct MemoryLog(RefCell<Vec<Event>>);

    impl EventLog for MemoryLog {
        fn append(&self, _: &str, event: &Event) -> Result<u64, String> {
            self.0.borrow_mut().push(event.clone());
            Ok(self.0.borrow().len() as u64)
        }

        fn events(&self, _: &str) -> Result<Vec<Event>, String> {
            Ok(self.0.borrow().clone())
        }
    }

    fn reply(content: &str, calls: Vec<ToolCall>) -> Result<Completion, String> {
        Ok(Completion {
            content: content.into(),
            finish_reason: if calls.is_empty() {
                "stop"
            } else {
                "tool_calls"
            }
            .into(),
            tool_calls: calls,
            ..Completion::default()
        })
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
}
