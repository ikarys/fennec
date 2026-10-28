//! The `record_decision` tool, owned by the loop and fennec's own: the agent
//! records a design decision (choice, reason, alternatives) so the human can
//! understand what was generated. Unlike `ask_user` it does not pause the turn.

use crate::domain::event::Event;
use serde_json::{Value, json};

pub const NAME: &str = "record_decision";

/// What the model reads back once the decision is in the log.
pub const RECORDED: &str = "Decision recorded.";

/// The OpenAI tool definition.
pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Record a significant design decision for the human reviewer: a choice between real alternatives (a library, a structure, a trade-off). Not for trivia. It does not pause the work.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Short name of the decision",
                    },
                    "choice": {
                        "type": "string",
                        "description": "What was decided",
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why this choice",
                    },
                    "alternatives": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "The options considered and not taken",
                    },
                },
                "required": ["title", "choice", "reason"],
            },
        },
    })
}

pub struct Decision {
    pub title: String,
    pub choice: String,
    pub reason: String,
    pub alternatives: Vec<String>,
}

fn required(args: &Value, field: &str) -> Result<String, String> {
    args.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("record_decision requires a non-empty '{field}' string."))
}

/// Validates the model's `record_decision` arguments. The error is a result
/// the model reads, so it must say what to fix.
pub fn parse(args: &Value) -> Result<Decision, String> {
    let alternatives = match args.get("alternatives") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::trim))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "'alternatives' must be an array of strings.".to_string())?
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        Some(_) => return Err("'alternatives' must be an array of strings.".to_string()),
    };
    Ok(Decision {
        title: required(args, "title")?,
        choice: required(args, "choice")?,
        reason: required(args, "reason")?,
        alternatives,
    })
}

pub fn event(call_id: &str, decision: Decision) -> Event {
    Event::DecisionRecorded {
        call_id: call_id.into(),
        title: decision.title,
        choice: decision.choice,
        reason: decision.reason,
        alternatives: decision.alternatives,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trims_and_reads_every_field() {
        let d = parse(&json!({
            "title": " Storage ",
            "choice": "SQLite",
            "reason": " one file ",
            "alternatives": ["Postgres", "  ", " JSON files "],
        }))
        .unwrap();
        assert_eq!(d.title, "Storage");
        assert_eq!(d.reason, "one file");
        assert_eq!(d.alternatives, ["Postgres", "JSON files"]);
    }

    #[test]
    fn alternatives_are_optional() {
        let d = parse(&json!({"title": "t", "choice": "c", "reason": "r"})).unwrap();
        assert!(d.alternatives.is_empty());
    }

    #[test]
    fn parse_names_the_missing_field() {
        let err = parse(&json!({"title": "t", "choice": " ", "reason": "r"}))
            .err()
            .unwrap();
        assert!(err.contains("'choice'"));
        assert!(parse(&json!({"title": "t", "choice": "c"})).is_err());
    }

    #[test]
    fn parse_rejects_malformed_alternatives() {
        let base =
            |alt: Value| json!({"title": "t", "choice": "c", "reason": "r", "alternatives": alt});
        assert!(parse(&base(json!("Postgres"))).is_err());
        assert!(parse(&base(json!([1]))).is_err());
    }
}
