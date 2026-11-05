//! The event log read back as the chat messages sent to the model.

use super::compaction::model_view;
use super::event::{Event, Role, ToolResult};
use super::vision::first_kept_image;
use serde_json::{Value, json};

/// Replaces the images the window no longer holds (`vision::first_kept_image`)
/// by a text part.
fn cap_images(messages: &mut [Value]) {
    let is_image = |part: &Value| part["type"] == "image_url";
    let mut parts: Vec<&mut Value> = messages
        .iter_mut()
        .filter_map(|message| message["content"].as_array_mut())
        .flatten()
        .filter(|part| is_image(part))
        .collect();
    let keep_from = first_kept_image(parts.len());
    for part in parts.iter_mut().take(keep_from) {
        **part = json!({
            "type": "text",
            "text": "[image dropped from the context: read the file again to see it]",
        });
    }
}

/// `renderToolResultContent`: the single LLM-facing
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

/// The assistant texts among `events`, in order (start content plus deltas), empty ones skipped.
pub fn assistant_texts(events: &[Event]) -> Vec<String> {
    let mut texts: Vec<(&str, String)> = Vec::new();
    for event in events {
        match event {
            Event::MessageStart {
                message_id,
                role: Role::Assistant,
                content,
                ..
            } => texts.push((message_id, content.clone().unwrap_or_default())),
            Event::MessageDelta {
                message_id,
                content,
                ..
            } => {
                if let Some((_, text)) = texts.iter_mut().rev().find(|(id, _)| id == message_id) {
                    text.push_str(content);
                }
            }
            _ => {}
        }
    }
    texts
        .into_iter()
        .map(|(_, text)| text)
        .filter(|text| !text.trim().is_empty())
        .collect()
}

/// True when an event after the last assistant message start matches: the
/// human acted on that answer (wrote its draft, added its rules).
pub fn since_last_answer(events: &[Event], matches: impl Fn(&Event) -> bool) -> bool {
    let last = events.iter().rposition(|event| {
        matches!(
            event,
            Event::MessageStart {
                role: Role::Assistant,
                ..
            }
        )
    });
    last.is_some_and(|start| events[start + 1..].iter().any(matches))
}

/// The text of the last assistant message that has any, among `events`
/// (its start content plus its deltas); "" when none. A workflow node's
/// output.
pub fn last_assistant_text(events: &[Event]) -> String {
    assistant_texts(events).pop().unwrap_or_default()
}

/// A workflow agent node's output among `events`: when the agent recorded a
/// plan with `propose_plan`, the last one as fennec's canonical markdown;
/// else the last assistant text holding a plan or report contract (a
/// `## Plan: …` or `## Report: …` heading, level 2 or 3), else the last
/// assistant text. A closing line the model writes after its plan must not
/// replace the plan.
pub fn node_output(events: &[Event]) -> String {
    if let Some(markdown) = events.iter().rev().find_map(|event| match event {
        Event::PlanProposed { markdown, .. } => Some(markdown),
        _ => None,
    }) {
        return markdown.clone();
    }
    let texts = assistant_texts(events);
    texts
        .iter()
        .rev()
        .find(|text| holds_contract(text))
        .cloned()
        .unwrap_or_else(|| texts.last().cloned().unwrap_or_default())
}

fn holds_contract(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        let rest = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
            .map(str::trim)
            .unwrap_or_default();
        is_contract_heading(rest)
    })
}

/// `Plan: …` or `Report: …`, also with a short qualifier the model adds
/// ("Plan révisé : …", "Revised Plan: …"); the UI's `splitContract` reads the same.
fn is_contract_heading(heading: &str) -> bool {
    let Some((label, _)) = heading.split_once(':') else {
        return false;
    };
    let words: Vec<&str> = label.split_whitespace().collect();
    let keyword = |word: &&str| *word == "Plan" || *word == "Report";
    match words.as_slice() {
        [only] => keyword(only),
        [first, second] => keyword(first) || keyword(second),
        _ => false,
    }
}

/// The content of the first message the human typed (not system-generated,
/// not a loop reminder): what the session was opened for.
pub fn first_request(events: &[Event]) -> Option<String> {
    events.iter().find_map(human_request).map(str::to_string)
}

/// The text of a message the human typed (not one fennec generated), if it is one.
pub fn human_request(event: &Event) -> Option<&str> {
    match event {
        Event::MessageStart {
            role: Role::User,
            is_system_generated: false,
            metadata: None,
            content: Some(content),
            ..
        } if !content.trim().is_empty() => Some(content),
        _ => None,
    }
}

/// The system prompt and tool definitions for the session: the prompt is the
/// first snapshot's (byte-stable for the whole session, so the server's prompt
/// cache holds), the tools are the last snapshot's that carries them (a
/// re-freeze after an MCP change is picked up; sessions from before the tools
/// were frozen got them in a later snapshot).
pub fn frozen_prompt(events: &[Event]) -> (Option<&str>, Option<&Value>) {
    let snapshots: Vec<(&str, Option<&Value>)> = events
        .iter()
        .filter_map(|event| match event {
            Event::TurnSnapshot {
                cached_system_prompt,
                cached_tools,
                ..
            } => Some((cached_system_prompt.as_str(), cached_tools.as_ref())),
            _ => None,
        })
        .collect();
    let prompt = snapshots.first().map(|(prompt, _)| *prompt);
    let tools = snapshots.iter().rev().find_map(|(_, tools)| *tools);
    (prompt, tools)
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
                ..
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
                ..
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
                        "function": {"name": tool_call.name, "arguments": tool_call
                            .raw_arguments
                            .clone()
                            .unwrap_or_else(|| tool_call.arguments.to_string())},
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
    cap_images(&mut messages);
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::attachment::Attachment;
    use crate::domain::event::{ToolCall, ToolResult};

    #[test]
    fn last_assistant_text_skips_messages_without_text() {
        let delta = |id: &str, text: &str| Event::MessageDelta {
            message_id: id.into(),
            content: text.into(),
            tokens_per_second: None,
        };
        let events = vec![
            start("a", Role::Assistant, None, false),
            delta("a", "first "),
            delta("a", "answer"),
            start("u", Role::User, Some("go on"), false),
            delta("u", "ignored"),
            start("b", Role::Assistant, None, false),
        ];
        assert_eq!(last_assistant_text(&events), "first answer");
        assert_eq!(last_assistant_text(&events[3..]), "");
    }

    #[test]
    fn node_output_prefers_plan_over_closing_line() {
        let delta = |id: &str, text: &str| Event::MessageDelta {
            message_id: id.into(),
            content: text.into(),
            tokens_per_second: None,
        };
        let events = vec![
            start("p", Role::Assistant, None, false),
            delta("p", "## Plan: build\n## Done when\n- done"),
            start("c", Role::Assistant, Some("Plan ready."), false),
        ];
        assert_eq!(node_output(&events), "## Plan: build\n## Done when\n- done");
    }

    #[test]
    fn contract_heading_allows_a_short_qualifier() {
        for heading in [
            "Plan: x",
            "Plan : x",
            "Plan révisé : x",
            "Revised Plan: x",
            "Report: x",
        ] {
            assert!(is_contract_heading(heading), "{heading}");
        }
        for heading in ["Planning: x", "Plan", "Plan of the day for the team: x"] {
            assert!(!is_contract_heading(heading), "{heading}");
        }
    }

    #[test]
    fn node_output_falls_back_to_last_assistant_text_without_contract() {
        let events = vec![
            start("a", Role::Assistant, Some("hello"), false),
            start("b", Role::Assistant, Some("world"), false),
        ];
        assert_eq!(node_output(&events), last_assistant_text(&events));
    }

    #[test]
    fn node_output_prefers_later_report_over_plan() {
        let events = vec![
            start("p", Role::Assistant, Some("## Plan: y"), false),
            start("r", Role::Assistant, Some("### Report: x"), false),
        ];
        assert_eq!(node_output(&events), "### Report: x");
    }

    #[test]
    fn node_output_uses_the_last_proposed_plan_markdown() {
        use crate::domain::plan::{Plan, PlanStep, PlannedCriterion};
        let first = Plan {
            title: "First".into(),
            goal: "goal".into(),
            steps: vec![PlanStep {
                title: "s".into(),
                files: vec![],
                change: "c".into(),
            }],
            done_when: vec![PlannedCriterion {
                text: "done".into(),
                check: None,
            }],
            risks: vec![],
        };
        let second = Plan {
            title: "Second".into(),
            ..first.clone()
        };
        let events = vec![
            start("a", Role::Assistant, Some("Plan ready."), false),
            Event::PlanProposed {
                call_id: "c1".into(),
                markdown: first.to_markdown(),
                plan: first.clone(),
            },
            Event::PlanProposed {
                call_id: "c2".into(),
                markdown: second.to_markdown(),
                plan: second.clone(),
            },
            start("c", Role::Assistant, Some("Closing line."), false),
        ];
        assert_eq!(node_output(&events), second.to_markdown());
        // The closing line after the proposal must not replace the plan.
        assert_eq!(
            node_output(&events[..events.len() - 1]),
            second.to_markdown()
        );
        // With only the first proposal, its plan is the output.
        assert_eq!(node_output(&events[..2]), first.to_markdown());
    }

    #[test]
    fn first_request_skips_system_generated_and_assistant_messages() {
        let events = vec![
            start("s", Role::User, Some("system"), true),
            start("a", Role::Assistant, Some("hello"), false),
            start("u", Role::User, Some("  "), false),
            start("v", Role::User, Some("do it"), false),
        ];
        assert_eq!(first_request(&events), Some("do it".into()));
    }

    #[test]
    fn first_request_returns_the_first_user_message() {
        let events = vec![
            start("u1", Role::User, Some("first"), false),
            start("u2", Role::User, Some("second"), false),
        ];
        assert_eq!(first_request(&events), Some("first".into()));
    }

    #[test]
    fn first_request_is_none_without_user_messages() {
        let events = vec![start("a", Role::Assistant, Some("hi"), false)];
        assert_eq!(first_request(&events), None);
    }

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
    fn frozen_prompt_reads_the_first_prompt_and_the_last_tools() {
        let events = vec![
            Event::TurnSnapshot {
                cached_system_prompt: "first".into(),
                cached_tools: None,
                mcp_tool_names: Vec::new(),
            },
            Event::TurnSnapshot {
                cached_system_prompt: "second".into(),
                cached_tools: Some(json!([1])),
                mcp_tool_names: Vec::new(),
            },
            Event::TurnSnapshot {
                cached_system_prompt: "third".into(),
                cached_tools: Some(json!([2])),
                mcp_tool_names: Vec::new(),
            },
        ];

        let (prompt, tools) = frozen_prompt(&events);
        assert_eq!(prompt, Some("first"));
        assert_eq!(tools, Some(&json!([2])));
        assert_eq!(frozen_prompt(&[]), (None, None));
    }

    #[test]
    fn fold_rebuilds_assistant_tool_calls_and_tool_results() {
        let events = vec![
            start("u", Role::User, Some("do it"), false),
            start("a", Role::Assistant, None, false),
            Event::MessageThinking {
                message_id: "a".into(),
                content: "hmm".into(),
                tokens_per_second: None,
            },
            Event::MessageDelta {
                message_id: "a".into(),
                content: "ok".into(),
                tokens_per_second: None,
            },
            Event::ToolCall {
                message_id: "a".into(),
                tool_call: ToolCall {
                    id: "c1".into(),
                    name: "read_file".into(),
                    arguments: json!({"path": "x"}),
                    parse_error: None,
                    raw_arguments: None,
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
    fn tool_call_arguments_fold_back_as_the_model_streamed_them() {
        let raw = r#"{"path": "x", "content": "y"}"#;
        let events = [
            start("a", Role::Assistant, None, false),
            Event::ToolCall {
                message_id: "a".into(),
                tool_call: ToolCall {
                    id: "c1".into(),
                    name: "write_file".into(),
                    arguments: serde_json::from_str(raw).unwrap(),
                    parse_error: None,
                    raw_arguments: Some(raw.into()),
                },
            },
        ];
        let msgs = fold_messages(&events);
        let sent = msgs
            .iter()
            .find_map(|m| m["tool_calls"][0]["function"]["arguments"].as_str());
        // Re-serialized, the keys would come back sorted and without spaces.
        assert_eq!(sent, Some(raw));
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

    #[test]
    fn cap_images_leaves_three_images_unchanged() {
        let mut messages = vec![
            json!({"role": "user", "content": [image_part("a")]}),
            json!({"role": "user", "content": [image_part("b")]}),
            json!({"role": "user", "content": [image_part("c")]}),
        ];
        cap_images(&mut messages);
        for message in &messages {
            assert_eq!(
                message["content"][0].get("type").and_then(Value::as_str),
                Some("image_url")
            );
        }
    }

    #[test]
    fn cap_images_leaves_six_images_unchanged() {
        let mut messages: Vec<Value> = (0..6)
            .map(|i| {
                json!({
                    "role": "user",
                    "content": [image_part(&format!("img{}", i))],
                })
            })
            .collect();
        cap_images(&mut messages);
        for message in &messages {
            assert_eq!(
                message["content"][0].get("type").and_then(Value::as_str),
                Some("image_url")
            );
        }
    }

    #[test]
    fn cap_images_drops_the_first_three_of_seven_images() {
        let mut messages: Vec<Value> = (0..7)
            .map(|i| {
                json!({
                    "role": "user",
                    "content": [image_part(&format!("img{}", i))],
                })
            })
            .collect();
        cap_images(&mut messages);
        for message in &messages[..3] {
            assert_eq!(message["content"][0], dropped_part());
        }
        for message in &messages[3..] {
            assert_eq!(
                message["content"][0].get("type").and_then(Value::as_str),
                Some("image_url")
            );
        }
    }

    #[test]
    fn cap_images_drops_the_first_six_of_ten_images() {
        let mut messages: Vec<Value> = (0..10)
            .map(|i| {
                json!({
                    "role": "user",
                    "content": [image_part(&format!("img{}", i))],
                })
            })
            .collect();
        cap_images(&mut messages);
        for message in &messages[..6] {
            assert_eq!(message["content"][0], dropped_part());
        }
        for message in &messages[6..] {
            assert_eq!(
                message["content"][0].get("type").and_then(Value::as_str),
                Some("image_url")
            );
        }
    }

    fn image_part(url: &str) -> Value {
        json!({
            "type": "image_url",
            "image_url": {"url": url},
        })
    }

    fn dropped_part() -> Value {
        json!({
            "type": "text",
            "text": "[image dropped from the context: read the file again to see it]",
        })
    }
}
