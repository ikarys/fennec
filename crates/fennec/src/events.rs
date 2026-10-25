//! Port of openfox `src/server/events/{types,store,fold-messages}.ts`: the
//! event log is the single source of truth; the conversation sent to the
//! model is folded from it.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
    pub truncated: bool,
}

/// The subset of openfox's `TurnEvent` union the agent loop needs. Wire
/// names and field names match openfox so stored payloads stay comparable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    #[serde(rename = "session.initialized", rename_all = "camelCase")]
    SessionInitialized { workdir: String },
    #[serde(rename = "message.start", rename_all = "camelCase")]
    MessageStart {
        message_id: String,
        role: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_system_generated: bool,
    },
    #[serde(rename = "message.delta", rename_all = "camelCase")]
    MessageDelta { message_id: String, content: String },
    #[serde(rename = "message.thinking", rename_all = "camelCase")]
    MessageThinking { message_id: String, content: String },
    #[serde(rename = "message.done", rename_all = "camelCase")]
    MessageDone {
        message_id: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        partial: bool,
    },
    #[serde(rename = "tool.call", rename_all = "camelCase")]
    ToolCall {
        message_id: String,
        tool_call: ToolCall,
    },
    #[serde(rename = "tool.output", rename_all = "camelCase")]
    ToolOutput {
        message_id: String,
        tool_call_id: String,
        stream: String,
        content: String,
    },
    #[serde(rename = "tool.result", rename_all = "camelCase")]
    ToolResult {
        message_id: String,
        tool_call_id: String,
        result: ToolResult,
    },
    #[serde(rename = "chat.done", rename_all = "camelCase")]
    ChatDone { message_id: String, reason: String },
    #[serde(rename = "chat.error", rename_all = "camelCase")]
    ChatError { error: String, recoverable: bool },
}

impl Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Event::SessionInitialized { .. } => "session.initialized",
            Event::MessageStart { .. } => "message.start",
            Event::MessageDelta { .. } => "message.delta",
            Event::MessageThinking { .. } => "message.thinking",
            Event::MessageDone { .. } => "message.done",
            Event::ToolCall { .. } => "tool.call",
            Event::ToolOutput { .. } => "tool.output",
            Event::ToolResult { .. } => "tool.result",
            Event::ChatDone { .. } => "chat.done",
            Event::ChatError { .. } => "chat.error",
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Unique enough for ids local to one machine; avoids a uuid dependency.
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{:x}-{n:x}", std::process::id())
}

/// SQLite event store, same `sessions`/`events` schema as openfox (columns
/// fennec does not use yet are left out).
pub struct Store {
    db: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Store> {
        Store::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> rusqlite::Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> rusqlite::Result<Store> {
        db.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS sessions (
               id TEXT PRIMARY KEY,
               workdir TEXT NOT NULL,
               title TEXT,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS events (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               session_id TEXT NOT NULL,
               seq INTEGER NOT NULL,
               timestamp INTEGER NOT NULL,
               event_type TEXT NOT NULL,
               payload TEXT NOT NULL,
               UNIQUE(session_id, seq),
               FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_events_session_seq ON events(session_id, seq);",
        )?;
        Ok(Store { db })
    }

    pub fn create_session(&self, workdir: &str, title: &str) -> rusqlite::Result<String> {
        let id = new_id();
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT INTO sessions (id, workdir, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, workdir, title, now],
        )?;
        self.append(
            &id,
            &Event::SessionInitialized {
                workdir: workdir.to_string(),
            },
        )?;
        Ok(id)
    }

    pub fn append(&self, session_id: &str, event: &Event) -> rusqlite::Result<u64> {
        let payload = serde_json::to_string(event).expect("events always serialize");
        self.db.query_row(
            "INSERT INTO events (session_id, seq, timestamp, event_type, payload)
             VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM events WHERE session_id = ?1), ?2, ?3, ?4)
             RETURNING seq",
            params![session_id, now_ms() as i64, event.kind(), payload],
            |row| row.get::<_, i64>(0).map(|s| s as u64),
        )
    }

    pub fn events(&self, session_id: &str) -> rusqlite::Result<Vec<Event>> {
        let mut stmt = self
            .db
            .prepare("SELECT payload FROM events WHERE session_id = ?1 ORDER BY seq")?;
        let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for payload in rows {
            // A payload this version cannot read is skipped, not fatal: the
            // log may hold event types ported later.
            if let Ok(event) = serde_json::from_str(&payload?) {
                out.push(event);
            }
        }
        Ok(out)
    }

    /// Sessions, newest first: (id, title, workdir).
    pub fn sessions(&self) -> rusqlite::Result<Vec<(String, String, String)>> {
        let mut stmt = self.db.prepare(
            "SELECT id, COALESCE(title, ''), workdir FROM sessions ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect()
    }
}

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

/// Folds the log into OpenAI chat messages (without the system prompt).
pub fn fold_messages(events: &[Event]) -> Vec<Value> {
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
                ..
            } => {
                index.push((message_id.clone(), messages.len()));
                messages
                    .push(json!({"role": role, "content": content.clone().unwrap_or_default()}));
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
            } => messages.push(json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": render_tool_result(result),
            })),
            _ => {}
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_round_trip_through_the_store_in_order() {
        let store = Store::in_memory().unwrap();
        let sid = store.create_session("/w", "t").unwrap();
        let ev = Event::MessageDelta {
            message_id: "m".into(),
            content: "hi".into(),
        };
        assert_eq!(store.append(&sid, &ev).unwrap(), 2);
        let all = store.events(&sid).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1], ev);
        let wire = serde_json::to_value(&ev).unwrap();
        assert_eq!(wire["type"], "message.delta");
        assert_eq!(wire["data"]["messageId"], "m");
    }

    #[test]
    fn fold_rebuilds_assistant_tool_calls_and_tool_results() {
        let start = |id: &str, role: &str, content: Option<&str>| Event::MessageStart {
            message_id: id.into(),
            role: role.into(),
            content: content.map(Into::into),
            is_system_generated: false,
        };
        let events = vec![
            start("u", "user", Some("do it")),
            start("a", "assistant", None),
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
                },
            },
        ];
        let msgs = fold_messages(&events);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["content"], "ok");
        assert_eq!(
            msgs[1]["tool_calls"][0]["function"]["arguments"],
            r#"{"path":"x"}"#
        );
        assert_eq!(
            msgs[2],
            json!({"role": "tool", "tool_call_id": "c1", "content": "Error: nope"})
        );
    }
}
