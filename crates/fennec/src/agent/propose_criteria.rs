//! The `propose_criteria` tool, owned by the loop and fennec's own: the
//! builder proposes a corrected criteria list; as for `ask_user`, the turn
//! ends and the human's answer is the tool result.

use crate::domain::criteria::{ACCEPT_PROPOSAL, Criterion, REFUSE_PROPOSAL, proposal_question};
use crate::domain::event::{AskType, ChoiceOption, Event};
use serde_json::{Value, json};

pub const NAME: &str = "propose_criteria";

/// The OpenAI tool definition.
pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Propose a corrected list of acceptance criteria when one recorded criterion is wrong (its check cannot pass, or tests the wrong thing). It replaces the whole list, so include every criterion to keep. The human accepts or refuses; nothing changes until they accept.",
            "parameters": {
                "type": "object",
                "properties": {
                    "criteria": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "description": {
                                    "type": "string",
                                    "description": "What the criterion is",
                                },
                                "check": {
                                    "type": "string",
                                    "description": "Shell command that exits 0 once the criterion holds; omit when only a reader can judge it",
                                },
                            },
                            "required": ["description"],
                        },
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why the recorded criteria are wrong",
                    },
                },
                "required": ["criteria", "reason"],
            },
        },
    })
}

pub struct Proposal {
    pub criteria: Vec<Criterion>,
    pub reason: String,
}

/// Validates the model's `propose_criteria` arguments. The error is a result
/// the model reads, so it must say what to fix.
pub fn parse(args: &Value) -> Result<Proposal, String> {
    let raw = args
        .get("criteria")
        .and_then(Value::as_array)
        .ok_or_else(|| "propose_criteria requires a 'criteria' array.".to_string())?;
    if raw.is_empty() {
        return Err("propose_criteria requires at least one criterion.".to_string());
    }

    let mut criteria = Vec::with_capacity(raw.len());
    for (index, item) in raw.iter().enumerate() {
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                format!(
                    "criterion C{} needs a non-empty 'description' string.",
                    index + 1
                )
            })?;
        let check = item
            .get("check")
            .and_then(Value::as_str)
            .map(str::trim)
            .map(|check| check.trim_matches('`').trim())
            .filter(|check| !check.is_empty())
            .map(str::to_string);
        criteria.push(Criterion {
            id: format!("C{}", index + 1),
            description,
            check,
        });
    }

    let reason = args
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "propose_criteria requires a non-empty 'reason' string.".to_string())?;

    Ok(Proposal { criteria, reason })
}

/// The proposal, then the question the human answers: the list shown is the
/// list recorded, both built by fennec from the same arguments.
pub fn events(call_id: &str, proposal: Proposal) -> [Event; 2] {
    let question = proposal_question(&proposal.criteria, &proposal.reason);
    let option = |value: &str, label: &str| ChoiceOption {
        value: value.into(),
        label: label.into(),
        description: None,
    };
    [
        Event::CriteriaProposed {
            call_id: call_id.into(),
            criteria: proposal.criteria,
            reason: proposal.reason,
        },
        Event::ChatAskUser {
            call_id: call_id.into(),
            question,
            ask_type: AskType::Choice,
            options: Some(vec![
                option(ACCEPT_PROPOSAL, "Accepter"),
                option(REFUSE_PROPOSAL, "Refuser"),
            ]),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_reads_criteria_and_reason() {
        let p = parse(&json!({
            "criteria": [
                {"description": "builds", "check": "cargo build"},
                {"description": "docs", "check": "  `true`  "},
            ],
            "reason": "the old check cannot run",
        }))
        .unwrap();
        assert_eq!(
            p.criteria,
            vec![
                Criterion {
                    id: "C1".into(),
                    description: "builds".into(),
                    check: Some("cargo build".into()),
                },
                Criterion {
                    id: "C2".into(),
                    description: "docs".into(),
                    check: Some("true".into()),
                },
            ]
        );
        assert_eq!(p.reason, "the old check cannot run");
    }

    #[test]
    fn parse_rejects_an_empty_list() {
        assert!(parse(&json!({"criteria": [], "reason": "r"})).is_err());
        assert!(parse(&json!({"reason": "r"})).is_err());
    }

    #[test]
    fn parse_rejects_a_missing_reason() {
        assert!(parse(&json!({"criteria": [{"description": "one"}]})).is_err());
    }

    #[test]
    fn parse_drops_an_empty_check() {
        let p = parse(&json!({
            "criteria": [{"description": "one", "check": "` `"}],
            "reason": "r",
        }))
        .unwrap();
        assert_eq!(p.criteria[0].check, None);
    }
}
