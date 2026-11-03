//! Adapter from `Model` to `llm_providers`.
//!
//! It sends no provider shims in the request body for llama.cpp
//! `chat_template_kwargs.thinking` or Qwen3 `enable_thinking`; the standard
//! `reasoning_effort` field is sent when set.

use super::ModelGate;
use crate::agent::ports::{Completion, Delta, Model, Request};
use crate::config;
use crate::domain::event::ToolCall;
use crate::domain::id::new_id;
use llm_providers::{ChatRequest, Endpoint, StreamedToolCall};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, OnceLock};

/// One gate per local endpoint: a local GPU serves one round at a time
/// (`FENNEC_MODEL_CONCURRENCY`, default one) and keeps serving the same
/// session between its rounds so the server's prompt cache stays warm. Remote endpoints are not gated;
/// the setting applies to local ones. The gates are keyed by the endpoint's
/// base URL, not shared process-wide. Tool calls are not gated.
/// ponytail: host heuristic; a per-provider concurrency setting if a remote
/// endpoint needs a cap.
static GATES: OnceLock<Mutex<HashMap<String, Arc<ModelGate>>>> = OnceLock::new();

fn gate_for(base_url: &str) -> Arc<ModelGate> {
    let gates = GATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = gates.lock().unwrap();
    map.entry(base_url.to_string())
        .or_insert_with(|| {
            Arc::new(ModelGate::new(
                config::model_concurrency(),
                config::model_affinity(),
            ))
        })
        .clone()
}

/// Local endpoints are the ones a single machine can serialize: loopback,
/// private LAN, link-local, and mDNS `.local` names. A URL without a host is
/// gated too, because the gate is the safe side.
fn is_local(base_url: &str) -> bool {
    let host = host_from_url(base_url);
    if host.is_empty() {
        return true;
    }
    if host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(ip) => is_local_ip(ip),
        Err(_) => false,
    }
}

fn host_from_url(url: &str) -> &str {
    let without_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    let authority = without_scheme.split('/').next().unwrap_or("");
    let without_userinfo = authority.rsplit('@').next().unwrap_or(authority);
    if let Some(bracketed) = without_userinfo.strip_prefix('[') {
        return bracketed.split(']').next().unwrap_or(bracketed);
    }
    without_userinfo
        .split_once(':')
        .map_or(without_userinfo, |(host, _port)| host)
}

fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
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
        // Only local endpoints take a gate slot; a stopped session must not
        // wait behind another session's long local round.
        let gate = is_local(&self.endpoint.base_url).then(|| gate_for(&self.endpoint.base_url));
        let _slot = match &gate {
            Some(gate) => Some(
                gate.enter(req.session, req.stop)
                    .ok_or_else(|| "Aborted".to_string())?,
            ),
            None => None,
        };
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

    fn release(&self, session: &str) {
        if is_local(&self.endpoint.base_url) {
            gate_for(&self.endpoint.base_url).release(session);
        }
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
    use std::sync::atomic::AtomicBool;

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
        let _a_slot = a.enter("s", &AtomicBool::new(false)).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let b2 = b.clone();
        let worker = std::thread::spawn(move || {
            let _b_slot = b2.enter("s", &AtomicBool::new(false)).unwrap();
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
            let _c_slot = c2.enter("s", &AtomicBool::new(false)).unwrap();
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

    #[test]
    fn local_and_remote_hosts_are_gated_differently() {
        assert!(is_local("http://127.0.0.1:8000/v1"));
        assert!(is_local("http://localhost:8000/v1"));
        assert!(is_local("http://172.20.0.1:8000/v1"));
        assert!(is_local("http://[::1]:8000/v1"));
        assert!(!is_local("https://api.openai.com/v1"));
        assert!(!is_local("https://openrouter.ai/api/v1"));
        assert!(is_local("http://user@10.0.0.2/v1"));
    }
}
