//! Test doubles shared by the agent's test modules: a scripted model and an
//! in-memory log, so each test module keeps one copy.

use crate::agent::ports::{Completion, Delta, EventLog, Model, Request};
use crate::domain::event::{Event, ToolCall};
use std::cell::RefCell;

/// Plays back one scripted reply per call, like openfox's mock LLM.
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
