//! The session log's vocabulary: port of openfox `events/types.ts`, the
//! subset fennec produces. Wire names match openfox.

use super::stats::MessageStats;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

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

impl ToolResult {
    /// `duration_ms` is set by whoever times the call.
    pub fn success(output: String, truncated: bool) -> ToolResult {
        ToolResult {
            success: true,
            output: Some(output),
            error: None,
            duration_ms: 0,
            truncated,
        }
    }

    pub fn failure(error: String) -> ToolResult {
        ToolResult {
            success: false,
            output: None,
            error: Some(error),
            duration_ms: 0,
            truncated: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// How a turn ended (openfox `chat.done` reasons fennec produces).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoneReason {
    Complete,
    Stopped,
    Error,
    Truncated,
    #[serde(rename = "waiting_for_user")]
    WaitingForUser,
}

/// The kind of question `ask_user` asks (openfox `type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AskType {
    #[default]
    Text,
    Confirm,
    Choice,
}

/// One option of a `choice` question (openfox `ChoiceOption`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceOption {
    pub value: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
        role: Role,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stats: Option<MessageStats>,
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
        stream: OutputStream,
        content: String,
    },
    #[serde(rename = "tool.result", rename_all = "camelCase")]
    ToolResult {
        message_id: String,
        tool_call_id: String,
        result: ToolResult,
    },
    #[serde(rename = "context.state", rename_all = "camelCase")]
    ContextState {
        current_tokens: u64,
        max_tokens: u64,
    },
    #[serde(rename = "running.changed", rename_all = "camelCase")]
    RunningChanged { is_running: bool },
    #[serde(rename = "chat.done", rename_all = "camelCase")]
    ChatDone {
        message_id: String,
        reason: DoneReason,
    },
    #[serde(rename = "chat.error", rename_all = "camelCase")]
    ChatError { error: String, recoverable: bool },
    /// The model asked the human a question; the turn ends and the answer
    /// arrives as the next turn's input.
    #[serde(rename = "chat.ask_user", rename_all = "camelCase")]
    ChatAskUser {
        call_id: String,
        question: String,
        #[serde(rename = "type", default)]
        ask_type: AskType,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<Vec<ChoiceOption>>,
    },
    /// openfox keeps more in a snapshot; fennec only needs the system
    /// prompt, frozen for the session so the server's prompt cache holds.
    #[serde(rename = "turn.snapshot", rename_all = "camelCase")]
    TurnSnapshot { cached_system_prompt: String },
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
            Event::ContextState { .. } => "context.state",
            Event::RunningChanged { .. } => "running.changed",
            Event::ChatDone { .. } => "chat.done",
            Event::ChatError { .. } => "chat.error",
            Event::ChatAskUser { .. } => "chat.ask_user",
            Event::TurnSnapshot { .. } => "turn.snapshot",
        }
    }
}

/// Wire names, for display.
impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        })
    }
}

impl fmt::Display for DoneReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            DoneReason::Complete => "complete",
            DoneReason::Stopped => "stopped",
            DoneReason::Error => "error",
            DoneReason::Truncated => "truncated",
            DoneReason::WaitingForUser => "waiting_for_user",
        })
    }
}
