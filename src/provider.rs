//! The model behind an agent: any OpenAI-compatible chat endpoint.

use std::io;
use std::time::{Duration, Instant};

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

/// One question without tools, asked again without reasoning when the reply
/// holds no answer (seen: a whole budget spent thinking). Returns the answer,
/// reasoning stripped, and the exchange recorded the way an agent's is
/// (`{"model", "messages", "turns"}`), so the human can read what was said.
pub fn ask(model: &dyn Model, messages: &[Value]) -> io::Result<(String, Value)> {
    let (mut said, mut turns, mut answer) = (messages.to_vec(), Vec::new(), String::new());
    for think in [true, false] {
        let asked = Instant::now();
        let mut reply = match think {
            true => model.chat(messages, &json!([]))?,
            false => model.chat_quick(messages, &json!([]))?,
        };
        let usage = reply.as_object_mut().and_then(|m| m.remove("usage"));
        turns.push(json!({"model_ms": asked.elapsed().as_millis() as u64,
                          "think": think, "usage": usage}));
        let content = reply["content"].as_str().unwrap_or("");
        // Some servers leave the reasoning inline.
        answer = content
            .rsplit("</think>")
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        said.push(reply);
        if !answer.is_empty() {
            break;
        }
    }
    Ok((
        answer,
        json!({"model": model.label(), "messages": said, "turns": turns}),
    ))
}

/// A model that writes the exchange so far to a file after every call, in
/// the shape [`ask`] and agents record it, so a human can follow a step while
/// it runs instead of only once it ends.
pub struct Live<'a> {
    inner: &'a dyn Model,
    path: std::path::PathBuf,
    turns: std::sync::Mutex<Vec<Value>>,
}

impl<'a> Live<'a> {
    /// Starts empty: a file left by the previous step is not this one.
    pub fn new(inner: &'a dyn Model, path: std::path::PathBuf) -> Self {
        let _ = std::fs::remove_file(&path);
        Self {
            inner,
            path,
            turns: Default::default(),
        }
    }

    fn record(
        &self,
        messages: &[Value],
        reply: io::Result<Value>,
        think: bool,
        asked: Instant,
    ) -> io::Result<Value> {
        let mut reply = reply?;
        let mut turns = self.turns.lock().unwrap_or_else(|e| e.into_inner());
        // A conversation of two messages is a new one: another attempt, or a
        // stopped agent asked for its question.
        if messages.len() <= 2 {
            turns.clear();
        }
        let usage = reply.as_object_mut().and_then(|m| m.remove("usage"));
        turns.push(json!({"model_ms": asked.elapsed().as_millis() as u64,
                          "think": think, "usage": usage.clone()}));
        let mut said = messages.to_vec();
        said.push(reply.clone());
        let now = json!({"model": self.inner.label(), "messages": said, "turns": *turns});
        // Watching is best effort: a write that fails never stops the work.
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, now.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
        if let (Some(m), Some(u)) = (reply.as_object_mut(), usage) {
            m.insert("usage".into(), u);
        }
        Ok(reply)
    }
}

impl Model for Live<'_> {
    fn chat(&self, messages: &[Value], tools: &Value) -> io::Result<Value> {
        let asked = Instant::now();
        self.record(messages, self.inner.chat(messages, tools), true, asked)
    }

    fn chat_quick(&self, messages: &[Value], tools: &Value) -> io::Result<Value> {
        let asked = Instant::now();
        self.record(
            messages,
            self.inner.chat_quick(messages, tools),
            false,
            asked,
        )
    }

    fn label(&self) -> String {
        self.inner.label()
    }
}

pub struct OpenAi {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: f64,
    /// See [`Settings::max_tokens`].
    pub max_tokens: u64,
    /// See [`Settings::effort`].
    pub effort: String,
    agent: ureq::Agent,
}

impl OpenAi {
    pub fn new(base_url: &str, model: &str, api_key: Option<String>, temperature: f64) -> Self {
        // Local models on shared hardware can take minutes for one turn.
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(900)))
            // An error status is read below: its body says why.
            .http_status_as_error(false)
            .build();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key,
            temperature,
            max_tokens: DEFAULT_MAX_TOKENS,
            effort: String::new(),
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
            max_tokens,
            effort,
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
                Self {
                    max_tokens,
                    effort: effort.clone(),
                    ..Self::new(&base, model, key.clone(), temperature)
                }
            })
            .collect())
    }
}

/// Where the model is: the human's settings file
/// (`~/.config/fennec/model.json`, written from the web UI), each field
/// overridden by its environment variable (`FENNEC_OS_BASE_URL`,
/// `FENNEC_OS_MODELS` comma-separated, `FENNEC_OS_API_KEY`, `FENNEC_OS_MAX_TOKENS`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub base_url: String,
    /// Empty: every model the server lists.
    pub models: Vec<String>,
    pub api_key: Option<String>,
    /// A reply's budget, reasoning included; 0 leaves it to the server.
    /// Servers default low (ninfer: 8192), and a reasoning model can spend
    /// all of it thinking and answer nothing.
    pub max_tokens: u64,
    /// The standard `reasoning_effort` sent with a reasoning call (none,
    /// low, medium, xhigh…); empty sends none, so the model's own default
    /// holds (Qwen 3.8: xhigh, which can think a whole budget away).
    pub effort: String,
}

pub const DEFAULT_MAX_TOKENS: u64 = 32_768;
pub const DEFAULT_BASE_URL: &str = "http://localhost:8000/v1";
/// The variables that win over the file, for the UI to say so.
pub const ENV: [&str; 4] = [
    "FENNEC_OS_BASE_URL",
    "FENNEC_OS_MODELS",
    "FENNEC_OS_API_KEY",
    "FENNEC_OS_MAX_TOKENS",
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
            max_tokens: v["max_tokens"].as_u64().unwrap_or(DEFAULT_MAX_TOKENS),
            effort: v["effort"].as_str().unwrap_or("").to_string(),
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
        if let Some(n) = std::env::var("FENNEC_OS_MAX_TOKENS")
            .ok()
            .and_then(|n| n.parse().ok())
        {
            s.max_tokens = n;
        }
        Ok(s)
    }

    /// Readable by its owner only: it may hold a key.
    pub fn save(&self) -> io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = Self::path()?;
        std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
        let json = json!({"base_url": self.base_url, "models": self.models, "api_key": self.api_key,
                   "max_tokens": self.max_tokens, "effort": self.effort});
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
            "messages": outgoing(messages),
            "temperature": self.temperature,
        });
        if self.max_tokens > 0 {
            body["max_tokens"] = json!(self.max_tokens);
        }
        // The chat-template switch Qwen-style models read (vLLM, sglang,
        // ninfer); servers without it ignore unknown keys.
        if !think {
            body["chat_template_kwargs"] = json!({"enable_thinking": false});
        } else if !self.effort.is_empty() {
            // Not with the switch above: ninfer rejects the two together.
            body["reasoning_effort"] = json!(self.effort);
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
        let status = resp.status().as_u16();
        if status >= 400 {
            // The server says why (an unknown model, a context too long);
            // ureq's own error would keep the status alone.
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            let why = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                .unwrap_or(text);
            let why: String = why.chars().take(300).collect();
            return Err(io::Error::other(format!("http status: {status}: {why}")));
        }
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

/// Reasoning is the model's scratch space, kept in the exchange for the human
/// to read; sending it back costs prefill. Images the human attached go as
/// images (see [`image_line`]).
fn outgoing(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| {
            let mut m = m.clone();
            if let Some(o) = m.as_object_mut() {
                o.remove("reasoning_content");
                o.remove("reasoning");
            }
            with_images(&mut m);
            m
        })
        .collect()
}

/// A human's message carries an image as a line of its own,
/// `![image](file:///…/<16 hex>.png)`, written by fennec-lead's upload. The
/// file is read at send time, so the exchange on file keeps the line, not
/// megabytes. Only the human's messages: a line the model writes stays text.
fn with_images(m: &mut Value) {
    if m["role"] != "user" {
        return;
    }
    let Some(text) = m["content"].as_str() else {
        return;
    };
    let (mut words, mut images) = (Vec::new(), Vec::new());
    for line in text.lines() {
        match image_line(line) {
            Some(url) => images.push(json!({"type": "image_url", "image_url": {"url": url}})),
            None => words.push(line),
        }
    }
    if images.is_empty() {
        return;
    }
    images.insert(0, json!({"type": "text", "text": words.join("\n").trim()}));
    m["content"] = json!(images);
}

/// The image a line names, as a data URL; `None` if it names none, or one
/// that is not an upload (any image on disk could otherwise be sent).
fn image_line(line: &str) -> Option<String> {
    let path = line
        .trim()
        .strip_prefix("![image](file://")?
        .strip_suffix(')')?;
    let file = std::path::Path::new(path);
    let stem = file.file_stem()?.to_str()?;
    if stem.len() != 16 || !stem.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let kind = match file.extension()?.to_str()? {
        "png" => "png",
        "jpg" | "jpeg" => "jpeg",
        "gif" => "gif",
        "webp" => "webp",
        _ => return None,
    };
    let bytes = std::fs::read(file).ok()?;
    Some(format!("data:image/{kind};base64,{}", base64(&bytes)))
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
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

    struct Echo;
    impl Model for Echo {
        fn chat(&self, messages: &[Value], _: &Value) -> io::Result<Value> {
            Ok(
                json!({"role": "assistant", "content": format!("{} seen", messages.len()),
                      "usage": {"prompt_tokens": 10}}),
            )
        }
        fn label(&self) -> String {
            "echo".into()
        }
    }

    #[test]
    fn a_live_model_shows_the_exchange_so_far() {
        let path = std::env::temp_dir().join(format!("fennec-live-{}.json", std::process::id()));
        std::fs::write(&path, "left by the previous step").unwrap();
        let live = Live::new(&Echo, path.clone());
        assert!(!path.exists(), "a new step starts empty");
        let read =
            || -> Value { serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap() };
        let two = [json!({"role": "system"}), json!({"role": "user"})];
        let reply = live.chat(&two, &json!([])).unwrap();
        assert_eq!(
            reply["usage"]["prompt_tokens"], 10,
            "the caller still gets the usage"
        );
        let three = [two[0].clone(), two[1].clone(), reply];
        live.chat_quick(&three, &json!([])).unwrap();
        let now = read();
        assert_eq!(now["model"], "echo");
        assert_eq!(now["messages"].as_array().unwrap().len(), 4);
        assert_eq!(now["messages"][3]["content"], "3 seen");
        assert!(
            now["messages"][3]["usage"].is_null(),
            "usage is for the cost, not the messages"
        );
        assert_eq!(now["turns"].as_array().unwrap().len(), 2);
        assert_eq!(now["turns"][1]["think"], false);
        live.chat(&two, &json!([])).unwrap();
        assert_eq!(
            read()["turns"].as_array().unwrap().len(),
            1,
            "a new conversation starts over"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn reasoning_is_kept_but_never_sent_back() {
        let said = [
            json!({"role": "assistant", "content": "ok", "reasoning_content": "hmm", "reasoning": "hmm"}),
        ];
        let sent = outgoing(&said);
        assert_eq!(sent[0], json!({"role": "assistant", "content": "ok"}));
        assert_eq!(said[0]["reasoning_content"], "hmm");
    }

    #[test]
    fn an_uploaded_image_goes_as_an_image() {
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"M"), "TQ==");
        let file = std::env::temp_dir().join("00112233aabbccdd.png");
        std::fs::write(&file, b"Man").unwrap();
        let line = format!("![image](file://{})", file.display());
        let said = [
            json!({"role": "user", "content": format!("vois\n{line}")}),
            json!({"role": "assistant", "content": line}),
            json!({"role": "user", "content": "![image](file:///etc/passwd.png)"}),
        ];
        let sent = outgoing(&said);
        assert_eq!(
            sent[0]["content"],
            json!([{"type": "text", "text": "vois"},
                   {"type": "image_url", "image_url": {"url": "data:image/png;base64,TWFu"}}])
        );
        assert!(
            sent[1]["content"].is_string(),
            "the model's lines stay text"
        );
        assert!(sent[2]["content"].is_string(), "only uploads are read");
        std::fs::remove_file(file).unwrap();
    }

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

    #[test]
    fn the_effort_goes_with_a_reasoning_call_only() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut got = Vec::new();
                let mut chunk = [0u8; 4096];
                // Headers, then as many bytes as they announce.
                let body = loop {
                    let n = stream.read(&mut chunk).unwrap();
                    got.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&got).to_string();
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let len: usize = head
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length: ")
                                    .map(str::to_string)
                            })
                            .and_then(|l| l.trim().parse().ok())
                            .unwrap_or(0);
                        if body.len() >= len {
                            break body.to_string();
                        }
                    }
                };
                bodies.push(serde_json::from_str::<Value>(&body).unwrap());
                let reply = r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#;
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                )
                .unwrap();
            }
            bodies
        });
        let model = OpenAi {
            effort: "low".into(),
            ..OpenAi::new(&base, "m", None, 0.3)
        };
        let said = [json!({"role": "user", "content": "hi"})];
        model.chat(&said, &json!([])).unwrap();
        model.chat_quick(&said, &json!([])).unwrap();
        let bodies = server.join().unwrap();
        assert_eq!(bodies[0]["reasoning_effort"], "low");
        assert!(bodies[0]["chat_template_kwargs"].is_null());
        assert!(bodies[1]["reasoning_effort"].is_null(), "{}", bodies[1]);
        assert_eq!(bodies[1]["chat_template_kwargs"]["enable_thinking"], false);
    }
}
