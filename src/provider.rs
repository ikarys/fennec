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

    /// One variant per (model, temperature) pair the run asks for, from
    /// [`Settings::current`].
    pub fn variants_from_env(attempts: usize) -> io::Result<Vec<Self>> {
        let Settings {
            base_url: base,
            models,
            api_key: key,
        } = Settings::current()?;
        // None chosen: the models the server says it serves, so a local
        // server that serves one model needs no configuration but its address.
        let models = if models.is_empty() {
            served(&base, key.as_deref()).map_err(|e| {
                io::Error::other(format!(
                    "no model: none is set and {base}/models did not answer ({e})"
                ))
            })?
        } else {
            models
        };
        if models.is_empty() {
            return Err(io::Error::other(format!(
                "no model: {base}/models lists none"
            )));
        }
        // Diversity knob: the same model at different temperatures explores
        // different paths; several models add a second axis.
        const TEMPERATURES: [f64; 4] = [0.3, 0.8, 0.5, 1.0];
        Ok((0..attempts)
            .map(|i| {
                let model = &models[i % models.len()];
                let temperature = TEMPERATURES[(i / models.len()) % TEMPERATURES.len()];
                Self::new(&base, model, key.clone(), temperature)
            })
            .collect())
    }
}

/// Where the model is: the human's settings file
/// (`~/.config/fennec/model.json`, written from the web UI), each field
/// overridden by its environment variable (`FENNEC_OS_BASE_URL`,
/// `FENNEC_OS_MODELS` comma-separated, `FENNEC_OS_API_KEY`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub base_url: String,
    /// Empty: every model the server lists.
    pub models: Vec<String>,
    pub api_key: Option<String>,
}

pub const DEFAULT_BASE_URL: &str = "http://localhost:8000/v1";
/// The variables that win over the file, for the UI to say so.
pub const ENV: [&str; 3] = [
    "FENNEC_OS_BASE_URL",
    "FENNEC_OS_MODELS",
    "FENNEC_OS_API_KEY",
];

impl Settings {
    fn path() -> io::Result<std::path::PathBuf> {
        Ok(crate::rules::config_dir()?.join("model.json"))
    }

    /// The file alone, as the human wrote it.
    pub fn saved() -> io::Result<Self> {
        let v: Value = match std::fs::read_to_string(Self::path()?) {
            Ok(t) => serde_json::from_str(&t).map_err(io::Error::other)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Value::Null,
            Err(e) => return Err(e),
        };
        Ok(Self {
            base_url: v["base_url"]
                .as_str()
                .filter(|b| !b.is_empty())
                .unwrap_or(DEFAULT_BASE_URL)
                .to_string(),
            models: v["models"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|m| m.as_str().map(str::to_string))
                .collect(),
            api_key: v["api_key"]
                .as_str()
                .filter(|k| !k.is_empty())
                .map(str::to_string),
        })
    }

    /// What a run uses: the file, then the environment on top.
    pub fn current() -> io::Result<Self> {
        let mut s = Self::saved()?;
        if let Ok(b) = std::env::var("FENNEC_OS_BASE_URL") {
            s.base_url = b;
        }
        if let Ok(m) = std::env::var("FENNEC_OS_MODELS") {
            s.models = m
                .split(',')
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Ok(k) = std::env::var("FENNEC_OS_API_KEY") {
            s.api_key = Some(k);
        }
        Ok(s)
    }

    /// Readable by its owner only: it may hold a key.
    pub fn save(&self) -> io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = Self::path()?;
        std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
        let json =
            json!({"base_url": self.base_url, "models": self.models, "api_key": self.api_key});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        std::io::Write::write_all(
            &mut file,
            (serde_json::to_string_pretty(&json).map_err(io::Error::other)? + "\n").as_bytes(),
        )
    }
}

/// The model ids an OpenAI-compatible server lists.
pub fn served(base: &str, key: Option<&str>) -> io::Result<Vec<String>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let mut req = agent.get(format!("{}/models", base.trim_end_matches('/')));
    if let Some(key) = key {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let mut resp = req.call().map_err(io::Error::other)?;
    let list: Value = resp.body_mut().read_json().map_err(io::Error::other)?;
    Ok(list["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().map(str::to_string))
        .collect())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn the_server_names_its_models_when_none_is_set() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let n = stream.read(&mut request).unwrap();
            assert!(String::from_utf8_lossy(&request[..n]).starts_with("GET /v1/models "));
            let body = r#"{"data":[{"id":"qwen3.8-27b"},{"id":"small"}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        assert_eq!(served(&base, None).unwrap(), ["qwen3.8-27b", "small"]);
        server.join().unwrap();
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let gone = format!("http://{}/v1", closed.local_addr().unwrap());
        drop(closed);
        assert!(served(&gone, None).is_err(), "nothing listens");
    }
}
