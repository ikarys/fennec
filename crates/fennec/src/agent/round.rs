//! One model round: stream the model's answer into the log, retrying a
//! request that fails before any content (openfox case 1) and asking to
//! continue after a stream that breaks mid-answer (case 2).

use super::delta_buffer::{DeltaBuffer, DeltaKind};
use super::ports::{Completion, Delta, Request};
use super::turn::Turn;
use crate::domain::conversation::fold_messages;
use crate::domain::event::{DoneReason, Event, Role};
use crate::domain::preferences::ReasoningEffort;
use crate::domain::stats::{MessageStats, Usage};
use serde_json::Value;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub const CONTINUE_AFTER_STREAM_ERROR_PROMPT: &str = "The LLM stream was interrupted mid-response. Continue exactly where you left off — do not repeat what was already written.";
const MAX_REQUEST_RETRIES: u32 = 3;

pub struct RoundPrompt<'a> {
    pub system_prompt: &'a str,
    pub tools: &'a Value,
    pub max_tokens: u32,
    pub reasoning_effort: Option<ReasoningEffort>,
}

pub enum Round {
    /// The model answered; its message is started, not yet done.
    Answered(Completion, MessageStats),
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
            Ok((answer, stats)) => {
                if !started {
                    turn.append(assistant_start(message_id))?;
                }
                return Ok(Round::Answered(answer, stats));
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
            turn.append_user(CONTINUE_AFTER_STREAM_ERROR_PROMPT, true, None, Vec::new())?;
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
) -> Result<Result<(Completion, MessageStats), String>, String> {
    let messages = fold_messages(&turn.log.events(turn.session_id)?);
    let req = Request {
        system_prompt: prompt.system_prompt,
        messages: &messages,
        tools: prompt.tools,
        max_tokens: prompt.max_tokens,
        reasoning_effort: prompt.reasoning_effort,
        stop: turn.stop,
    };
    let model = turn.model;
    let mut buffer = DeltaBuffer::new(message_id);
    let mut log_error = None;
    let sent = Instant::now();
    let mut first_token = None;
    let answer = model.stream(&req, &mut |delta| {
        first_token.get_or_insert_with(|| sent.elapsed());
        if log_error.is_some() {
            return;
        }
        let (kind, chunk) = match &delta {
            Delta::Text(chunk) => (DeltaKind::Text, chunk),
            Delta::Thinking(chunk) => (DeltaKind::Thinking, chunk),
            Delta::ToolPreparing { .. } => return,
        };
        // The assistant message starts with its first content, so a request
        // that fails before any leaves nothing in the log.
        if !*started {
            *started = true;
            log_error = turn.append(assistant_start(message_id)).err();
        }
        for event in buffer.push(kind, chunk, Instant::now()) {
            if log_error.is_none() {
                log_error = turn.append(event).err();
            }
        }
    });
    if let (None, Some(event)) = (&log_error, buffer.flush(Instant::now())) {
        log_error = turn.append(event).err();
    }
    if let Some(e) = log_error {
        return Err(e);
    }
    Ok(answer.map(|answer| {
        let usage = Usage {
            prompt_tokens: answer.prompt_tokens,
            cached_tokens: answer.cached_tokens,
            completion_tokens: answer.completion_tokens,
        };
        let total = sent.elapsed();
        let stats =
            MessageStats::measure(model.name(), &usage, first_token.unwrap_or(total), total);
        (answer, stats)
    }))
}

fn end_partial(turn: &mut Turn, message_id: &str) -> Result<(), String> {
    turn.append(Event::MessageDone {
        message_id: message_id.into(),
        stats: None,
        partial: true,
    })
}

fn assistant_start(message_id: &str) -> Event {
    Event::MessageStart {
        message_id: message_id.into(),
        role: Role::Assistant,
        content: None,
        is_system_generated: false,
        metadata: None,
        attachments: Vec::new(),
    }
}
