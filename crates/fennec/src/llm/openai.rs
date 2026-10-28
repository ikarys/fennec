//! Adapter from fennec's `Model` port to `llm_providers`.
//!
//! Not ported yet: the provider shims of openfox `requestBody`
//! (llama.cpp `chat_template_kwargs.thinking`, Qwen3 `enable_thinking`);
//! the standard `reasoning_effort` field is sent when set.

use crate::agent::ports::{Completion, Delta, Model, Request};
use crate::config::LlmSettings;
use crate::domain::event::ToolCall;
use crate::domain::id::new_id;
use llm_providers::{ChatRequest, Endpoint, StreamedToolCall};
use serde_json::{Value, json};

pub struct OpenAiClient {
    endpoint: Endpoint,
    model: String,
}

impl OpenAiClient {
    pub fn new(settings: &LlmSettings, model: &str) -> OpenAiClient {
        OpenAiClient {
            endpoint: settings.endpoint(),
            model: model.to_string(),
        }
    }
}

impl Model for OpenAiClient {
    fn name(&self) -> &str {
        &self.model
    }

    fn stream(&self, req: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
        let chat_request = ChatRequest {
            model: &self.model,
            system_prompt: req.system_prompt,
            messages: req.messages,
            tools: req.tools,
            max_tokens: req.max_tokens,
            reasoning_effort: req.reasoning_effort,
        };
        let streamed = llm_providers::stream(&self.endpoint, &chat_request, req.stop, on)?;
        Ok(Completion {
            content: streamed.content,
            thinking: streamed.thinking,
            tool_calls: streamed.tool_calls.into_iter().map(tool_call).collect(),
            finish_reason: streamed.finish_reason,
            prompt_tokens: streamed.prompt_tokens,
            cached_tokens: streamed.cached_tokens,
            completion_tokens: streamed.completion_tokens,
        })
    }
}

fn tool_call(streamed: StreamedToolCall) -> ToolCall {
    let raw: &str = if streamed.arguments.trim().is_empty() {
        "{}"
    } else {
        &streamed.arguments
    };
    // Invalid JSON is not sent back: some chat templates parse it.
    let (arguments, parse_error, raw_arguments) = match serde_json::from_str::<Value>(raw) {
        Ok(v) => (v, None, Some(raw.to_string())),
        Err(e) => (
            json!({}),
            Some(format!("Invalid JSON arguments: {e}: {raw}")),
            None,
        ),
    };
    ToolCall {
        id: if streamed.id.is_empty() {
            new_id()
        } else {
            streamed.id
        },
        name: streamed.name,
        arguments,
        parse_error,
        raw_arguments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_tool_arguments_are_kept_as_a_parse_error() {
        let call = tool_call(StreamedToolCall {
            id: "c".into(),
            name: "x".into(),
            arguments: "{oops".into(),
        });
        assert!(call.parse_error.is_some());
    }

    #[test]
    fn valid_tool_arguments_keep_the_raw_arguments() {
        let call = tool_call(StreamedToolCall {
            id: "c".into(),
            name: "x".into(),
            arguments: "{\"path\":\"a\"}".into(),
        });
        assert!(call.parse_error.is_none());
        assert_eq!(call.raw_arguments.as_deref(), Some("{\"path\":\"a\"}"));
        assert_eq!(call.arguments, json!({"path": "a"}));
    }
}
