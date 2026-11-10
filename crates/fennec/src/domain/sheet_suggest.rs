//! Prompt and reply parsing for suggesting one task sheet field.
//!
//! The model proposes a draft; the human accepts or rejects it.

use crate::domain::lang::Lang;
use backlog::{SheetField, TaskSheet};
use serde::Serialize;

pub const MAX_AGENTS_MD_CHARS: usize = 8_000;
pub const MAX_ATTACHMENT_CHARS: usize = 8_000;
pub const MAX_REPLY_CHARS: usize = 4_000;
pub const MAX_CRITERIA: usize = 20;

/// A model's draft: `{text}` for a text field, `{criteria}` for done when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Suggestion {
    Text(String),
    Criteria(Vec<String>),
}

/// The help line a field shows when empty, and the model reads.
pub fn help(field: SheetField) -> &'static str {
    match field {
        SheetField::Objective => {
            "What must be true once the task is done, in one or two sentences."
        }
        SheetField::Context => {
            "What the agent must know first: where it happens, why, what exists already."
        }
        SheetField::Constraints => {
            "Rules the change must respect: compatibility, performance, style, dependencies."
        }
        SheetField::OutOfScope => "What the task must not touch or do.",
        SheetField::Deliverable => {
            "What is handed over at the end: files, endpoint, screen, documentation."
        }
        SheetField::Approach => "The steps to get there, in order.",
        SheetField::DoneWhen => {
            "Verifiable criteria, one per line, each one observable without reading the code."
        }
    }
}

/// The model-facing prompt for one field.
pub fn prompt(
    idea_text: &str,
    text_attachments: &[(String, String)],
    sheet: &TaskSheet,
    field: SheetField,
    agents_md: Option<&str>,
    lang: Lang,
) -> String {
    let mut out = String::new();

    out.push_str(
        "You help a human fill one field of a task sheet for a coding agent. Write the field ",
    );
    out.push_str(field_name(field));
    out.push_str(": ");
    out.push_str(help(field));
    out.push('\n');
    out.push_str(lang.pick("Write in English.", "Write in French."));
    out.push('\n');

    if field == SheetField::DoneWhen {
        out.push_str(
            "Reply with 2 to 6 criteria, one per line, as a plain list. Never give a command or a check: a check is only trusted once fennec runs it.",
        );
    } else {
        out.push_str("Reply with only the field's text, no heading, no preamble.");
    }
    out.push('\n');

    out.push_str("The idea:\n");
    out.push_str(idea_text);

    for (filename, content) in text_attachments {
        out.push_str("\n\nAttached file `");
        out.push_str(filename);
        out.push_str("`:\n");
        out.push_str(&truncate_chars(content, MAX_ATTACHMENT_CHARS));
    }

    out.push_str("\n\nThe task sheet so far:\n");
    out.push_str(&sheet.to_markdown("Task"));

    if let Some(agents_md) = agents_md
        && !agents_md.trim().is_empty()
    {
        out.push_str("\n\nThe project's AGENTS.md (its rules):\n");
        out.push_str(&truncate_chars(agents_md, MAX_AGENTS_MD_CHARS));
    }

    out
}

/// The model's reply as a draft; criteria never carry a check.
pub fn parse_reply(field: SheetField, reply: &str) -> Result<Suggestion, String> {
    let lines: Vec<&str> = reply
        .trim()
        .lines()
        .filter(|line| !line.trim().starts_with("```"))
        .collect();

    if field == SheetField::DoneWhen {
        let mut criteria = Vec::new();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let mut text = line.trim();
            if let Some(rest) = text.strip_prefix("- [ ] ") {
                text = rest;
            } else if let Some(rest) = text.strip_prefix("- [x] ") {
                text = rest;
            } else if let Some(rest) = text.strip_prefix("- ") {
                text = rest;
            } else if let Some(rest) = text.strip_prefix("* ") {
                text = rest;
            } else if let Some(rest) = text.strip_prefix("• ") {
                text = rest;
            } else if let Some(rest) = strip_numbered(text) {
                text = rest;
            }

            if let Some(pos) = text.find("— check:") {
                text = &text[..pos];
            } else if let Some(pos) = text.find(" - check:") {
                text = &text[..pos];
            } else if let Some(pos) = text.find("check:") {
                text = &text[..pos];
            }

            let text = text.trim_end_matches([' ', '-', '—']);
            if text.is_empty() {
                continue;
            }

            criteria.push(truncate_chars(text, 500));
            if criteria.len() == MAX_CRITERIA {
                break;
            }
        }

        if criteria.is_empty() {
            Err("the model returned nothing".to_string())
        } else {
            Ok(Suggestion::Criteria(criteria))
        }
    } else {
        let text = lines
            .iter()
            .map(|line| {
                let indent = line.len() - line.trim_start().len();
                if let Some(rest) = line.trim_start().strip_prefix("## ") {
                    format!("{}### {rest}", &line[..indent])
                } else {
                    (*line).to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let text = truncate_chars(&text, MAX_REPLY_CHARS);
        if text.trim().is_empty() {
            Err("the model returned nothing".to_string())
        } else {
            Ok(Suggestion::Text(text.trim().to_string()))
        }
    }
}

fn field_name(field: SheetField) -> &'static str {
    match field {
        SheetField::Objective => "objective",
        SheetField::Context => "context",
        SheetField::Constraints => "constraints",
        SheetField::OutOfScope => "outOfScope",
        SheetField::Deliverable => "deliverable",
        SheetField::Approach => "approach",
        SheetField::DoneWhen => "doneWhen",
    }
}

fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn strip_numbered(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }

    if i == 0 {
        return None;
    }

    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        if i < bytes.len() && bytes[i] == b' ' {
            return Some(&text[i + 1..]);
        }
    }

    if i < bytes.len() && bytes[i] == b')' {
        i += 1;
        if i < bytes.len() && bytes[i] == b' ' {
            return Some(&text[i + 1..]);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sheet() -> TaskSheet {
        TaskSheet {
            objective: "Ship it".to_string(),
            context: "Context".to_string(),
            constraints: "Constraints".to_string(),
            out_of_scope: "None".to_string(),
            deliverable: "Deliverable".to_string(),
            approach: "Approach".to_string(),
            done_when: vec![],
        }
    }

    #[test]
    fn the_prompt_carries_the_sheet_the_field_and_its_help() {
        let prompt = prompt(
            "Do the thing",
            &[],
            &sheet(),
            SheetField::Constraints,
            None,
            Lang::Fr,
        );

        assert!(prompt.contains("Do the thing"));
        assert!(prompt.contains(help(SheetField::Constraints)));
        assert!(prompt.contains("constraints"));
        assert!(prompt.contains("Ship it"));
        assert!(prompt.contains("Write in French."));
    }

    #[test]
    fn agents_md_is_truncated() {
        let agents_md = "a".repeat(9000);
        let prompt = prompt(
            "idea",
            &[],
            &sheet(),
            SheetField::Objective,
            Some(&agents_md),
            Lang::En,
        );

        assert!(prompt.contains(&"a".repeat(8000)));
        assert!(!prompt.contains(&"a".repeat(8001)));
    }

    #[test]
    fn a_placeholder_in_the_idea_is_not_substituted() {
        let prompt = prompt(
            "{sheet}",
            &[],
            &sheet(),
            SheetField::Objective,
            None,
            Lang::En,
        );

        assert!(prompt.contains("{sheet}"));
        assert_eq!(prompt.matches("The task sheet so far").count(), 1);
    }

    #[test]
    fn parse_reply_strips_fences_and_bullets() {
        assert_eq!(
            parse_reply(SheetField::DoneWhen, "```\n- one\n2. two\n* three\n```"),
            Ok(Suggestion::Criteria(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string()
            ]))
        );

        assert_eq!(
            parse_reply(SheetField::Objective, "```text\nHello\n```"),
            Ok(Suggestion::Text("Hello".to_string()))
        );
    }

    #[test]
    fn an_empty_reply_is_an_error() {
        assert!(parse_reply(SheetField::Context, "").is_err());
        assert!(parse_reply(SheetField::DoneWhen, "```\n```").is_err());
    }

    #[test]
    fn criteria_never_carry_a_check() {
        assert_eq!(
            parse_reply(SheetField::DoneWhen, "- Builds — check: `rm -rf /`"),
            Ok(Suggestion::Criteria(vec!["Builds".to_string()]))
        );
    }

    #[test]
    fn a_plan_heading_in_a_reply_is_demoted() {
        let reply = parse_reply(SheetField::Context, "## Done when\n- x").unwrap();

        match reply {
            Suggestion::Text(text) => assert!(text.starts_with("### Done when")),
            Suggestion::Criteria(_) => panic!("expected text"),
        }
    }

    #[test]
    fn suggestion_serializes_as_text_or_criteria() {
        assert_eq!(
            serde_json::to_value(Suggestion::Text("a".to_string())).unwrap(),
            json!({ "text": "a" })
        );
        assert_eq!(
            serde_json::to_value(Suggestion::Criteria(vec!["b".to_string()])).unwrap(),
            json!({ "criteria": ["b"] })
        );
    }
}
