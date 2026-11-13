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
    pub subject: String,
    pub kind: rules::TargetKind,
    pub pattern: String,
    pub access: Option<String>,
    pub folders: Vec<String>,
}

/// The pattern an answer stores: the folder the human picked, else for "never" on a path the exact path asked (a refusal must not spread to its folder), else the proposal.
pub fn stored_pattern(pending: &Pending, choice: Choice, chosen: Option<String>) -> String {
    if let Some(chosen) = chosen {
        return chosen;
    }
    if choice == Choice::Never && pending.kind == rules::TargetKind::Path {
        return pending.subject.clone();
    }
    pending.pattern.clone()
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
            subject,
            kind,
            pattern,
            access,
            folders,
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
            // A kind outside the three the kernel writes leaves nothing to answer.
            .zip(rules::TargetKind::parse(kind))
            .map(|((message_id, call), kind)| Pending {
                call_id: call_id.clone(),
                message_id,
                call,
                subject: subject.clone(),
                kind,
                pattern: pattern.clone(),
                access: access.clone(),
                folders: folders.clone(),
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
            folders: Vec::new(),
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
        assert_eq!(pending.kind, rules::TargetKind::Tool);
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

    fn make_pending(subject: &str, pattern: &str) -> Pending {
        Pending {
            call_id: "c1".into(),
            message_id: "m".into(),
            call: ToolCall {
                id: "c1".into(),
                name: "write_file".into(),
                arguments: json!({}),
                parse_error: None,
                raw_arguments: None,
            },
            subject: subject.into(),
            kind: rules::TargetKind::Path,
            pattern: pattern.into(),
            access: Some("write".into()),
            folders: vec![pattern.into()],
        }
    }

    #[test]
    fn stored_pattern_uses_the_chosen_folder() {
        let pending = make_pending("/tmp/mini.py", "/tmp/");
        assert_eq!(
            stored_pattern(&pending, Choice::Never, Some("/tmp/".into())),
            "/tmp/"
        );
    }

    #[test]
    fn stored_pattern_never_blocks_the_exact_path() {
        let pending = make_pending("/tmp/mini.py", "/tmp/");
        assert_eq!(
            stored_pattern(&pending, Choice::Never, None),
            "/tmp/mini.py"
        );
    }

    #[test]
    fn stored_pattern_uses_the_proposal_for_other_choices() {
        let pending = make_pending("/tmp/mini.py", "/tmp/");
        assert_eq!(stored_pattern(&pending, Choice::Always, None), "/tmp/");
    }
}
