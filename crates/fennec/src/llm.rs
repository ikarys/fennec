//! Port of openfox `src/server/llm/{client,streaming}.ts`: one streaming
//! call to an OpenAI-compatible `/chat/completions` endpoint.

use crate::events::ToolCall;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};

pub struct Request<'a> {
    pub system_prompt: &'a str,
    pub messages: &'a [Value],
    pub tools: &'a Value,
    pub max_tokens: u32,
}

/// What the stream yields while it runs (openfox `StreamEvent`).
#[derive(Debug, PartialEq)]
pub enum Delta {
    Text(String),
    Thinking(String),
    /// A tool call's name is known before its arguments finish streaming.
    ToolPreparing {
        index: usize,
        name: String,
    },
}

#[derive(Debug, Default, PartialEq)]
pub struct Completion {
    pub content: String,
    pub thinking: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// The model behind the loop; the trait exists so the loop can be tested
/// against a scripted model.
pub trait Model {
    fn stream(&self, req: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String>;
}

pub struct OpenAiClient {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl Model for OpenAiClient {
    fn stream(&self, req: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
        let mut messages = vec![json!({"role": "system", "content": req.system_prompt})];
        messages.extend_from_slice(req.messages);
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": req.max_tokens,
            "stream": true,
            "stream_options": {"include_usage": true},
        });
        if req.tools.as_array().is_some_and(|t| !t.is_empty()) {
            body["tools"] = req.tools.clone();
            body["tool_choice"] = json!("auto");
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut call = agent.post(&url).header("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            call = call.header("Authorization", &format!("Bearer {key}"));
        }
        let resp = call.send_json(&body).map_err(|e| e.to_string())?;
        let status = resp.status();
        let mut body = resp.into_body();
        if !status.is_success() {
            let text = body.read_to_string().unwrap_or_default();
            return Err(format!("LLM HTTP {status}: {text}"));
        }
        parse_sse(BufReader::new(body.into_reader()), on)
    }
}

#[derive(Default)]
struct PendingCall {
    id: String,
    name: String,
    arguments: String,
}

/// Parses an SSE chat-completion stream, accumulating tool-call fragments
/// by index as the OpenAI protocol sends them.
pub fn parse_sse(reader: impl BufRead, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
    let mut out = Completion::default();
    let mut calls: Vec<PendingCall> = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| format!("stream read failed: {e}"))?;
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" {
            break;
        }
        let chunk: Value = serde_json::from_str(data).map_err(|e| format!("bad SSE chunk: {e}"))?;
        if let Some(err) = chunk.get("error") {
            return Err(format!("LLM error: {err}"));
        }
        if let Some(usage) = chunk.get("usage").filter(|u| u.is_object()) {
            out.prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
            out.completion_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
        }
        let Some(choice) = chunk["choices"].get(0) else {
            continue;
        };
        if let Some(reason) = choice["finish_reason"].as_str() {
            out.finish_reason = reason.to_string();
        }
        let delta = &choice["delta"];
        // Same field precedence as openfox `getThinking`.
        let thinking = ["reasoning", "reasoning_content", "thinking"]
            .iter()
            .find_map(|k| delta[k].as_str());
        if let Some(t) = thinking.filter(|t| !t.is_empty()) {
            out.thinking.push_str(t);
            on(Delta::Thinking(t.to_string()));
        }
        if let Some(t) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            out.content.push_str(t);
            on(Delta::Text(t.to_string()));
        }
        for tc in delta["tool_calls"].as_array().into_iter().flatten() {
            let index = tc["index"].as_u64().unwrap_or(0) as usize;
            if calls.len() <= index {
                calls.resize_with(index + 1, PendingCall::default);
            }
            let call = &mut calls[index];
            if let Some(id) = tc["id"].as_str() {
                call.id = id.to_string();
            }
            if let Some(name) = tc["function"]["name"].as_str().filter(|n| !n.is_empty()) {
                call.name.push_str(name);
                on(Delta::ToolPreparing {
                    index,
                    name: call.name.clone(),
                });
            }
            if let Some(args) = tc["function"]["arguments"].as_str() {
                call.arguments.push_str(args);
            }
        }
    }
    out.tool_calls = calls
        .into_iter()
        .filter(|c| !c.name.is_empty())
        .map(|c| {
            let raw = if c.arguments.trim().is_empty() {
                "{}"
            } else {
                &c.arguments
            };
            let (arguments, parse_error) = match serde_json::from_str::<Value>(raw) {
                Ok(v) => (v, None),
                Err(e) => (
                    json!({}),
                    Some(format!("Invalid JSON arguments: {e}: {raw}")),
                ),
            };
            ToolCall {
                id: if c.id.is_empty() {
                    crate::events::new_id()
                } else {
                    c.id
                },
                name: c.name,
                arguments,
                parse_error,
            }
        })
        .collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_accumulates_text_thinking_and_fragmented_tool_calls() {
        let stream = r#"data: {"choices":[{"delta":{"reasoning_content":"hmm"}}]}

data: {"choices":[{"delta":{"content":"Hi"}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"read_file","arguments":"{\"pa"}}]}}]}
data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":\"a\"}"}}]},"finish_reason":"tool_calls"}]}
data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}
data: [DONE]
"#;
        let mut deltas = Vec::new();
        let c = parse_sse(stream.as_bytes(), &mut |d| deltas.push(d)).unwrap();
        assert_eq!(c.content, "Hi");
        assert_eq!(c.thinking, "hmm");
        assert_eq!(c.finish_reason, "tool_calls");
        assert_eq!((c.prompt_tokens, c.completion_tokens), (10, 5));
        assert_eq!(c.tool_calls.len(), 1);
        assert_eq!(c.tool_calls[0].id, "c1");
        assert_eq!(c.tool_calls[0].arguments, json!({"path": "a"}));
        assert_eq!(deltas.len(), 3);
    }

    #[test]
    fn invalid_tool_arguments_are_kept_as_a_parse_error() {
        let stream = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c","function":{"name":"x","arguments":"{oops"}}]}}]}
"#;
        let c = parse_sse(stream.as_bytes(), &mut |_| {}).unwrap();
        assert!(c.tool_calls[0].parse_error.is_some());
    }
}
