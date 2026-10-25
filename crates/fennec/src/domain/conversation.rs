//! Port of openfox `events/fold-messages.ts`: the event log read back as the
//! OpenAI chat messages sent to the model.

use super::event::{Event, ToolResult};
use serde_json::{Value, json};

/// Port of openfox `renderToolResultContent`: the single LLM-facing
/// rendering of a tool result, shared by the live path and the fold.
pub fn render_tool_result(result: &ToolResult) -> String {
    let error = result.error.as_deref().unwrap_or("");
    match (&result.output, result.success) {
        (Some(out), true) => out.clone(),
        (None, true) => "Success".into(),
        (Some(out), false) => format!("{out}\n\nError: {error}"),
        (None, false) => format!("Error: {error}"),
    }
}

/// Folds the log into OpenAI chat messages (without the system prompt).
pub fn fold_messages(events: &[Event]) -> Vec<Value> {
    let mut messages: Vec<Value> = Vec::new();
    // Index in `messages` of each assistant/user message, by message id.
    let mut index: Vec<(String, usize)> = Vec::new();
    let find = |index: &[(String, usize)], id: &str| {
        index.iter().rev().find(|(m, _)| m == id).map(|(_, i)| *i)
    };
    for event in events {
        match event {
            Event::MessageStart {
                message_id,
                role,
                content,
                ..
            } => {
                index.push((message_id.clone(), messages.len()));
                messages
                    .push(json!({"role": role, "content": content.clone().unwrap_or_default()}));
            }
            Event::MessageDelta {
                message_id,
                content,
            } => {
                if let Some(i) = find(&index, message_id) {
                    let text = messages[i]["content"].as_str().unwrap_or("").to_string() + content;
                    messages[i]["content"] = json!(text);
                }
            }
            // Sent back as the server streamed it: the prompt then extends the
            // exact tokens of the previous call, which the prompt cache holds.
            Event::MessageThinking {
                message_id,
                content,
            } => {
                if let Some(i) = find(&index, message_id) {
                    let text = messages[i]["reasoning_content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                        + content;
                    messages[i]["reasoning_content"] = json!(text);
                }
            }
            Event::ToolCall {
                message_id,
                tool_call,
            } => {
                if let Some(i) = find(&index, message_id) {
                    let call = json!({
                        "id": tool_call.id,
                        "type": "function",
                        "function": {"name": tool_call.name, "arguments": tool_call.arguments.to_string()},
                    });
                    match messages[i]["tool_calls"].as_array_mut() {
                        Some(calls) => calls.push(call),
                        None => messages[i]["tool_calls"] = json!([call]),
                    }
                }
            }
            Event::ToolResult {
                tool_call_id,
                result,
                ..
            } => messages.push(json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": render_tool_result(result),
            })),
            _ => {}
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{Role, ToolCall, ToolResult};

    #[test]
    fn fold_rebuilds_assistant_tool_calls_and_tool_results() {
        let start = |id: &str, role: Role, content: Option<&str>| Event::MessageStart {
            message_id: id.into(),
            role,
            content: content.map(Into::into),
            is_system_generated: false,
        };
        let events = vec![
            start("u", Role::User, Some("do it")),
            start("a", Role::Assistant, None),
            Event::MessageThinking {
                message_id: "a".into(),
                content: "hmm".into(),
            },
            Event::MessageDelta {
                message_id: "a".into(),
                content: "ok".into(),
            },
            Event::ToolCall {
                message_id: "a".into(),
                tool_call: ToolCall {
                    id: "c1".into(),
                    name: "read_file".into(),
                    arguments: json!({"path": "x"}),
                    parse_error: None,
                },
            },
            Event::MessageDone {
                message_id: "a".into(),
                stats: None,
                partial: false,
            },
            Event::ToolResult {
                message_id: "a".into(),
                tool_call_id: "c1".into(),
                result: ToolResult {
                    success: false,
                    output: None,
                    error: Some("nope".into()),
                    duration_ms: 1,
                    truncated: false,
                },
            },
        ];
        let msgs = fold_messages(&events);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["content"], "ok");
        assert_eq!(msgs[1]["reasoning_content"], "hmm");
        assert_eq!(
            msgs[1]["tool_calls"][0]["function"]["arguments"],
            r#"{"path":"x"}"#
        );
        assert_eq!(
            msgs[2],
            json!({"role": "tool", "tool_call_id": "c1", "content": "Error: nope"})
        );
    }
}
