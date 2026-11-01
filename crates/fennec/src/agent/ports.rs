//! What the agent loop needs from the outside world, as traits: the model,
//! the session log and the tools. Adapters implement them; the loop never
//! sees HTTP, SQL or processes.

use crate::domain::agent::CommandAccess;
use crate::domain::event::{DoneReason, Event, OutputStream, ToolCall, ToolResult};
use crate::domain::preferences::ReasoningEffort;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub use llm_providers::Delta;

pub struct Request<'a> {
    pub system_prompt: &'a str,
    pub messages: &'a [Value],
    pub tools: &'a Value,
    pub max_tokens: u32,
    /// Sent as `reasoning_effort` when set; `None` omits the field.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Checked on every streamed line: the stream ends with "Aborted".
    pub stop: &'a AtomicBool,
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

/// What the human's rules say about one tool call, and what it was about (the command,
/// the path, the tool name) for the log.
pub struct RuleCheck<'r> {
    pub decision: rules::Decision<'r>,
    pub subject: String,
}

/// The tools a model can call (adapter: `tools::LocalTools`, on the host).
/// `access` is what the calling agent's commands may do to the workdir.
pub trait Toolbox {
    /// OpenAI-compatible tool definitions.
    fn definitions(&self) -> Value;
    /// Runs one call; never fails, errors are results the model reads.
    fn execute(
        &self,
        name: &str,
        args: &Value,
        access: CommandAccess,
        stop: &AtomicBool,
        on_output: &mut dyn FnMut(OutputStream, &str),
    ) -> ToolResult;
    /// The human's rules on this call, decided before it runs; the default allows.
    fn check_rules(&self, _name: &str, _args: &Value) -> RuleCheck<'_> {
        RuleCheck {
            decision: rules::Decision::Allow(None),
            subject: String::new(),
        }
    }
}

/// What an agent node run in a child session reports back.
pub struct AgentRun {
    pub reason: DoneReason,
    /// the agent's last text
    pub text: String,
}

/// Child sessions for `fresh` agent nodes and parallel branches (adapter:
/// web; tests use a double). Sync: parallel branches call it from threads.
pub trait ChildSessions: Sync {
    /// Creates a child session of `parent_session_id`; returns its id.
    fn create(&self, parent_session_id: &str, title: &str) -> Result<String, String>;
    /// Sends `prompt` to agent `agent_id` in that child session and runs the
    /// loop to the end of its turn. Calling it again continues the same session.
    fn send(
        &self,
        child_session_id: &str,
        agent_id: &str,
        prompt: &str,
        stop: &AtomicBool,
    ) -> Result<AgentRun, String>;
}

/// How a workflow shell node ended.
pub struct CommandRun {
    /// None when it timed out, was stopped or could not start
    pub exit_code: Option<i32>,
    pub stopped: bool,
    /// stdout then stderr
    pub output: String,
}

/// Runs a workflow's shell nodes in the working directory (adapter:
/// tools::LocalTools, the process runner of run_command).
pub trait Commands: Sync {
    fn run(&self, command: &str, timeout: Duration, stop: &AtomicBool) -> CommandRun;
}
