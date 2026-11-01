//! The human's pending answer to a `rule.approval_requested`: which call is
//! waiting, and what the "session", "always" and "never" answers would store.

use super::compaction::model_view;
use super::event::{Event, ToolCall};
use super::open_calls::open_calls;
use std::collections::HashSet;

/// The human's answer to a `rule.approval_requested`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Once,
    Session,
    Always,
    Never,
}

impl Choice {
    pub fn parse(value: &str) -> Option<Choice> {
        Some(match value {
            "once" => Choice::Once,
            "session" => Choice::Session,
            "always" => Choice::Always,
            "never" => Choice::Never,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Choice::Once => "once",
            Choice::Session => "session",
            Choice::Always => "always",
            Choice::Never => "never",
        }
    }
}

/// The open call a `rule.approval_requested` is waiting on.
pub struct Pending {
    pub call_id: String,
    pub message_id: String,
    pub call: ToolCall,
    pub kind: String,
    pub pattern: String,
    pub access: Option<String>,
}

/// The last `rule.approval_requested` whose call is still open and has no
/// `rule.approval_answered` in the model's view.
pub fn pending(events: &[Event]) -> Option<Pending> {
    let view = model_view(events);
    let open: HashSet<String> = open_calls(events)
        .into_iter()
        .map(|call| call.call_id)
        .collect();
    let answered: HashSet<&str> = view
        .iter()
        .filter_map(|event| match event {
            Event::RuleApprovalAnswered { call_id, .. } => Some(call_id.as_str()),
            _ => None,
        })
        .collect();

    view.iter().rev().find_map(|event| match event {
        Event::RuleApprovalRequested {
            call_id,
            kind,
            pattern,
            access,
            ..
        } if open.contains(call_id) && !answered.contains(call_id.as_str()) => view
            .iter()
            .find_map(|event| match event {
                Event::ToolCall {
                    message_id,
                    tool_call,
                } if tool_call.id == *call_id => Some((message_id.clone(), tool_call.clone())),
                _ => None,
            })
            .map(|(message_id, call)| Pending {
                call_id: call_id.clone(),
                message_id,
                call,
                kind: kind.clone(),
                pattern: pattern.clone(),
                access: access.clone(),
            }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::ToolResult;
    use serde_json::json;

    fn tool_call(id: &str, name: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: name.into(),
                arguments: json!({}),
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    fn request(id: &str) -> Event {
        Event::RuleApprovalRequested {
            call_id: id.into(),
            subject: "subject".into(),
            rule_id: None,
            kind: "tool".into(),
            pattern: "pattern".into(),
            access: None,
        }
    }

    fn answered(id: &str) -> Event {
        Event::RuleApprovalAnswered {
            call_id: id.into(),
            choice: "once".into(),
            rule_id: None,
        }
    }

    fn result(id: &str) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: id.into(),
            result: ToolResult::success("ok".into(), false),
        }
    }

    #[test]
    fn pending_after_a_request() {
        let events = vec![tool_call("c1", "read_file"), request("c1")];
        let pending = pending(&events).expect("a pending approval");
        assert_eq!(pending.call_id, "c1");
        assert_eq!(pending.message_id, "m");
        assert_eq!(pending.call.name, "read_file");
        assert_eq!(pending.kind, "tool");
        assert_eq!(pending.pattern, "pattern");
        assert_eq!(pending.access, None);
    }

    #[test]
    fn none_after_an_answer() {
        let events = vec![tool_call("c1", "read_file"), request("c1"), answered("c1")];
        assert!(pending(&events).is_none());
    }

    #[test]
    fn none_after_the_call_got_a_result() {
        let events = vec![tool_call("c1", "read_file"), request("c1"), result("c1")];
        assert!(pending(&events).is_none());
    }
}
