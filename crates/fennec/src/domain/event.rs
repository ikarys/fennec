//! The session log's vocabulary: port of openfox `events/types.ts`, the
//! subset fennec produces. Wire names match openfox.

use super::attachment::Attachment;
use super::criteria::{Criterion, CriterionStatus};
use super::stats::MessageStats;
use super::workflow::definition::Workflow;
use super::workflow::run::{RunOutcome, Via};
use super::workflow::stats::RunStats;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ToolResultMetadata>,
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
            metadata: None,
        }
    }

    pub fn failure(error: String) -> ToolResult {
        ToolResult {
            success: false,
            output: None,
            error: Some(error),
            duration_ms: 0,
            truncated: false,
            metadata: None,
        }
    }

    pub fn image(output: String, metadata: ToolResultMetadata) -> ToolResult {
        ToolResult {
            success: true,
            output: Some(output),
            error: None,
            duration_ms: 0,
            truncated: false,
            metadata: Some(metadata),
        }
    }
}

/// openfox tool result `metadata` for an image read_file returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultMetadata {
    pub mime_type: String,
    pub data_url: String,
    pub path: String,
    pub size: u64,
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

/// openfox message `metadata` for an agent reminder: `{type: "agent", name, color, kind}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageMetadata {
    #[serde(rename = "type")]
    pub source: MetadataSource,
    pub name: String,
    pub color: String,
    pub kind: ReminderKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetadataSource {
    Agent,
}

/// openfox writes `definition` (the full agent prompt) or `reminder` (a short
/// nudge each turn); fennec only re-sends the definition when the agent changes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReminderKind {
    Definition,
    Reminder,
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
        /// Set on the agent reminders the loop injects (openfox `metadata`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<MessageMetadata>,
        /// openfox `attachments`: images the human sent with the message.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<Attachment>,
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
    /// Marks the cut where a new context window started from a summary.
    /// openfox also carries `closedWindowId`/`newWindowId`; fennec has no
    /// window ids, so the cut is just this event in the log.
    #[serde(rename = "context.compacted", rename_all = "camelCase")]
    ContextCompacted {
        before_tokens: u64,
        after_tokens: u64,
        summary: String,
    },
    #[serde(rename = "running.changed", rename_all = "camelCase")]
    RunningChanged { is_running: bool },
    /// The human switched the session's agent (openfox `mode.changed`).
    #[serde(rename = "mode.changed", rename_all = "camelCase")]
    ModeChanged {
        mode: String,
        auto: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// fennec's own (openfox deletes the events instead): the human replayed
    /// `message_id`; the model's view drops that message and everything after
    /// it, the log keeps them.
    #[serde(rename = "session.rewound", rename_all = "camelCase")]
    SessionRewound { message_id: String },
    /// fennec's own: a workflow run starts; the definition is frozen for the run.
    #[serde(rename = "workflow.started", rename_all = "camelCase")]
    WorkflowStarted {
        run_id: String,
        workflow: Workflow,
        task: String,
        #[serde(default)]
        step_by_step: bool,
    },
    #[serde(rename = "workflow.node.started", rename_all = "camelCase")]
    WorkflowNodeStarted {
        run_id: String,
        node_id: String,
        pass: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via: Option<Via>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        child_session_id: Option<String>,
    },
    #[serde(rename = "workflow.node.finished", rename_all = "camelCase")]
    WorkflowNodeFinished {
        run_id: String,
        node_id: String,
        port: String,
        output: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        comment: Option<String>,
        /// The node's working time, measured by fennec; None for a human
        /// node, whose time is the human's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    #[serde(rename = "workflow.paused", rename_all = "camelCase")]
    WorkflowPaused {
        run_id: String,
        from: String,
        port: String,
        to: String,
    },
    #[serde(rename = "workflow.resumed", rename_all = "camelCase")]
    WorkflowResumed {
        run_id: String,
        #[serde(default)]
        comment: String,
    },
    #[serde(rename = "workflow.finished", rename_all = "camelCase")]
    WorkflowFinished {
        run_id: String,
        outcome: RunOutcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stats: Option<RunStats>,
    },
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
    #[serde(rename = "criteria.set", rename_all = "camelCase")]
    CriteriaSet { criteria: Vec<Criterion> },
    #[serde(rename = "criterion.updated", rename_all = "camelCase")]
    CriterionUpdated {
        criterion_id: String,
        status: CriterionStatus,
    },
    /// The human integrated the session's branch (fennec's own event).
    #[serde(rename = "integration.applied", rename_all = "camelCase")]
    IntegrationApplied {
        branch: String,
        base: String,
        commits: Vec<String>,
        head: String,
        acknowledged: Vec<String>,
    },
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
            Event::ContextCompacted { .. } => "context.compacted",
            Event::RunningChanged { .. } => "running.changed",
            Event::ModeChanged { .. } => "mode.changed",
            Event::SessionRewound { .. } => "session.rewound",
            Event::WorkflowStarted { .. } => "workflow.started",
            Event::WorkflowNodeStarted { .. } => "workflow.node.started",
            Event::WorkflowNodeFinished { .. } => "workflow.node.finished",
            Event::WorkflowPaused { .. } => "workflow.paused",
            Event::WorkflowResumed { .. } => "workflow.resumed",
            Event::WorkflowFinished { .. } => "workflow.finished",
            Event::ChatDone { .. } => "chat.done",
            Event::ChatError { .. } => "chat.error",
            Event::ChatAskUser { .. } => "chat.ask_user",
            Event::TurnSnapshot { .. } => "turn.snapshot",
            Event::CriteriaSet { .. } => "criteria.set",
            Event::CriterionUpdated { .. } => "criterion.updated",
            Event::IntegrationApplied { .. } => "integration.applied",
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mode_changed_round_trips() {
        let wire = json!({
            "type": "mode.changed",
            "data": {
                "mode": "builder",
                "auto": false
            }
        });

        let event: Event = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(
            &event,
            Event::ModeChanged {
                mode,
                auto: false,
                reason: None
            } if mode == "builder"
        ));

        assert_eq!(serde_json::to_value(&event).unwrap(), wire);
    }

    #[test]
    fn session_rewound_round_trips() {
        let wire = json!({
            "type": "session.rewound",
            "data": {
                "messageId": "m"
            }
        });

        let event: Event = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(
            &event,
            Event::SessionRewound { message_id } if message_id == "m"
        ));

        assert_eq!(serde_json::to_value(&event).unwrap(), wire);
    }

    #[test]
    fn message_start_metadata_round_trips() {
        let wire = json!({
            "type": "message.start",
            "data": {
                "messageId": "m",
                "role": "user",
                "content": "reminder",
                "isSystemGenerated": true,
                "metadata": {
                    "type": "agent",
                    "name": "Builder",
                    "color": "#3b82f6",
                    "kind": "definition"
                }
            }
        });

        let event: Event = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(
            &event,
            Event::MessageStart {
                message_id,
                role: Role::User,
                content: Some(content),
                is_system_generated: true,
                metadata: Some(MessageMetadata {
                    source: MetadataSource::Agent,
                    name,
                    color,
                    kind: ReminderKind::Definition,
                }),
                ..
            } if message_id == "m"
                && content == "reminder"
                && name == "Builder"
                && color == "#3b82f6"
        ));

        assert_eq!(serde_json::to_value(&event).unwrap(), wire);
    }

    #[test]
    fn message_start_attachments_round_trip_and_omit_when_empty() {
        let with = Event::MessageStart {
            message_id: "m".into(),
            role: Role::User,
            content: Some("hi".into()),
            is_system_generated: false,
            metadata: None,
            attachments: vec![Attachment {
                id: "a".into(),
                filename: "a.png".into(),
                mime_type: "image/png".into(),
                size: 1,
                data: "data:image/png;base64,AAAA".into(),
            }],
        };
        let wire = serde_json::to_value(&with).unwrap();
        assert_eq!(wire["data"]["attachments"][0]["mimeType"], "image/png");
        assert_eq!(serde_json::from_value::<Event>(wire).unwrap(), with);

        let without = Event::MessageStart {
            message_id: "m".into(),
            role: Role::User,
            content: Some("hi".into()),
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        };
        let wire = serde_json::to_value(&without).unwrap();
        assert!(wire["data"].get("attachments").is_none());
        assert_eq!(serde_json::from_value::<Event>(wire).unwrap(), without);
    }

    #[test]
    fn workflow_node_started_with_via_round_trips() {
        let wire = json!({
            "type": "workflow.node.started",
            "data": {
                "runId": "r",
                "nodeId": "n",
                "pass": 1,
                "via": {
                    "from": "f",
                    "port": "done"
                }
            }
        });

        let event: Event = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(
            &event,
            Event::WorkflowNodeStarted {
                run_id,
                node_id,
                pass: 1,
                via: Some(via),
                child_session_id: None,
            } if run_id == "r"
                && node_id == "n"
                && via.from == "f"
                && via.port == "done"
        ));

        assert_eq!(serde_json::to_value(&event).unwrap(), wire);
    }
}
