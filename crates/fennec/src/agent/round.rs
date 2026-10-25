//! One model round: stream the model's answer into the log, retrying a
//! request that fails before any content (openfox case 1) and asking to
//! continue after a stream that breaks mid-answer (case 2).

use super::ports::{Completion, Delta, Request};
use super::turn::Turn;
use crate::domain::conversation::fold_messages;
use crate::domain::event::{DoneReason, Event, Role};
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::time::Duration;

pub const CONTINUE_AFTER_STREAM_ERROR_PROMPT: &str = "The LLM stream was interrupted mid-response. Continue exactly where you left off — do not repeat what was already written.";
const MAX_REQUEST_RETRIES: u32 = 3;

pub struct RoundPrompt<'a> {
    pub system_prompt: &'a str,
    pub tools: &'a Value,
    pub max_tokens: u32,
}

pub enum Round {
    /// The model answered; its message is started, not yet done.
    Answered(Completion),
    /// The stream broke mid-answer: the partial answer is kept and the model
    /// asked to continue in the next round.
    Interrupted,
    /// The turn must end (stopped by the human, or the model unreachable).
    Ended(DoneReason),
}

pub fn stream(turn: &mut Turn, message_id: &str, prompt: &RoundPrompt) -> Result<Round, String> {
    let mut started = false;
    let mut failures = 0;
    loop {
        let attempt = stream_once(turn, message_id, prompt, &mut started)?;
        let error = match attempt {
            Ok(answer) => {
                if !started {
                    turn.append(assistant_start(message_id))?;
                }
                return Ok(Round::Answered(answer));
            }
            Err(error) => error,
        };
        if turn.stop.load(Ordering::Relaxed) {
            if started {
                end_partial(turn, message_id)?;
            }
            return Ok(Round::Ended(DoneReason::Stopped));
        }
        if started {
            end_partial(turn, message_id)?;
            turn.append(Event::ChatError {
                error,
                recoverable: true,
            })?;
            turn.append_user(CONTINUE_AFTER_STREAM_ERROR_PROMPT, true)?;
            return Ok(Round::Interrupted);
        }
        failures += 1;
        if failures > MAX_REQUEST_RETRIES {
            turn.append(Event::ChatError {
                error,
                recoverable: false,
            })?;
            return Ok(Round::Ended(DoneReason::Error));
        }
        std::thread::sleep(Duration::from_secs(1 << (failures - 1)));
    }
}

/// One request. The outer `Result` is the log failing (fatal), the inner
/// one the model failing (retried by `stream`).
fn stream_once(
    turn: &mut Turn,
    message_id: &str,
    prompt: &RoundPrompt,
    started: &mut bool,
) -> Result<Result<Completion, String>, String> {
    let messages = fold_messages(&turn.log.events(turn.session_id)?);
    let req = Request {
        system_prompt: prompt.system_prompt,
        messages: &messages,
        tools: prompt.tools,
        max_tokens: prompt.max_tokens,
        stop: turn.stop,
    };
    let model = turn.model;
    let mut log_error = None;
    let answer = model.stream(&req, &mut |delta| {
        if log_error.is_some() {
            return;
        }
        let event = match delta {
            Delta::Text(content) => Event::MessageDelta {
                message_id: message_id.into(),
                content,
            },
            Delta::Thinking(content) => Event::MessageThinking {
                message_id: message_id.into(),
                content,
            },
            Delta::ToolPreparing { .. } => return,
        };
        // The assistant message starts with its first content, so a request
        // that fails before any leaves nothing in the log.
        if !*started {
            *started = true;
            log_error = turn.append(assistant_start(message_id)).err();
        }
        if log_error.is_none() {
            log_error = turn.append(event).err();
        }
    });
    match log_error {
        Some(e) => Err(e),
        None => Ok(answer),
    }
}

fn end_partial(turn: &mut Turn, message_id: &str) -> Result<(), String> {
    turn.append(Event::MessageDone {
        message_id: message_id.into(),
        partial: true,
    })
}

fn assistant_start(message_id: &str) -> Event {
    Event::MessageStart {
        message_id: message_id.into(),
        role: Role::Assistant,
        content: None,
        is_system_generated: false,
    }
}
