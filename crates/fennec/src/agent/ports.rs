//! What the agent loop needs from the outside world, as traits: the model,
//! the session log and the tools. Adapters implement them; the loop never
//! sees HTTP, SQL or processes.

use crate::domain::event::{Event, OutputStream, ToolCall, ToolResult};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

pub struct Request<'a> {
    pub system_prompt: &'a str,
    pub messages: &'a [Value],
    pub tools: &'a Value,
    pub max_tokens: u32,
    /// Checked on every streamed line: the stream ends with "Aborted".
    pub stop: &'a AtomicBool,
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
    /// The part of the prompt served from the server's cache.
    pub cached_tokens: u64,
    pub completion_tokens: u64,
}

/// The language model behind the loop (adapter: `llm::OpenAiClient`; tests
/// use a scripted model).
pub trait Model {
    fn name(&self) -> &str;
    fn stream(&self, req: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String>;
}

/// The session's append-only event log (adapter: `store::SqliteStore`).
pub trait EventLog {
    /// Appends `event`, returns its sequence number in the session.
    fn append(&self, session_id: &str, event: &Event) -> Result<u64, String>;
    fn events(&self, session_id: &str) -> Result<Vec<Event>, String>;
}

/// The tools a model can call (adapter: `tools::LocalTools`, on the host).
pub trait Toolbox {
    /// OpenAI-compatible tool definitions.
    fn definitions(&self) -> Value;
    /// Runs one call; never fails, errors are results the model reads.
    fn execute(
        &self,
        name: &str,
        args: &Value,
        stop: &AtomicBool,
        on_output: &mut dyn FnMut(OutputStream, &str),
    ) -> ToolResult;
}
