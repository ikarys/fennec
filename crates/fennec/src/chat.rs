//! Port of openfox `src/server/chat/{agent-loop,execute-tools,prompts}.ts`:
//! the multi-turn loop. Each round folds the event log into messages,
//! streams the model, records what it said, runs its tool calls and loops
//! until it answers without calling a tool.
//!
//! Not ported yet: compaction, auto-retry patterns, vision fallback, queue,
//! pause gate, sub-agents, return_value/step_done, parallel tool batches.

use crate::events::{Event, Store, ToolCall, ToolResult, fold_messages, new_id};
use crate::llm::{Delta, Model, Request};
use crate::tools;
use std::path::Path;
use std::time::Duration;

pub const CONTINUE_AFTER_STREAM_ERROR_PROMPT: &str = "The LLM stream was interrupted mid-response. Continue exactly where you left off — do not repeat what was already written.";
const CONTINUE_AFTER_TRUNCATION_PROMPT: &str =
    "Continue your previous response exactly where you left off.";
const MAX_TRUNCATION_RETRIES: u32 = 3;
const MAX_REQUEST_RETRIES: u32 = 3;
/// openfox loops until aborted; without an abort button yet, a cap keeps a
/// confused model from running forever.
const MAX_ROUNDS: u32 = 200;

pub struct LoopConfig<'a> {
    pub model: &'a dyn Model,
    pub model_name: &'a str,
    pub store: &'a Store,
    pub session_id: &'a str,
    pub workdir: &'a Path,
    pub max_tokens: u32,
    /// Called for every event right after it is stored (the CLI prints them).
    pub on_event: &'a mut dyn FnMut(&Event),
}

impl LoopConfig<'_> {
    fn append(&mut self, event: Event) -> Result<(), String> {
        self.store
            .append(self.session_id, &event)
            .map_err(|e| format!("event store: {e}"))?;
        (self.on_event)(&event);
        Ok(())
    }

    fn append_user(&mut self, content: &str, system_generated: bool) -> Result<(), String> {
        let message_id = new_id();
        self.append(Event::MessageStart {
            message_id: message_id.clone(),
            role: "user".into(),
            content: Some(content.into()),
            is_system_generated: system_generated,
        })?;
        self.append(Event::MessageDone {
            message_id,
            partial: false,
        })
    }
}

/// Appends the user's prompt and runs the loop to completion. Returns the
/// `chat.done` reason.
pub fn run_turn(cfg: &mut LoopConfig, prompt: &str) -> Result<String, String> {
    cfg.append_user(prompt, false)?;
    run_top_level_agent_loop(cfg)
}

pub fn run_top_level_agent_loop(cfg: &mut LoopConfig) -> Result<String, String> {
    let system_prompt = build_base_prompt(cfg.workdir, cfg.model_name);
    let tool_defs = tools::definitions();
    let mut max_tokens = cfg.max_tokens;
    let mut truncation_retries = 0;

    for _ in 0..MAX_ROUNDS {
        let message_id = new_id();
        let mut started = false;
        let mut failures = 0;

        // Case 1 (openfox): a request that fails before any content is
        // retried with backoff and leaves nothing behind. Case 2: a stream
        // that fails mid-flight keeps its partial content and gets one
        // continuation prompt.
        let result = loop {
            let messages = fold_messages(
                &cfg.store
                    .events(cfg.session_id)
                    .map_err(|e| e.to_string())?,
            );
            let req = Request {
                system_prompt: &system_prompt,
                messages: &messages,
                tools: &tool_defs,
                max_tokens,
            };
            let mut store_err = None;
            let model = cfg.model;
            let attempt = model.stream(&req, &mut |delta| {
                if store_err.is_some() {
                    return;
                }
                let ev = match delta {
                    Delta::Text(content) => Event::MessageDelta {
                        message_id: message_id.clone(),
                        content,
                    },
                    Delta::Thinking(content) => Event::MessageThinking {
                        message_id: message_id.clone(),
                        content,
                    },
                    Delta::ToolPreparing { .. } => return,
                };
                if !started {
                    started = true;
                    if let Err(e) = cfg.append(assistant_start(&message_id)) {
                        store_err = Some(e);
                        return;
                    }
                }
                if let Err(e) = cfg.append(ev) {
                    store_err = Some(e);
                }
            });
            if let Some(e) = store_err {
                return Err(e);
            }
            match attempt {
                Ok(c) => break Some(c),
                Err(e) if started => {
                    cfg.append(Event::MessageDone {
                        message_id: message_id.clone(),
                        partial: true,
                    })?;
                    cfg.append(Event::ChatError {
                        error: e,
                        recoverable: true,
                    })?;
                    cfg.append_user(CONTINUE_AFTER_STREAM_ERROR_PROMPT, true)?;
                    break None;
                }
                Err(e) => {
                    failures += 1;
                    if failures > MAX_REQUEST_RETRIES {
                        cfg.append(Event::ChatError {
                            error: e,
                            recoverable: false,
                        })?;
                        cfg.append(Event::ChatDone {
                            message_id,
                            reason: "error".into(),
                        })?;
                        return Ok("error".into());
                    }
                    std::thread::sleep(Duration::from_secs(1 << (failures - 1)));
                }
            }
        };
        let Some(result) = result else { continue };
        if !started {
            cfg.append(assistant_start(&message_id))?;
        }

        if result.finish_reason == "length" && result.tool_calls.is_empty() {
            cfg.append(Event::MessageDone {
                message_id: message_id.clone(),
                partial: truncation_retries >= MAX_TRUNCATION_RETRIES,
            })?;
            if truncation_retries >= MAX_TRUNCATION_RETRIES {
                cfg.append(Event::ChatDone {
                    message_id,
                    reason: "truncated".into(),
                })?;
                return Ok("truncated".into());
            }
            truncation_retries += 1;
            max_tokens = max_tokens.saturating_mul(3) / 2;
            cfg.append_user(CONTINUE_AFTER_TRUNCATION_PROMPT, true)?;
            continue;
        }

        if result.tool_calls.is_empty() {
            cfg.append(Event::MessageDone {
                message_id: message_id.clone(),
                partial: false,
            })?;
            cfg.append(Event::ChatDone {
                message_id,
                reason: "complete".into(),
            })?;
            return Ok("complete".into());
        }

        for call in &result.tool_calls {
            cfg.append(Event::ToolCall {
                message_id: message_id.clone(),
                tool_call: call.clone(),
            })?;
        }
        cfg.append(Event::MessageDone {
            message_id: message_id.clone(),
            partial: false,
        })?;
        execute_tools(cfg, &message_id, &result.tool_calls)?;
    }

    cfg.append(Event::ChatError {
        error: format!("stopped after {MAX_ROUNDS} rounds"),
        recoverable: false,
    })?;
    cfg.append(Event::ChatDone {
        message_id: new_id(),
        reason: "stopped".into(),
    })?;
    Ok("stopped".into())
}

fn assistant_start(message_id: &str) -> Event {
    Event::MessageStart {
        message_id: message_id.into(),
        role: "assistant".into(),
        content: None,
        is_system_generated: false,
    }
}

/// Port of openfox `executeTools`, sequential for now.
fn execute_tools(cfg: &mut LoopConfig, message_id: &str, calls: &[ToolCall]) -> Result<(), String> {
    for call in calls {
        let result = match &call.parse_error {
            Some(pe) => ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Failed to parse tool call arguments: {pe}. Please ensure your JSON function call arguments are valid."
                )),
                duration_ms: 0,
                truncated: false,
            },
            None => {
                let workdir = cfg.workdir;
                let mut store_err = None;
                let r = tools::execute(
                    &call.name,
                    &call.arguments,
                    workdir,
                    &mut |stream, chunk| {
                        if store_err.is_none() {
                            store_err = cfg
                                .append(Event::ToolOutput {
                                    message_id: message_id.into(),
                                    tool_call_id: call.id.clone(),
                                    stream: stream.into(),
                                    content: chunk.into(),
                                })
                                .err();
                        }
                    },
                );
                if let Some(e) = store_err {
                    return Err(e);
                }
                r
            }
        };
        cfg.append(Event::ToolResult {
            message_id: message_id.into(),
            tool_call_id: call.id.clone(),
            result,
        })?;
    }
    Ok(())
}

/// Port of openfox `buildBasePrompt`, without the sections for features
/// not ported yet (skills, sub-agents, modes, caveman thinking).
pub fn build_base_prompt(workdir: &Path, model_name: &str) -> String {
    let today = crate::events::now_ms() / 86_400_000;
    let (y, m, d) = civil_from_days(today as i64);
    format!(
        "You are fennec, an agentic assistant.

Today's date is {y:04}/{m:02}/{d:02}

## ENVIRONMENT
Working directory: {workdir}
Platform: {os} ({arch})
Shell (run_command): sh
Model: {model_name}

## CORE BEHAVIOR
Help user complete tasks safely and efficiently.
Do everything to satisfy user requirements.
Read file before editing.
Prefer precise, minimal changes.
Use available tools when needed.
Explain tradeoffs clearly when requirements are ambiguous.
Follow repository and project instructions exactly.
Commands run from your working directory — do not prepend \"cd /path/\" to change to a directory you are already in.

## TONE AND STYLE
Be concise, direct, and to the point.
Explain non-trivial bash commands (what & why).
Output renders as CLI markdown (CommonMark).
Only use tools for tasks, not communication.
No emojis unless asked.
Minimize tokens, stay on-topic.
No preamble/postamble.
Keep responses under ~4 lines unless detail requested.

## IMPORTANT GUARDRAILS
NEVER delete/git checkout already modified file: would result in data loss.

# Proactiveness
Allowed to be proactive, but only when user asks you to do something. Balance:
1. Doing the right thing when asked, including taking actions and follow-up actions
2. Not surprising user with actions you take without asking
For example, if user asks you how to approach something, you should do your best to answer their question first, and not immediately jump into taking actions.
3. Do not add additional code explanation summary unless requested by user. After working on a file, just stop, rather than providing an explanation of what you did.

# Following conventions
Understand file's code conventions. Mimic code style, use existing libraries and utilities, follow existing patterns.
NEVER assume that a given library is available. To use library or framework, first check that codebase already uses given library.
When you edit code, look at code's surrounding context (imports) to understand code's choice of frameworks and libraries. Then consider how to make given change in way that is most idiomatic.
Always follow security best practices. Never introduce code that exposes or logs secrets and keys. Never commit secrets or keys to the repository.

# Doing tasks
Use run_command to understand the codebase and user's query.
Implement solution using all tools available to you.
Verify the solution if possible with tests. NEVER assume specific test framework or test script. Check README or search codebase to determine testing approach.
NEVER commit changes. Integration is the user's decision.

# Tool usage policy
You have the capability to call multiple tools in a single response. When multiple independent pieces of information are requested, batch your tool calls together for optimal performance.
",
        workdir = workdir.display(),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
    )
}

/// Days since 1970-01-01 to (year, month, day), Howard Hinnant's algorithm;
/// avoids a date crate for one line of the prompt.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::Completion;
    use serde_json::json;
    use std::cell::RefCell;

    /// Plays back one scripted reply per call, like openfox's mock LLM.
    struct Scripted(RefCell<Vec<Result<Completion, String>>>);

    impl Model for Scripted {
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

    fn run(script: Vec<Result<Completion, String>>, workdir: &Path) -> (String, Vec<Event>) {
        let store = Store::in_memory().unwrap();
        let sid = store
            .create_session(&workdir.display().to_string(), "t")
            .unwrap();
        let model = Scripted(RefCell::new(script));
        let mut on_event = |_: &Event| {};
        let mut cfg = LoopConfig {
            model: &model,
            model_name: "mock",
            store: &store,
            session_id: &sid,
            workdir,
            max_tokens: 100,
            on_event: &mut on_event,
        };
        let reason = run_turn(&mut cfg, "go").unwrap();
        (reason, store.events(&sid).unwrap())
    }

    #[test]
    fn tool_call_round_then_final_answer() {
        let dir = std::env::temp_dir().join(format!("fennec-chat-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let call = ToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: json!({"path": "hello.txt", "content": "hi"}),
            parse_error: None,
        };
        let (reason, events) = run(vec![reply("", vec![call]), reply("done", vec![])], &dir);
        assert_eq!(reason, "complete");
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
        let (_, events) = run(
            vec![reply("", vec![call]), reply("ok", vec![])],
            Path::new("/tmp"),
        );
        assert!(events.iter().any(|e| matches!(e,
            Event::ToolResult { result, .. } if result.error.as_deref().unwrap().starts_with("Failed to parse"))));
    }

    #[test]
    fn mid_stream_failure_keeps_partial_and_asks_to_continue() {
        struct FailsMidStream(RefCell<u32>);
        impl Model for FailsMidStream {
            fn stream(&self, _: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
                *self.0.borrow_mut() += 1;
                on(Delta::Text("part".into()));
                if *self.0.borrow() == 1 {
                    return Err("reset".into());
                }
                reply("part", vec![])
            }
        }
        let store = Store::in_memory().unwrap();
        let sid = store.create_session("/tmp", "t").unwrap();
        let mut on_event = |_: &Event| {};
        let mut cfg = LoopConfig {
            model: &FailsMidStream(RefCell::new(0)),
            model_name: "mock",
            store: &store,
            session_id: &sid,
            workdir: Path::new("/tmp"),
            max_tokens: 100,
            on_event: &mut on_event,
        };
        assert_eq!(run_turn(&mut cfg, "go").unwrap(), "complete");
        let msgs = fold_messages(&store.events(&sid).unwrap());
        let contents: Vec<_> = msgs
            .iter()
            .map(|m| m["content"].as_str().unwrap())
            .collect();
        assert_eq!(
            contents,
            ["go", "part", CONTINUE_AFTER_STREAM_ERROR_PROMPT, "part"]
        );
    }

    #[test]
    fn civil_date() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_751), (2026, 10, 25));
    }
}
