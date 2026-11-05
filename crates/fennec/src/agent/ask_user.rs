//! The `ask_user` tool, owned by the loop: it pauses the turn to ask the
//! human a question. It has no in-memory promise machinery: fennec ends
//! the turn with `waiting_for_user` and the answer arrives as the next
//! turn's input, so a pending question survives a restart.

use crate::domain::event::{AskType, ChoiceOption, OptionTone};
use serde_json::{Value, json};

pub const NAME: &str = "ask_user";

/// The OpenAI tool definition.
pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Pause execution and ask the user a question. Use this when you need clarification or user input before proceeding.",
            "parameters": {
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to ask the user",
                    },
                    "type": {
                        "type": "string",
                        "enum": ["text", "confirm", "choice"],
                        "description": "Type of question (text, confirm, or choice)",
                    },
                    "options": {
                        "type": "array",
                        "description": "Options for choice-type questions. Each entry may be a plain string or an object {value, label, description?, tone?} (or legacy {label, description?}). Mark the refuse/cancel option with tone \"danger\" so it is shown in red. The server normalizes everything to {value, label, description?, tone?}.",
                        "items": {
                            "oneOf": [
                                { "type": "string" },
                                {
                                    "type": "object",
                                    "properties": {
                                        "value": { "type": "string" },
                                        "label": { "type": "string" },
                                        "description": { "type": "string" },
                                        "tone": {
                                            "type": "string",
                                            "enum": ["danger"],
                                            "description": "Render this option as a danger (red) action. Use it for the refuse/cancel option.",
                                        },
                                    },
                                    "required": ["label"],
                                },
                            ],
                        },
                    },
                },
                "required": ["question"],
            },
        },
    })
}

pub struct Question {
    pub question: String,
    pub ask_type: AskType,
    pub options: Option<Vec<ChoiceOption>>,
}

/// Validates and normalises the model's `ask_user` arguments. The error is
/// a result the model reads, so it must say what to fix.
pub fn parse(args: &Value) -> Result<Question, String> {
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            "ask_user requires a non-empty 'question' string. Provide the question to ask the user."
                .to_string()
        })?;
    let ask_type = args
        .get("type")
        .and_then(Value::as_str)
        .map(parse_ask_type)
        .unwrap_or(AskType::Text);
    let options = args.get("options").and_then(normalize_options);
    Ok(Question {
        question,
        ask_type,
        options,
    })
}

fn parse_ask_type(s: &str) -> AskType {
    match s {
        "confirm" => AskType::Confirm,
        "choice" => AskType::Choice,
        _ => AskType::Text,
    }
}

/// Coerce the model's input into the canonical `ChoiceOption[]` shape.
/// A string entry becomes `{value: s, label: s}`; an object requires a
/// non-empty string `label` (else it is skipped), `value` defaults to
/// `label`, `description` is kept when present. An empty or missing list
/// is `None`.
fn normalize_options(raw: &Value) -> Option<Vec<ChoiceOption>> {
    match raw {
        Value::Null => None,
        Value::String(s) => Some(vec![option_from_string(s)]),
        Value::Array(items) => {
            let mut out: Vec<ChoiceOption> = Vec::new();
            for item in items {
                match item {
                    Value::String(s) => out.push(option_from_string(s)),
                    Value::Object(obj) => {
                        if let Some(opt) = option_from_object(obj) {
                            out.push(opt);
                        }
                    }
                    _ => {}
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

fn option_from_string(s: &str) -> ChoiceOption {
    let trimmed = s.trim();
    ChoiceOption {
        value: trimmed.to_string(),
        label: trimmed.to_string(),
        description: None,
        tone: None,
    }
}

fn option_from_object(obj: &serde_json::Map<String, Value>) -> Option<ChoiceOption> {
    let label = obj.get("label")?.as_str()?;
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    let value = obj
        .get("value")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| label.to_string());
    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .filter(|d| !d.is_empty())
        .map(str::to_string);
    let tone = obj
        .get("tone")
        .and_then(Value::as_str)
        .filter(|t| *t == "danger")
        .map(|_| OptionTone::Danger);
    Some(ChoiceOption {
        value,
        label: label.to_string(),
        description,
        tone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_requires_a_non_empty_question() {
        assert!(parse(&json!({})).is_err());
        assert!(parse(&json!({"question": ""})).is_err());
        assert!(parse(&json!({"question": "   "})).is_err());
        assert!(parse(&json!({"question": 42})).is_err());
    }

    #[test]
    fn parse_reads_the_question() {
        let q = parse(&json!({"question": "Which backend?"})).unwrap();
        assert_eq!(q.question, "Which backend?");
        assert_eq!(q.ask_type, AskType::Text);
        assert!(q.options.is_none());
    }

    #[test]
    fn parse_defaults_type_to_text() {
        let q = parse(&json!({"question": "q"})).unwrap();
        assert_eq!(q.ask_type, AskType::Text);
        let q = parse(&json!({"question": "q", "type": "unknown"})).unwrap();
        assert_eq!(q.ask_type, AskType::Text);
    }

    #[test]
    fn parse_reads_type() {
        assert_eq!(
            parse(&json!({"question": "q", "type": "confirm"}))
                .unwrap()
                .ask_type,
            AskType::Confirm
        );
        assert_eq!(
            parse(&json!({"question": "q", "type": "choice"}))
                .unwrap()
                .ask_type,
            AskType::Choice
        );
    }

    #[test]
    fn parse_normalizes_string_options() {
        let q =
            parse(&json!({"question": "Pick:", "type": "choice", "options": ["A", "B"]})).unwrap();
        assert_eq!(
            q.options,
            Some(vec![
                ChoiceOption {
                    value: "A".into(),
                    label: "A".into(),
                    description: None,
                    tone: None,
                },
                ChoiceOption {
                    value: "B".into(),
                    label: "B".into(),
                    description: None,
                    tone: None,
                },
            ])
        );
    }

    #[test]
    fn parse_normalizes_object_options_preserving_description() {
        let q = parse(&json!({
            "question": "Pick:",
            "type": "choice",
            "options": [
                {"label": "Continuer", "description": "Reprendre"},
                {"label": "Annuler", "value": "stop", "description": "Stopper"},
            ],
        }))
        .unwrap();
        assert_eq!(
            q.options,
            Some(vec![
                ChoiceOption {
                    value: "Continuer".into(),
                    label: "Continuer".into(),
                    description: Some("Reprendre".into()),
                    tone: None,
                },
                ChoiceOption {
                    value: "stop".into(),
                    label: "Annuler".into(),
                    description: Some("Stopper".into()),
                    tone: None,
                },
            ])
        );
    }

    #[test]
    fn parse_reads_danger_tone_on_options() {
        let q = parse(&json!({
            "question": "Pick:",
            "type": "choice",
            "options": [
                {"label": "Accepter"},
                {"label": "Refuser", "tone": "danger"},
                {"label": "Indifferent", "tone": "loud"},
            ],
        }))
        .unwrap();
        assert_eq!(
            q.options,
            Some(vec![
                ChoiceOption {
                    value: "Accepter".into(),
                    label: "Accepter".into(),
                    description: None,
                    tone: None,
                },
                ChoiceOption {
                    value: "Refuser".into(),
                    label: "Refuser".into(),
                    description: None,
                    tone: Some(OptionTone::Danger),
                },
                ChoiceOption {
                    value: "Indifferent".into(),
                    label: "Indifferent".into(),
                    description: None,
                    tone: None,
                },
            ])
        );
    }

    #[test]
    fn parse_drops_malformed_option_entries() {
        let q = parse(&json!({
            "question": "Pick:",
            "type": "choice",
            "options": [null, 42, true, {"label": "OK"}, {"label": ""}, {"description": "no label"}, {"label": 123}, "legacy"],
        }))
        .unwrap();
        assert_eq!(
            q.options,
            Some(vec![
                ChoiceOption {
                    value: "OK".into(),
                    label: "OK".into(),
                    description: None,
                    tone: None,
                },
                ChoiceOption {
                    value: "legacy".into(),
                    label: "legacy".into(),
                    description: None,
                    tone: None,
                },
            ])
        );
    }

    #[test]
    fn parse_empty_or_missing_options_is_none() {
        assert!(
            parse(&json!({"question": "q", "options": []}))
                .unwrap()
                .options
                .is_none()
        );
        assert!(parse(&json!({"question": "q"})).unwrap().options.is_none());
        assert!(
            parse(&json!({"question": "q", "options": [null, 1]}))
                .unwrap()
                .options
                .is_none()
        );
    }

    #[test]
    fn definition_is_stable() {
        let d = definition();
        assert_eq!(d["function"]["name"], "ask_user");
        assert_eq!(d["function"]["parameters"]["required"], json!(["question"]));
        assert_eq!(
            d["function"]["parameters"]["properties"]["type"]["enum"],
            json!(["text", "confirm", "choice"])
        );
    }
}
