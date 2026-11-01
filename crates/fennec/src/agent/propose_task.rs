//! The `propose_task` tool, owned by the loop and fennec's own: the agent
//! proposes a backlog task for the human to accept. Unlike `ask_user` it does
//! not pause the turn.

use crate::domain::event::Event;
use serde_json::{Value, json};

pub const NAME: &str = "propose_task";

/// What the model reads back once the proposal is in the log.
pub const PROPOSED: &str =
    "Task proposed to the human; it joins the backlog only if they accept it.";

/// The OpenAI tool definition.
pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Propose a task for the project's backlog: one self-contained piece of work a later session can plan and build on its own (what to do, why, and how to tell it is done). The human decides whether it joins the backlog. It does not pause the work.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Short imperative title of the task",
                    },
                    "text": {
                        "type": "string",
                        "description": "What to do, why, and how to tell it is done",
                    },
                },
                "required": ["title", "text"],
            },
        },
    })
}

pub struct TaskProposal {
    pub title: String,
    pub text: String,
}

fn required(args: &Value, field: &str) -> Result<String, String> {
    args.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("propose_task requires a non-empty '{field}' string."))
}

/// Validates the model's `propose_task` arguments. The error is a result
/// the model reads, so it must say what to fix.
pub fn parse(args: &Value) -> Result<TaskProposal, String> {
    let title = required(args, "title")?;
    if title.chars().count() > 120 {
        return Err("'title' must be at most 120 characters.".to_string());
    }
    let text = required(args, "text")?;
    Ok(TaskProposal { title, text })
}

pub fn event(call_id: &str, proposal: TaskProposal) -> Event {
    Event::TaskProposed {
        call_id: call_id.into(),
        title: proposal.title,
        text: proposal.text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trims_fields() {
        let p = parse(&json!({"title": " Title ", "text": " Body "})).unwrap();
        assert_eq!(p.title, "Title");
        assert_eq!(p.text, "Body");
    }

    #[test]
    fn parse_names_the_missing_field() {
        let err = parse(&json!({"title": "t"})).err().unwrap();
        assert!(err.contains("'text'"));
        let err = parse(&json!({"text": "x"})).err().unwrap();
        assert!(err.contains("'title'"));
        assert!(parse(&json!({"title": " ", "text": "x"})).is_err());
    }

    #[test]
    fn parse_rejects_a_title_over_120_chars() {
        let long = "a".repeat(121);
        let err = parse(&json!({"title": long, "text": "x"})).err().unwrap();
        assert!(err.contains("120"));
        let ok = "a".repeat(120);
        assert!(parse(&json!({"title": ok, "text": "x"})).is_ok());
    }
}
