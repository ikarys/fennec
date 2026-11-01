//! The AGENTS.md draft an agent ends its answer with: parsed from the answer
//! text, and whether the human already wrote it since that answer.

use crate::domain::event::{Event, Role};

/// The first `## AGENTS.md draft` section's ```md block, if any.
///
/// Nested fences are tracked so a ```bash block inside the draft does not end
/// it. The result ends with exactly one newline.
pub fn draft(answer: &str) -> Option<String> {
    let mut after_heading = false;
    let mut depth = 0usize;
    let mut content = Vec::new();

    for line in answer.lines() {
        let trimmed = line.trim();

        if !after_heading {
            if trimmed == "## AGENTS.md draft" || trimmed == "### AGENTS.md draft" {
                after_heading = true;
            }
            continue;
        }

        if depth == 0 {
            if trimmed.starts_with("## ") {
                return None;
            }
            if trimmed.starts_with("```md") || trimmed.starts_with("```markdown") {
                depth = 1;
            }
            continue;
        }

        if trimmed == "```" {
            depth -= 1;
            if depth == 0 {
                let text = content.join("\n");
                let text = text.trim_end();
                return if text.is_empty() {
                    None
                } else {
                    Some(format!("{text}\n"))
                };
            }
        } else if trimmed.starts_with("```") && !trimmed[3..].trim().is_empty() {
            depth += 1;
        }
        content.push(line.to_string());
    }

    None
}

/// True when the human wrote the draft after the last assistant answer.
pub fn written_since_last_answer(events: &[Event]) -> bool {
    let mut last_assistant = None;
    for (index, event) in events.iter().enumerate() {
        if matches!(
            event,
            Event::MessageStart {
                role: Role::Assistant,
                ..
            }
        ) {
            last_assistant = Some(index);
        }
    }
    if let Some(start) = last_assistant {
        events[start + 1..]
            .iter()
            .any(|event| matches!(event, Event::AgentsMdWritten { .. }))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn extracts_the_md_block_after_the_heading() {
        let answer = "intro\n## AGENTS.md draft\n```md\nrule one\n```\n";
        assert_eq!(draft(answer).as_deref(), Some("rule one\n"));
    }

    #[test]
    fn keeps_nested_bash_fences_inside_the_draft() {
        let answer = "## AGENTS.md draft\n```md\nrun:\n```bash\necho hi\n```\n```\n";
        assert_eq!(
            draft(answer).as_deref(),
            Some("run:\n```bash\necho hi\n```\n")
        );
    }

    #[test]
    fn returns_none_without_the_heading() {
        assert_eq!(draft("```md\nx\n```\n"), None);
    }

    #[test]
    fn returns_none_when_unterminated() {
        assert_eq!(draft("## AGENTS.md draft\n```md\nx\n"), None);
    }

    #[test]
    fn returns_none_when_empty() {
        assert_eq!(draft("## AGENTS.md draft\n```md\n```\n"), None);
        assert_eq!(draft("## AGENTS.md draft\n```md\n   \n```\n"), None);
    }

    #[test]
    fn written_since_last_answer_tracks_the_current_answer() {
        let events = vec![
            assistant_start("m1"),
            Event::AgentsMdWritten { replaced: false },
        ];
        assert!(written_since_last_answer(&events));

        let events = vec![
            assistant_start("m1"),
            Event::AgentsMdWritten { replaced: false },
            assistant_start("m2"),
        ];
        assert!(!written_since_last_answer(&events));
    }
}
