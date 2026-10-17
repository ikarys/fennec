//! The model behind an agent: any OpenAI-compatible chat endpoint.

use std::io;
use std::time::Duration;

use serde_json::{Value, json};

/// One chat turn. A trait so tests can script the model.
pub trait Model: Sync {
    /// Returns the assistant message (`content`, `tool_calls`).
    fn chat(&self, messages: &[Value], tools: &Value) -> io::Result<Value>;
    /// A turn without the model's reasoning, for turns that need none. A
    /// model that cannot skip it reasons anyway.
    fn chat_quick(&self, messages: &[Value], tools: &Value) -> io::Result<Value> {
        self.chat(messages, tools)
    }
    /// Names the variant in run records.
    fn label(&self) -> String;
}

pub struct OpenAi {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: f64,
    agent: ureq::Agent,
}

impl OpenAi {
    pub fn new(base_url: &str, model: &str, api_key: Option<String>, temperature: f64) -> Self {
        // Local models on shared hardware can take minutes for one turn.
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(900)))
            .build();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key,
            temperature,
            agent: ureq::Agent::new_with_config(config),
        }
    }

    /// One variant per (model, temperature) pair the run asks for.
    /// `FENNEC_OS_BASE_URL` (default `http://localhost:8000/v1`),
    /// `FENNEC_OS_MODELS` (comma-separated), `FENNEC_OS_API_KEY`.
    pub fn variants_from_env(attempts: usize) -> io::Result<Vec<Self>> {
        let base = std::env::var("FENNEC_OS_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8000/v1".to_string());
        let models = std::env::var("FENNEC_OS_MODELS")
            .map_err(|_| io::Error::other("FENNEC_OS_MODELS is not set"))?;
        let models: Vec<&str> = models
            .split(',')
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .collect();
        if models.is_empty() {
            return Err(io::Error::other("FENNEC_OS_MODELS is empty"));
        }
        let key = std::env::var("FENNEC_OS_API_KEY").ok();
        // Diversity knob: the same model at different temperatures explores
        // different paths; several models add a second axis.
        const TEMPERATURES: [f64; 4] = [0.3, 0.8, 0.5, 1.0];
        Ok((0..attempts)
            .map(|i| {
                let model = models[i % models.len()];
                let temperature = TEMPERATURES[(i / models.len()) % TEMPERATURES.len()];
                Self::new(&base, model, key.clone(), temperature)
            })
            .collect())
    }
}

impl OpenAi {
    fn send(&self, messages: &[Value], tools: &Value, think: bool) -> io::Result<Value> {
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "temperature": self.temperature,
        });
        // The chat-template switch Qwen-style models read (vLLM, sglang,
        // ninfer); servers without it ignore unknown keys.
        if !think {
            body["chat_template_kwargs"] = json!({"enable_thinking": false});
        }
        // Some servers reject an empty tool list.
        if tools.as_array().is_some_and(|t| !t.is_empty()) {
            body["tools"] = tools.clone();
        }
        let mut req = self
            .agent
            .post(format!("{}/chat/completions", self.base_url));
        if let Some(key) = &self.api_key {
            req = req.header("Authorization", &format!("Bearer {key}"));
        }
        let mut resp = req.send_json(&body).map_err(io::Error::other)?;
        let mut reply: Value = resp.body_mut().read_json().map_err(io::Error::other)?;
        match reply["choices"][0]["message"].take() {
            Value::Null => Err(io::Error::other(format!("no message in reply: {reply}"))),
            mut message => {
                // Token counts, for the profile; the agent takes them out.
                if !reply["usage"].is_null() {
                    message["usage"] = reply["usage"].take();
                }
                Ok(message)
            }
        }
    }
}

impl Model for OpenAi {
    fn chat(&self, messages: &[Value], tools: &Value) -> io::Result<Value> {
        self.send(messages, tools, true)
    }

    fn chat_quick(&self, messages: &[Value], tools: &Value) -> io::Result<Value> {
        self.send(messages, tools, false)
    }

    fn label(&self) -> String {
        format!("{}@{}", self.model, self.temperature)
    }
}
