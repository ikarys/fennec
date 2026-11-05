//! The architecture rules an agent proposes from the project's guidelines.
//! A guideline in AGENTS.md is text a model may ignore; a line in
//! `.agents/architecture` is checked by the integration gate, so the agent
//! only proposes it and the human adds it.

use std::collections::HashSet;

use crate::domain::architecture::Rules;
use crate::domain::conversation;
use crate::domain::event::Event;

/// The lines of the first fenced block under a `## Executable rules` heading.
///
/// Only `#` comments and lines `Rules::parse` recognises are kept, trimmed.
/// Trailing comments that no rule follows are dropped.
pub fn proposed_lines(answer: &str) -> Vec<String> {
    let mut after_heading = false;
    let mut in_block = false;
    let mut lines = Vec::new();

    for line in answer.lines() {
        let trimmed = line.trim();

        if !after_heading {
            if trimmed == "## Executable rules" || trimmed == "### Executable rules" {
                after_heading = true;
            }
            continue;
        }

        if !in_block {
            if trimmed.starts_with("## ") {
                break;
            }
            if trimmed.starts_with("```text")
                || trimmed.starts_with("```architecture")
                || trimmed == "```"
            {
                in_block = true;
            }
            continue;
        }

        if trimmed == "```" {
            break;
        }

        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') || !Rules::parse(trimmed).is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    while lines.last().is_some_and(|last| last.starts_with('#')) {
        lines.pop();
    }

    lines
}

/// The proposed lines that are not already in `existing`, with a comment kept
/// only when the next kept line is a rule.
pub fn new_lines(proposed: &[String], existing: &str) -> Vec<String> {
    let mut existing_rules = HashSet::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !Rules::parse(trimmed).is_empty() {
            existing_rules.insert(trimmed.to_string());
        }
    }

    let mut kept = Vec::new();
    let mut next_kept_is_rule = false;
    for line in proposed.iter().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            if next_kept_is_rule {
                kept.push(line.clone());
                next_kept_is_rule = false;
            }
        } else if !existing_rules.contains(trimmed) {
            kept.push(line.clone());
            next_kept_is_rule = true;
        }
    }
    kept.reverse();
    kept
}

/// True when the human appended the proposed rules after the last assistant answer.
pub fn appended_since_last_answer(events: &[Event]) -> bool {
    conversation::since_last_answer(events, |event| {
        matches!(event, Event::ArchitectureRulesAdded { .. })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::Role;

    fn assistant_start(message_id: &str) -> Event {
        Event::MessageStart {
            role: Role::Assistant,
            message_id: message_id.into(),
            content: None,
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn extracts_the_rule_block_after_the_heading() {
        let answer = "intro\n## Executable rules\n```text\n# no sql\nsrc/ forbids sql\nnot a rule\n# trailing\n```\n";
        assert_eq!(
            proposed_lines(answer),
            vec![String::from("# no sql"), String::from("src/ forbids sql")]
        );
    }

    #[test]
    fn drops_non_rule_lines_and_trailing_comments() {
        let answer = "### Executable rules\n```\n# only a comment\n```\n";
        assert!(proposed_lines(answer).is_empty());
    }

    #[test]
    fn removes_existing_rules_with_their_comment() {
        let proposed = vec![
            String::from("# keep"),
            String::from("src/ forbids sql"),
            String::from("# drop"),
            String::from("web/ forbids http"),
        ];
        let existing = "src/ forbids sql\n";
        assert_eq!(
            new_lines(&proposed, existing),
            vec![String::from("# drop"), String::from("web/ forbids http")]
        );
    }

    #[test]
    fn no_heading_is_empty() {
        let answer = "```text\nsrc/ forbids sql\n```\n";
        assert!(proposed_lines(answer).is_empty());
    }

    #[test]
    fn appended_since_last_answer_tracks_the_current_answer() {
        let events = vec![
            assistant_start("m1"),
            Event::ArchitectureRulesAdded { count: 1 },
        ];
        assert!(appended_since_last_answer(&events));

        let events = vec![
            assistant_start("m1"),
            Event::ArchitectureRulesAdded { count: 1 },
            assistant_start("m2"),
        ];
        assert!(!appended_since_last_answer(&events));
    }
}
