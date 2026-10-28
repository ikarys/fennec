//! Adapter from fennec's `Model` port to `llm_providers`.
//!
//! Not ported yet: the provider shims of openfox `requestBody`
//! (llama.cpp `chat_template_kwargs.thinking`, Qwen3 `enable_thinking`);
//! the standard `reasoning_effort` field is sent when set.

use super::ModelGate;
use crate::agent::ports::{Completion, Delta, Model, Request};
use crate::config;
use crate::domain::event::ToolCall;
use crate::domain::id::new_id;
use llm_providers::{ChatRequest, Endpoint, StreamedToolCall};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// One gate per endpoint: a local GPU serves one round at a time
/// (`FENNEC_MODEL_CONCURRENCY`, default one), but a cloud endpoint that
/// tolerates parallelism must not be blocked by a long local round. The gates
/// are keyed by the endpoint's base URL, not shared process-wide. Tool calls
/// are not gated.
static GATES: OnceLock<Mutex<HashMap<String, Arc<ModelGate>>>> = OnceLock::new();

fn gate_for(base_url: &str) -> Arc<ModelGate> {
    let gates = GATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = gates.lock().unwrap();
    map.entry(base_url.to_string())
        .or_insert_with(|| Arc::new(ModelGate::new(config::model_concurrency())))
        .clone()
}

pub struct OpenAiClient {
    endpoint: Endpoint,
    model: String,
}

impl OpenAiClient {
    pub fn new(endpoint: Endpoint, model: &str) -> OpenAiClient {
        OpenAiClient {
            endpoint,
            model: model.to_string(),
        }
    }
}

impl Model for OpenAiClient {
    fn name(&self) -> &str {
        &self.model
    }

    fn stream(&self, req: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
        let gate = gate_for(&self.endpoint.base_url);
        let _slot = gate.enter();
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

    #[test]
    fn distinct_endpoints_get_independent_gates() {
        let a = gate_for("http://gate-test-a.invalid");
        let b = gate_for("http://gate-test-b.invalid");

        // Holding a slot on endpoint A does not block endpoint B.
        let _a_slot = a.enter();
        let (tx, rx) = std::sync::mpsc::channel();
        let b2 = b.clone();
        let worker = std::thread::spawn(move || {
            let _b_slot = b2.enter();
            let _ = tx.send(());
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(200))
                .is_ok()
        );
        worker.join().unwrap();

        // The same endpoint is serialized: a slot on A blocks a second on A.
        let c = gate_for("http://gate-test-a.invalid");
        let (tx2, rx2) = std::sync::mpsc::channel();
        let c2 = c.clone();
        let worker2 = std::thread::spawn(move || {
            let _c_slot = c2.enter();
            let _ = tx2.send(());
        });
        assert!(
            rx2.recv_timeout(std::time::Duration::from_millis(80))
                .is_err()
        );
        drop(_a_slot);
        assert!(
            rx2.recv_timeout(std::time::Duration::from_millis(300))
                .is_ok()
        );
        worker2.join().unwrap();
    }
}
