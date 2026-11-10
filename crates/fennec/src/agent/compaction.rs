//! Compaction: when the context nears the window, ask the model for a
//! summary and open a new window from it.
//!
//! The message that opens the new window is built from checked facts
//! (files changed, the last request) plus the model's summary, and nothing
//! is deleted — a `context.compacted` event marks the cut.

use super::round::{self, Round, RoundPrompt};
use super::turn::Turn;
use crate::domain::compaction;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;

/// What the model is asked to summarise before the window is cut.
pub const COMPACTION_PROMPT: &str =
    "The context window is nearly full. fennec is about to open a new one for you from a summary you write now, to which fennec itself adds the list of files changed so far and the human's last request verbatim: do not repeat those.

Write what the log cannot give back, so the next window picks up without redoing the work:
1. The goal of the session and the plan in force, with its \"Done when\" criteria if any
2. What you learned from the code: the facts that matter, each with its path (and line when it counts)
3. The decisions taken and why, and the alternatives rejected
4. What failed, how it was fixed, and what is still broken
5. What is done, what is in progress, what remains, in order
6. The human's requirements and preferences that must persist

Do not answer any question from the conversation and call no tool: output the summary only, structured with those headings, thorough on facts and short on prose.";

/// Nudge when the model only tried tools instead of summarising.
const NO_TOOLS_PROMPT: &str = "Tool calls are not possible at this stage. STOP and produce a summary for compaction purposes NOW:";

/// How a compaction attempt ended.
#[derive(Debug, PartialEq)]
pub(super) enum Compacted {
    /// The summary was recorded and the new window opened.
    Done,
    /// No summary could be produced: the session keeps its full history.
    Skipped,
    /// The turn must end (stopped by the human, or the model unreachable).
    Ended(DoneReason),
}

/// One summary round: stream the model, record its `MessageDone`, and hand
/// back its content. The answer's tool calls are neither recorded nor run.
enum SummaryRound {
    Answered(String),
    Interrupted,
    Ended(DoneReason),
}

fn stream_summary(turn: &mut Turn, prompt: &RoundPrompt) -> Result<SummaryRound, String> {
    let message_id = new_id();
    match round::stream(turn, &message_id, prompt)? {
        Round::Answered(answer, stats) => {
            turn.append(Event::MessageDone {
                message_id,
                stats: Some(stats),
                partial: false,
            })?;
            Ok(SummaryRound::Answered(answer.content))
        }
        Round::Interrupted => Ok(SummaryRound::Interrupted),
        Round::Ended(reason) => Ok(SummaryRound::Ended(reason)),
    }
}

/// Runs one compaction: append the prompt, stream the summary (retrying a
/// broken stream, at most 3 tries; nudging once if the model only tried
/// tools), then open the new window from it.
pub(super) fn compact(turn: &mut Turn, prompt: &RoundPrompt) -> Result<Compacted, String> {
    let events = turn.log.events(turn.session_id)?;
    let before = compaction::context_tokens(&events);
    turn.append_system(COMPACTION_PROMPT)?;

    let mut summary = None;
    for _ in 0..3 {
        match stream_summary(turn, prompt)? {
            SummaryRound::Answered(content) => {
                summary = Some(content);
                break;
            }
            SummaryRound::Interrupted => continue,
            SummaryRound::Ended(reason) => return Ok(Compacted::Ended(reason)),
        }
    }
    let Some(summary) = summary else {
        return Ok(Compacted::Skipped);
    };

    // The model only tried tools: nudge it to summarise, once.
    let summary = if summary.trim().is_empty() {
        turn.append_system(NO_TOOLS_PROMPT)?;
        match stream_summary(turn, prompt)? {
            SummaryRound::Answered(content) => content,
            SummaryRound::Interrupted => return Ok(Compacted::Skipped),
            SummaryRound::Ended(reason) => return Ok(Compacted::Ended(reason)),
        }
    } else {
        summary
    };

    if summary.trim().is_empty() {
        // No summary: the session simply keeps its full history.
        return Ok(Compacted::Skipped);
    }

    let events = turn.log.events(turn.session_id)?;
    let files = compaction::files_changed(&events);
    let last_request = compaction::last_request(&events);
    let resume = compaction::resume_message(&summary, &files, last_request.as_deref());

    // A char-per-token estimate of what the next request sends (system
    // prompt, tool definitions, resume); the next round reports the real count.
    let after =
        ((prompt.system_prompt.len() + prompt.tools.to_string().len() + resume.len()) / 4) as u64;
    turn.append(Event::ContextCompacted {
        before_tokens: before,
        after_tokens: after,
        summary: summary.clone(),
    })?;
    turn.append_system(&resume)?;
    turn.append(Event::ContextState {
        current_tokens: after,
        max_tokens: turn.context_window,
        compact_at: Some(turn.compact_at),
    })?;
    Ok(Compacted::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::{Completion, EventLog};
    use crate::agent::test_support::{MemoryLog, Scripted, builder, reply};
    use crate::domain::conversation::fold_messages;
    use crate::domain::event::{Role, ToolCall, ToolResult};
    use crate::tools::LocalTools;
    use serde_json::json;
    use std::cell::RefCell;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

    fn run_compact(log: &MemoryLog, replies: Vec<Result<Completion, String>>) -> Compacted {
        let model = Scripted(RefCell::new(replies));
        let tools = LocalTools::new(Path::new("/tmp").to_path_buf());
        let stop = AtomicBool::new(false);
        let mut on_event = |_: &Event| {};
        let agents = builder();
        let mut turn = Turn {
            model: &model,
            log,
            tools: &tools,
            session_id: "s",
            workdir: Path::new("/tmp"),
            task_sheet: None,
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &agents,
            max_tokens: 100,
            context_window: 10_000,
            compact_at: 0.85,
            reasoning_effort: None,
            session_effort: None,
            autonomous: false,
            lang: crate::domain::lang::Lang::Fr,
            preflight_sent_back: false,

            stop: &stop,
            on_event: &mut on_event,
            children: None,
        };
        let tools = json!([]);
        let prompt = RoundPrompt {
            system_prompt: "system",
            tools: &tools,
            max_tokens: 100,
            reasoning_effort: None,
        };
        compact(&mut turn, &prompt).unwrap()
    }

    fn append(log: &MemoryLog, event: Event) {
        log.append("s", &event).unwrap();
    }

    fn full_context(log: &MemoryLog) {
        append(
            log,
            Event::ContextState {
                current_tokens: 9_000,
                max_tokens: 10_000,
                compact_at: None,
            },
        );
    }

    #[test]
    fn compaction_records_prompt_summary_cut_and_resume() {
        let log = MemoryLog::default();
        append(
            &log,
            Event::MessageStart {
                message_id: "u".into(),
                role: Role::User,
                content: Some("do the thing".into()),
                is_system_generated: false,
                metadata: None,
                attachments: Vec::new(),
            },
        );
        append(
            &log,
            Event::ToolCall {
                message_id: "a".into(),
                tool_call: ToolCall {
                    id: "c1".into(),
                    name: "write_file".into(),
                    arguments: json!({"path": "a.txt"}),
                    parse_error: None,
                    raw_arguments: None,
                },
            },
        );
        append(
            &log,
            Event::ToolResult {
                message_id: "a".into(),
                tool_call_id: "c1".into(),
                result: ToolResult::success("ok".into(), false),
            },
        );
        full_context(&log);

        let outcome = run_compact(&log, vec![reply("the summary", vec![])]);
        assert_eq!(outcome, Compacted::Done);

        let events = log.0.into_inner();
        assert!(events.iter().any(|e| matches!(e,
            Event::MessageStart { content: Some(c), is_system_generated: true, .. }
                if c == COMPACTION_PROMPT)));
        assert!(events.iter().any(|e| matches!(e,
            Event::ContextCompacted { before_tokens: 9_000, summary, .. }
                if summary == "the summary")));
        let expected = "This session was compacted to free the context window. \
            Summary of the earlier conversation, written by the model:\n\n\
            the summary\n\n\
            ## Files changed so far (from the session log)\n\
            - a.txt\n\n\
            ## The user's last request, verbatim\n\
            do the thing\n\n\
            Continue the work from here.";
        // The model's view of the log starts with the resume message.
        let messages = fold_messages(&events);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], expected);
    }

    #[test]
    fn an_empty_answer_twice_skips_compaction() {
        let log = MemoryLog::default();
        full_context(&log);
        let tried_a_tool = ToolCall {
            id: "c1".into(),
            name: "read_file".into(),
            arguments: json!({}),
            parse_error: None,
            raw_arguments: None,
        };
        let outcome = run_compact(&log, vec![reply("", vec![tried_a_tool]), reply("", vec![])]);
        assert_eq!(outcome, Compacted::Skipped);
        let events = log.0.into_inner();
        assert!(events.iter().any(|e| matches!(e,
            Event::MessageStart { content: Some(c), .. } if c == NO_TOOLS_PROMPT)));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::ContextCompacted { .. }))
        );
        assert!(!events.iter().any(|e| matches!(e, Event::ToolCall { .. })));
    }
}
