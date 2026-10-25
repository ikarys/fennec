//! Tool calls that were recorded without a result. A turn that ended at
//! `ask_user`, was stopped mid-batch, or died with the process leaves them
//! open; the model API rejects an assistant tool call without its tool
//! message, so the next turn closes them first.

use super::event::Event;
use std::collections::HashSet;

pub struct OpenCall {
    pub message_id: String,
    pub call_id: String,
    pub awaits_answer: bool,
}

/// The `tool.call` events with no matching `tool.result`, in log order.
/// `awaits_answer` is true when a `chat.ask_user` event with the same
/// `call_id` exists: that call's result is the human's answer.
pub fn open_calls(events: &[Event]) -> Vec<OpenCall> {
    let resolved: HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::ToolResult { tool_call_id, .. } => Some(tool_call_id.as_str()),
            _ => None,
        })
        .collect();
    let asked: HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::ChatAskUser { call_id, .. } => Some(call_id.as_str()),
            _ => None,
        })
        .collect();
    events
        .iter()
        .filter_map(|e| match e {
            Event::ToolCall {
                message_id,
                tool_call,
            } if !resolved.contains(tool_call.id.as_str()) => Some(OpenCall {
                message_id: message_id.clone(),
                call_id: tool_call.id.clone(),
                awaits_answer: asked.contains(tool_call.id.as_str()),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{AskType, ToolCall};
    use serde_json::json;

    fn call(id: &str, name: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: name.into(),
                arguments: json!({}),
                parse_error: None,
            },
        }
    }

    fn result(id: &str) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: id.into(),
            result: crate::domain::event::ToolResult::success("ok".into(), false),
        }
    }

    #[test]
    fn no_open_call_when_every_call_has_a_result() {
        let events = vec![call("c1", "read_file"), result("c1")];
        assert!(open_calls(&events).is_empty());
    }

    #[test]
    fn one_open_call_without_a_result() {
        let events = vec![call("c1", "read_file")];
        let open = open_calls(&events);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].call_id, "c1");
        assert_eq!(open[0].message_id, "m");
        assert!(!open[0].awaits_answer);
    }

    #[test]
    fn an_ask_user_open_call_awaits_an_answer() {
        let events = vec![
            call("c1", "ask_user"),
            Event::ChatAskUser {
                call_id: "c1".into(),
                question: "Which color?".into(),
                ask_type: AskType::Text,
                options: None,
            },
        ];
        let open = open_calls(&events);
        assert_eq!(open.len(), 1);
        assert!(open[0].awaits_answer);
    }

    #[test]
    fn a_call_with_a_result_is_not_open() {
        let events = vec![
            call("c1", "read_file"),
            result("c1"),
            call("c2", "write_file"),
        ];
        let open = open_calls(&events);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].call_id, "c2");
    }

    #[test]
    fn open_calls_keep_log_order() {
        let events = vec![
            call("c1", "a"),
            call("c2", "b"),
            result("c1"),
            call("c3", "c"),
        ];
        let open = open_calls(&events);
        assert_eq!(
            open.iter().map(|c| c.call_id.as_str()).collect::<Vec<_>>(),
            ["c2", "c3"]
        );
    }

    #[test]
    fn a_resolved_ask_user_is_not_open() {
        let events = vec![
            call("c1", "ask_user"),
            Event::ChatAskUser {
                call_id: "c1".into(),
                question: "q".into(),
                ask_type: AskType::Text,
                options: None,
            },
            result("c1"),
        ];
        assert!(open_calls(&events).is_empty());
    }
}
