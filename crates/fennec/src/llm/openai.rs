//! Port of openfox `llm/client.ts`: one streaming call to an
//! OpenAI-compatible `/chat/completions` endpoint.
//!
//! Not ported yet: the provider shims of openfox `requestBody`
//! (llama.cpp `chat_template_kwargs.thinking`, Qwen3 `enable_thinking`);
//! the standard `reasoning_effort` field is sent when set.

use super::sse;
use crate::agent::ports::{Completion, Delta, Model, Request};
use crate::config::LlmSettings;
use serde_json::json;
use std::io::BufReader;

pub struct OpenAiClient {
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl OpenAiClient {
    pub fn new(settings: &LlmSettings, model: &str) -> OpenAiClient {
        OpenAiClient {
            base_url: settings.base_url.clone(),
            model: model.to_string(),
            api_key: settings.api_key.clone(),
        }
    }
}

impl Model for OpenAiClient {
    fn name(&self) -> &str {
        &self.model
    }

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
        if let Some(effort) = req.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }
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
        sse::parse(BufReader::new(body.into_reader()), req.stop, on)
    }
}
