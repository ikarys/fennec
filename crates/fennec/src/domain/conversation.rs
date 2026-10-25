//! Port of openfox `events/fold-messages.ts`: the event log read back as the
//! OpenAI chat messages sent to the model.

use super::compaction::model_view;
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

/// Folds the log into OpenAI chat messages (without the system prompt). Only
/// the model's view is folded: the events after the last `context.compacted`,
/// minus what a later `session.rewound` cut off.
pub fn fold_messages(events: &[Event]) -> Vec<Value> {
    let events = model_view(events);
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
                attachments,
                ..
            } => {
                index.push((message_id.clone(), messages.len()));
                let content = if attachments.is_empty() {
                    json!(content.clone().unwrap_or_default())
                } else {
                    let mut parts = Vec::new();
                    let text = content.clone().unwrap_or_default();
                    if !text.is_empty() {
                        parts.push(json!({"type": "text", "text": text}));
                    }
                    for attachment in attachments {
                        parts.push(json!({
                            "type": "image_url",
                            "image_url": {"url": attachment.data},
                        }));
                    }
                    json!(parts)
                };
                messages.push(json!({"role": role, "content": content}));
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
            } => {
                let content = if let Some(metadata) = &result.metadata
                    && metadata.mime_type.starts_with("image/")
                {
                    json!([
                        {"type": "text", "text": render_tool_result(result)},
                        {"type": "image_url", "image_url": {"url": metadata.data_url}},
                    ])
                } else {
                    json!(render_tool_result(result))
                };
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": content,
                }));
            }
            _ => {}
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::attachment::Attachment;
    use crate::domain::event::{Role, ToolCall, ToolResult};

    fn start(id: &str, role: Role, content: Option<&str>, system_generated: bool) -> Event {
        Event::MessageStart {
            message_id: id.into(),
            role,
            content: content.map(Into::into),
            is_system_generated: system_generated,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn fold_rebuilds_assistant_tool_calls_and_tool_results() {
        let events = vec![
            start("u", Role::User, Some("do it"), false),
            start("a", Role::Assistant, None, false),
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
                    metadata: None,
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

    #[test]
    fn a_message_with_attachments_folds_to_text_and_image_parts() {
        let events = vec![Event::MessageStart {
            message_id: "u".into(),
            role: Role::User,
            content: Some("look".into()),
            is_system_generated: false,
            metadata: None,
            attachments: vec![Attachment {
                id: "a".into(),
                filename: "a.png".into(),
                mime_type: "image/png".into(),
                size: 1,
                data: "data:image/png;base64,AAAA".into(),
            }],
        }];
        let msgs = fold_messages(&events);
        assert_eq!(
            msgs[0]["content"][0],
            json!({"type": "text", "text": "look"})
        );
        assert_eq!(
            msgs[0]["content"][1],
            json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}})
        );
    }

    #[test]
    fn a_message_without_attachments_stays_a_plain_string() {
        let events = vec![start("u", Role::User, Some("plain"), false)];
        let msgs = fold_messages(&events);
        assert_eq!(msgs[0]["content"], "plain");
    }

    #[test]
    fn an_image_tool_result_folds_to_text_and_image_parts() {
        let events = vec![Event::ToolResult {
            message_id: "a".into(),
            tool_call_id: "c1".into(),
            result: ToolResult::image(
                "img".into(),
                crate::domain::event::ToolResultMetadata {
                    mime_type: "image/png".into(),
                    data_url: "data:image/png;base64,AAAA".into(),
                    path: "a.png".into(),
                    size: 1,
                },
            ),
        }];
        let msgs = fold_messages(&events);
        assert_eq!(
            msgs[0]["content"][0],
            json!({"type": "text", "text": "img"})
        );
        assert_eq!(
            msgs[0]["content"][1],
            json!({"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}})
        );
    }

    #[test]
    fn fold_drops_the_rewound_message_and_what_follows() {
        let events = vec![
            start("u1", Role::User, Some("first"), false),
            start("a1", Role::Assistant, Some("first"), false),
            Event::SessionRewound {
                message_id: "u1".into(),
            },
            start("u2", Role::User, Some("again"), false),
            start("a2", Role::Assistant, Some("second"), false),
        ];
        let msgs = fold_messages(&events);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["content"], "again");
        assert_eq!(msgs[1]["content"], "second");
    }

    #[test]
    fn fold_starts_after_the_last_compaction_with_the_resume_message() {
        let events = vec![
            start("u0", Role::User, Some("old request"), false),
            start("a0", Role::Assistant, Some("old answer"), false),
            Event::ContextCompacted {
                before_tokens: 9000,
                after_tokens: 100,
                summary: "the summary".into(),
            },
            start("resume", Role::User, Some("the resume message"), true),
            start("a1", Role::Assistant, Some("new answer"), false),
        ];
        let msgs = fold_messages(&events);
        // The events before the cut are gone; the first message is the resume.
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["content"], "the resume message");
        assert_eq!(msgs[1]["content"], "new answer");
    }
}
