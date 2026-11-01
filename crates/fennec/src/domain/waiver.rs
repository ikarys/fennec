//! The human's pending answer to a `waiver.requested`, the session's active
//! grants, and the ADR a grant writes.

use super::compaction::model_view;
use super::event::Event;
use super::open_calls::open_calls;
use std::collections::{HashMap, HashSet};

/// A waiver is a human decision, so it is short: at most a week.
pub const MAX_HOURS: u32 = 168;

/// The open call a `waiver.requested` is waiting on.
pub struct Request {
    pub call_id: String,
    pub path: String,
    pub reason: String,
}

/// The last `waiver.requested` whose call is still open and has no
/// `waiver.answered` in the model's view.
pub fn pending(events: &[Event]) -> Option<Request> {
    let view = model_view(events);
    let open: HashSet<String> = open_calls(events)
        .into_iter()
        .map(|call| call.call_id)
        .collect();
    let answered: HashSet<&str> = view
        .iter()
        .filter_map(|event| match event {
            Event::WaiverAnswered { call_id, .. } => Some(call_id.as_str()),
            _ => None,
        })
        .collect();

    view.iter().rev().find_map(|event| match event {
        Event::WaiverRequested {
            call_id,
            path,
            reason,
        } if open.contains(call_id) && !answered.contains(call_id.as_str()) => Some(Request {
            call_id: call_id.clone(),
            path: path.clone(),
            reason: reason.clone(),
        }),
        _ => None,
    })
}

/// The human's answer, recorded when they click.
pub struct Answer {
    pub granted: bool,
    pub expires_at: Option<u64>,
    pub adr: Option<String>,
}

/// The waiver `call_id` the human answered whose call has no result yet: the
/// answer is recorded at the click, before the turn that produces the result,
/// so that turn's protection already exempts a granted path.
pub fn answered(events: &[Event], call_id: &str) -> Option<(String, Answer)> {
    if !open_calls(events)
        .iter()
        .any(|call| call.call_id == call_id)
    {
        return None;
    }
    let path = events.iter().find_map(|event| match event {
        Event::WaiverRequested {
            call_id: id, path, ..
        } if id == call_id => Some(path.clone()),
        _ => None,
    })?;
    let answer = events.iter().rev().find_map(|event| match event {
        Event::WaiverAnswered {
            call_id: id,
            granted,
            expires_at,
            adr,
        } if id == call_id => Some(Answer {
            granted: *granted,
            expires_at: *expires_at,
            adr: adr.clone(),
        }),
        _ => None,
    })?;
    Some((path, answer))
}

/// The paths a granted, unexpired `waiver.answered` exempts for the session.
/// All events are used, not the model view: a compaction must not drop a grant.
pub fn active(events: &[Event], now_ms: u64) -> Vec<String> {
    let requested: HashMap<String, String> = events
        .iter()
        .filter_map(|event| match event {
            Event::WaiverRequested { call_id, path, .. } => Some((call_id.clone(), path.clone())),
            _ => None,
        })
        .collect();

    let mut paths = Vec::new();
    for event in events {
        if let Event::WaiverAnswered {
            call_id,
            granted: true,
            expires_at: Some(expires_at),
            ..
        } = event
            && *expires_at > now_ms
            && let Some(path) = requested.get(call_id)
            && !paths.contains(path)
        {
            paths.push(path.clone());
        }
    }
    paths
}

/// `NNNN-waiver-<slug>.md`, the ADR file name for a grant.
pub fn adr_file_name(number: u32, path: &str) -> String {
    format!("{number:04}-waiver-{}.md", slug(path))
}

/// The MADR a grant writes into the session's workdir.
pub fn adr(request: &Request, session_id: &str, date: &str, expires: &str) -> String {
    format!(
        "---\nstatus: accepted\ndate: {date}\n---\n# Waiver: write {path}\n\n## Context and Problem Statement\n\nRequested by the agent in session {session_id}:\n\n{reason}\n\n## Decision Outcome\n\nThe human waived the protection of `{path}` for this session only, until {expires} UTC. Granted through fennec's UI; an agent cannot grant itself a waiver.\n",
        path = request.path,
        reason = request.reason,
    )
}

/// The ADR slug: lowercased, every non-alphanumeric run as `-`, trimmed of
/// `-`, at most 40 chars.
fn slug(path: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in path.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            pending_dash = false;
        } else if !pending_dash {
            out.push('-');
            pending_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let truncated: String = trimmed.chars().take(40).collect();
    truncated.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{ToolCall, ToolResult};
    use serde_json::json;

    fn tool_call(id: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: "request_waiver".into(),
                arguments: json!({}),
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    fn request(id: &str) -> Event {
        Event::WaiverRequested {
            call_id: id.into(),
            path: "AGENTS.md".into(),
            reason: "why".into(),
        }
    }

    fn answered(id: &str, granted: bool, expires_at: Option<u64>) -> Event {
        Event::WaiverAnswered {
            call_id: id.into(),
            granted,
            expires_at,
            adr: None,
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
        let events = vec![tool_call("c1"), request("c1")];
        let pending = pending(&events).expect("a pending waiver");
        assert_eq!(pending.call_id, "c1");
        assert_eq!(pending.path, "AGENTS.md");
        assert_eq!(pending.reason, "why");
    }

    #[test]
    fn none_after_an_answer() {
        let events = vec![
            tool_call("c1"),
            request("c1"),
            answered("c1", true, Some(1)),
        ];
        assert!(pending(&events).is_none());
    }

    #[test]
    fn none_after_the_call_got_a_result() {
        let events = vec![tool_call("c1"), request("c1"), result("c1")];
        assert!(pending(&events).is_none());
    }

    #[test]
    fn active_granted_and_not_expired() {
        let events = vec![request("c1"), answered("c1", true, Some(100))];
        assert_eq!(active(&events, 50), vec!["AGENTS.md"]);
    }

    #[test]
    fn active_expires() {
        let events = vec![request("c1"), answered("c1", true, Some(100))];
        assert!(active(&events, 100).is_empty());
    }

    #[test]
    fn active_refused_is_not_active() {
        let events = vec![request("c1"), answered("c1", false, Some(100))];
        assert!(active(&events, 50).is_empty());
    }

    #[test]
    fn active_without_a_request_is_empty() {
        let events = vec![answered("c1", true, Some(100))];
        assert!(active(&events, 50).is_empty());
    }

    #[test]
    fn adr_round_trips_through_parse() {
        let request = Request {
            call_id: "c1".into(),
            path: "AGENTS.md".into(),
            reason: "why".into(),
        };
        let content = adr(&request, "s", "2026-01-01", "2026-01-02 00:00");
        let parsed = crate::domain::adr::parse("0001-waiver-agents-md.md", &content)
            .expect("the waiver ADR parses");
        assert_eq!(parsed.number, 1);
        assert_eq!(parsed.file, "0001-waiver-agents-md.md");
        assert_eq!(parsed.title, "Waiver: write AGENTS.md");
        assert_eq!(parsed.status.as_deref(), Some("accepted"));
        assert!(parsed.body.contains("protection of `AGENTS.md`"));
        assert!(parsed.body.contains("why"));
    }

    #[test]
    fn slug_lowercases_and_dashes_non_alphanumeric_runs() {
        assert_eq!(slug("AGENTS.md"), "agents-md");
        assert_eq!(slug("./src/a b/c"), "src-a-b-c");
        assert_eq!(slug("a---b"), "a-b");
        assert_eq!(slug(&"x".repeat(50)), "x".repeat(40));
    }
}
