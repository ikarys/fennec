//! Test doubles shared by the agent's test modules: a scripted model and an
//! in-memory log, so each test module keeps one copy.

use crate::agent::ports::{Completion, Delta, EventLog, Model, Request, RuleCheck, Toolbox};
use crate::agent::turn::Turn;
use crate::domain::agent::{AgentDefinition, AgentOrigin, CommandAccess};
use crate::domain::event::{Event, OutputStream, ToolCall, ToolResult};
use crate::tools::LocalTools;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// Plays back one scripted reply per call.
pub struct Scripted(pub RefCell<Vec<Result<Completion, String>>>);

impl Model for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn stream(&self, _: &Request, on: &mut dyn FnMut(Delta)) -> Result<Completion, String> {
        let next = self.0.borrow_mut().remove(0);
        if let Ok(c) = &next
            && !c.content.is_empty()
        {
            on(Delta::Text(c.content.clone()));
        }
        next
    }
}

/// An in-memory log: the loop's tests do not need SQLite.
#[derive(Default)]
pub struct MemoryLog(pub RefCell<Vec<Event>>);

impl EventLog for MemoryLog {
    fn append(&self, _: &str, event: &Event) -> Result<u64, String> {
        self.0.borrow_mut().push(event.clone());
        Ok(self.0.borrow().len() as u64)
    }

    fn events(&self, _: &str) -> Result<Vec<Event>, String> {
        Ok(self.0.borrow().clone())
    }
}

/// A scripted reply: `content` plus the tool calls it makes.
pub fn reply(content: &str, calls: Vec<ToolCall>) -> Result<Completion, String> {
    Ok(Completion {
        content: content.into(),
        finish_reason: if calls.is_empty() {
            "stop"
        } else {
            "tool_calls"
        }
        .into(),
        tool_calls: calls,
        ..Completion::default()
    })
}

/// A top-level builder agent that allows every tool used by the agent tests.
pub fn builder() -> Vec<AgentDefinition> {
    vec![AgentDefinition {
        id: "builder".into(),
        phase: None,
        name: "Builder".into(),
        description: String::new(),
        subagent: false,
        composer: true,
        allowed_tools: vec![
            "read_file".into(),
            "write_file".into(),
            "edit_file".into(),
            "run_command".into(),
            "ask_user".into(),
            "request_waiver".into(),
            "propose_criteria".into(),
            "record_decision".into(),
            "load_skill".into(),
        ],
        commands: CommandAccess::ReadWrite,
        writes: crate::domain::agent::WriteScope::Any,
        reasoning_effort: None,
        color: None,
        workflow: None,
        prompt: "Build.".into(),
        origin: AgentOrigin::Default,
        path: None,
    }]
}

/// A top-level plan agent: read-only by construction.
pub fn plan() -> AgentDefinition {
    AgentDefinition {
        id: "plan".into(),
        phase: None,
        name: "Plan".into(),
        description: String::new(),
        subagent: false,
        composer: true,
        allowed_tools: vec!["read_file".into(), "ask_user".into(), "load_skill".into()],
        commands: CommandAccess::ReadOnly,
        writes: crate::domain::agent::WriteScope::Any,
        reasoning_effort: None,
        color: None,
        workflow: None,
        prompt: "Plan.".into(),
        origin: AgentOrigin::Default,
        path: None,
    }
}

/// A toolbox for the agent loop tests: `run_command` always succeeds with the
/// marker `run_command` prints, so preflight can classify it without a shell.
pub struct FakeToolbox;

impl Toolbox for FakeToolbox {
    fn check_rules(&self, _name: &str, _args: &Value) -> RuleCheck<'_> {
        RuleCheck::allowed()
    }
    fn definitions(&self) -> Value {
        json!([])
    }

    fn execute(
        &self,
        name: &str,
        args: &Value,
        _access: CommandAccess,
        _stop: &AtomicBool,
        _on_output: &mut dyn FnMut(OutputStream, &str),
    ) -> ToolResult {
        if name == "run_command" {
            let command = args
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default();
            ToolResult::success(format!("$ {command}\n[Exit code: 0]"), false)
        } else {
            ToolResult::success("ok".into(), false)
        }
    }
}

/// Owns a turn's dependencies so a test can run several turns on the same
/// session and inspect the log between them.
pub struct Harness<'a> {
    pub log: MemoryLog,
    pub model: Box<dyn Model>,
    pub tools: LocalTools,
    pub stop: &'a AtomicBool,
    pub workdir: PathBuf,
    pub on_event: Box<dyn FnMut(&Event) + 'a>,
    pub agents: Vec<AgentDefinition>,
}

impl<'a> Harness<'a> {
    pub fn new(model: Box<dyn Model>, workdir: PathBuf, stop: &'a AtomicBool) -> Self {
        Harness {
            log: MemoryLog::default(),
            model,
            tools: LocalTools::new(workdir.clone()).with_rules(rules::Rules::default()),
            stop,
            workdir,
            on_event: Box::new(|_: &Event| {}),
            agents: builder(),
        }
    }

    /// The same harness with the human's rules.
    pub fn with_rules(mut self, rules: rules::Rules) -> Self {
        self.tools = LocalTools::new(self.workdir.clone()).with_rules(rules);
        self
    }

    pub fn turn(&mut self) -> Turn<'_> {
        self.turn_with(false)
    }

    pub fn turn_autonomous(&mut self) -> Turn<'_> {
        self.turn_with(true)
    }

    fn turn_with(&mut self, autonomous: bool) -> Turn<'_> {
        Turn {
            model: &*self.model,
            log: &self.log,
            tools: &self.tools,
            session_id: "s",
            workdir: &self.workdir,
            task_sheet: None,
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &self.agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            session_effort: None,
            autonomous,
            lang: crate::domain::lang::Lang::Fr,
            preflight_sent_back: false,
            stop: self.stop,
            on_event: &mut self.on_event,
            children: None,
        }
    }
}

/// A fresh empty directory under the system temp dir.
pub fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fennec-test-{}", crate::domain::id::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
