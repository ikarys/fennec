//! Pure helpers for compaction: what the model's view of the log is after
//! the last cut, how much context it holds, and the facts that open the new
//! window without going through the model (files changed, last request).

use super::event::{Event, Role};
use std::collections::HashSet;

/// `COMPACTION_HEADROOM_TOKENS`: compaction always fires when fewer
/// than this many tokens remain, regardless of the configured threshold.
const COMPACTION_HEADROOM_TOKENS: u64 = 5_000;
/// `COMPACTION_MAX_RATIO`: the effective threshold is capped here.
const COMPACTION_MAX_RATIO: f64 = 0.95;

/// The slice after the last `ContextCompacted` (the whole slice if none):
/// the model's view of the log starts after the last cut.
pub fn since_last_compaction(events: &[Event]) -> &[Event] {
    match events
        .iter()
        .rposition(|e| matches!(e, Event::ContextCompacted { .. }))
    {
        Some(i) => &events[i + 1..],
        None => events,
    }
}

/// The events the model sees: those after the last compaction, minus what a
/// later `session.rewound` cut off (its message and everything after it, up
/// to the rewind itself). The rewind events are not part of the view.
pub fn model_view(events: &[Event]) -> Vec<&Event> {
    let mut view: Vec<&Event> = Vec::new();
    for event in since_last_compaction(events) {
        match event {
            Event::SessionRewound { message_id } => {
                if let Some(i) = view.iter().rposition(|e| {
                    matches!(
                        e,
                        Event::MessageStart {
                            message_id: id,
                            ..
                        } if id.as_str() == message_id.as_str()
                    )
                }) {
                    view.truncate(i);
                }
            }
            _ => view.push(event),
        }
    }
    view
}

/// `current_tokens` of the last `ContextState` in the model's view, 0 if
/// none has been measured yet.
pub fn context_tokens(events: &[Event]) -> u64 {
    model_view(events)
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::ContextState { current_tokens, .. } => Some(*current_tokens),
            _ => None,
        })
        .unwrap_or(0)
}

/// The `path` of every successful `write_file`/`edit_file` call, over the
/// whole log (earlier compactions included), deduplicated in first-seen
/// order. These are checked facts, so they do not go through the model.
pub fn files_changed(events: &[Event]) -> Vec<String> {
    let succeeded: HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::ToolResult {
                tool_call_id,
                result,
                ..
            } if result.success => Some(tool_call_id.as_str()),
            _ => None,
        })
        .collect();
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for e in events {
        if let Event::ToolCall { tool_call, .. } = e
            && (tool_call.name == "write_file" || tool_call.name == "edit_file")
            && succeeded.contains(tool_call.id.as_str())
            && let Some(path) = tool_call.arguments.get("path").and_then(|v| v.as_str())
            && seen.insert(path.to_string())
        {
            files.push(path.to_string());
        }
    }
    files
}

/// The content of the last human message (not system-generated), over the
/// whole log: copied verbatim into the new window.
pub fn last_request(events: &[Event]) -> Option<String> {
    events.iter().rev().find_map(|e| match e {
        Event::MessageStart {
            role: Role::User,
            is_system_generated: false,
            content: Some(content),
            ..
        } => Some(content.clone()),
        _ => None,
    })
}

/// The message that opens the new window, built from the model's summary and
/// the checked facts above.
pub fn resume_message(summary: &str, files: &[String], last_request: Option<&str>) -> String {
    let mut msg = String::from(
        "This session was compacted to free the context window. Summary of the earlier conversation, written by the model:\n\n",
    );
    msg.push_str(summary);
    msg.push_str("\n\n## Files changed so far (from the session log)\n");
    if files.is_empty() {
        msg.push_str("(none)\n");
    } else {
        for file in files {
            msg.push_str("- ");
            msg.push_str(file);
            msg.push('\n');
        }
    }
    if let Some(request) = last_request {
        msg.push_str("\n## The user's last request, verbatim\n");
        msg.push_str(request);
        msg.push('\n');
    }
    msg.push_str("\nContinue the work from here.");
    msg
}

/// Whether the window must be compacted: false when the threshold is disabled;
/// the effective threshold is the minimum of the configured one, the ratio
/// that leaves `COMPACTION_HEADROOM_TOKENS` of headroom, and
/// `COMPACTION_MAX_RATIO`; compacts when the context passes it.
pub fn should_compact(current_tokens: u64, context_window: u64, threshold: f64) -> bool {
    if threshold <= 0.0 || context_window == 0 {
        return false;
    }
    let ceiling_ratio = (context_window.saturating_sub(COMPACTION_HEADROOM_TOKENS) as f64
        / context_window as f64)
        .clamp(0.0, 1.0);
    let effective = threshold.min(ceiling_ratio).min(COMPACTION_MAX_RATIO);
    current_tokens as f64 > context_window as f64 * effective
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::ToolCall;
    use serde_json::json;

    fn user(content: &str, system_generated: bool) -> Event {
        Event::MessageStart {
            message_id: "m".into(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: system_generated,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    fn write_call(id: &str, path: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: "write_file".into(),
                arguments: json!({"path": path}),
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    fn result(id: &str, success: bool) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: id.into(),
            result: if success {
                crate::domain::event::ToolResult::success("ok".into(), false)
            } else {
                crate::domain::event::ToolResult::failure("nope".into())
            },
        }
    }

    #[test]
    fn since_last_compaction_returns_the_whole_slice_without_a_cut() {
        let events = vec![user("a", false), user("b", false)];
        assert_eq!(since_last_compaction(&events), events.as_slice());
    }

    #[test]
    fn since_last_compaction_returns_the_slice_after_the_last_cut() {
        let events = vec![
            user("a", false),
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "s1".into(),
            },
            user("b", false),
            Event::ContextCompacted {
                before_tokens: 3,
                after_tokens: 4,
                summary: "s2".into(),
            },
            user("c", false),
        ];
        let after = since_last_compaction(&events);
        assert_eq!(after.len(), 1);
        assert!(matches!(&after[0], Event::MessageStart { content: Some(c), .. } if c == "c"));
    }

    fn user_id(id: &str, content: &str) -> Event {
        Event::MessageStart {
            message_id: id.into(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn model_view_is_the_same_as_since_last_compaction_without_a_rewind() {
        let events = vec![user_id("u1", "a"), user_id("u2", "b")];
        assert_eq!(model_view(&events), events.iter().collect::<Vec<_>>());
    }

    #[test]
    fn model_view_drops_the_rewound_message_and_what_follows() {
        let events = vec![
            user_id("u1", "first"),
            user_id("u2", "second"),
            Event::SessionRewound {
                message_id: "u1".into(),
            },
            user_id("u3", "after"),
        ];
        assert_eq!(model_view(&events), vec![&events[3]]);
    }

    #[test]
    fn model_view_ignores_a_rewind_target_before_the_last_compaction() {
        let events = vec![
            user_id("u0", "old"),
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "s".into(),
            },
            user_id("u1", "new"),
            Event::SessionRewound {
                message_id: "u0".into(),
            },
            user_id("u2", "after"),
        ];
        assert_eq!(model_view(&events), vec![&events[2], &events[4]]);
    }

    #[test]
    fn context_tokens_reads_the_last_state_in_the_view() {
        let events = vec![
            Event::ContextState {
                current_tokens: 100,
                max_tokens: 1000,
                compact_at: None,
            },
            Event::ContextCompacted {
                before_tokens: 100,
                after_tokens: 50,
                summary: "s".into(),
            },
            Event::ContextState {
                current_tokens: 200,
                max_tokens: 1000,
                compact_at: None,
            },
            Event::ContextState {
                current_tokens: 300,
                max_tokens: 1000,
                compact_at: None,
            },
        ];
        assert_eq!(context_tokens(&events), 300);
    }

    #[test]
    fn context_tokens_is_zero_without_a_state() {
        assert_eq!(context_tokens(&[user("a", false)]), 0);
    }

    #[test]
    fn files_changed_lists_successful_writes_in_first_seen_order() {
        let events = vec![
            write_call("c1", "a.txt"),
            result("c1", true),
            write_call("c2", "b.txt"),
            result("c2", true),
            write_call("c3", "a.txt"),
            result("c3", true),
            write_call("c4", "c.txt"),
            result("c4", false),
        ];
        assert_eq!(files_changed(&events), vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn files_changed_is_empty_without_successful_writes() {
        assert_eq!(
            files_changed(&[write_call("c1", "a.txt")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn last_request_is_the_last_human_message() {
        let events = vec![
            user("first", false),
            user("system note", true),
            user("second", false),
        ];
        assert_eq!(last_request(&events), Some("second".into()));
    }

    #[test]
    fn last_request_skips_system_generated_messages() {
        let events = vec![user("first", false), user("system note", true)];
        assert_eq!(last_request(&events), Some("first".into()));
    }

    #[test]
    fn last_request_is_none_without_a_human_message() {
        assert_eq!(last_request(&[user("system note", true)]), None);
    }

    #[test]
    fn resume_message_includes_files_and_request() {
        let msg = resume_message(
            "the summary",
            &["a.txt".into(), "b.txt".into()],
            Some("do it"),
        );
        assert_eq!(
            msg,
            "This session was compacted to free the context window. Summary of the earlier conversation, written by the model:\n\n\
             the summary\n\n\
             ## Files changed so far (from the session log)\n\
             - a.txt\n\
             - b.txt\n\n\
             ## The user's last request, verbatim\n\
             do it\n\n\
             Continue the work from here."
        );
    }

    #[test]
    fn resume_message_omits_the_request_section_and_marks_no_files() {
        let msg = resume_message("the summary", &[], None);
        assert_eq!(
            msg,
            "This session was compacted to free the context window. Summary of the earlier conversation, written by the model:\n\n\
             the summary\n\n\
             ## Files changed so far (from the session log)\n\
             (none)\n\n\
             Continue the work from here."
        );
    }

    #[test]
    fn should_compact_is_false_when_disabled() {
        assert!(!should_compact(9000, 10000, 0.0));
        assert!(!should_compact(9000, 10000, -0.5));
    }

    #[test]
    fn should_compact_fires_past_the_threshold() {
        assert!(should_compact(180_000, 200_000, 0.85));
        assert!(!should_compact(160_000, 200_000, 0.85));
    }

    #[test]
    fn should_compact_is_capped_by_the_max_ratio() {
        assert!(should_compact(191_000, 200_000, 0.99));
        assert!(!should_compact(189_000, 200_000, 0.99));
    }

    #[test]
    fn should_compact_keeps_the_headroom_on_a_small_window() {
        // 10k window - 5k headroom: the ceiling is 0.5, below the threshold.
        assert!(should_compact(5_100, 10_000, 0.85));
    }
}
