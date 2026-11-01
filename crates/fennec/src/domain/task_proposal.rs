//! A task an agent proposed with `propose_task` (fennec's own), shown to the
//! human and turned into a backlog idea only on acceptance.

use super::event::Event;

use serde::Serialize;

/// The wire shape matches the session detail's `proposedTasks` list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedTask {
    pub call_id: String,
    pub title: String,
    pub text: String,
    pub idea_id: Option<String>,
}

/// The longest backlog text an accepted proposal becomes (title, blank line, body).
pub const MAX_TASK_CHARS: usize = 4_000;

/// Every task proposed in the log, in order, with the idea id if accepted.
pub fn proposed_tasks(events: &[Event]) -> Vec<ProposedTask> {
    let accepted = events
        .iter()
        .filter_map(|event| match event {
            Event::TaskAccepted { call_id, idea_id } => Some((call_id.as_str(), idea_id.as_str())),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    events
        .iter()
        .filter_map(|event| match event {
            Event::TaskProposed {
                call_id,
                title,
                text,
            } => Some(ProposedTask {
                call_id: call_id.clone(),
                title: title.clone(),
                text: text.clone(),
                idea_id: accepted.get(call_id.as_str()).map(|id| id.to_string()),
            }),
            _ => None,
        })
        .collect()
}

impl ProposedTask {
    /// The text stored in the backlog idea, capped to `MAX_TASK_CHARS` chars.
    pub fn task_text(&self) -> String {
        format!("{}\n\n{}", self.title, self.text)
            .chars()
            .take(MAX_TASK_CHARS)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposed(call_id: &str, title: &str, text: &str) -> Event {
        Event::TaskProposed {
            call_id: call_id.into(),
            title: title.into(),
            text: text.into(),
        }
    }

    fn accepted(call_id: &str, idea_id: &str) -> Event {
        Event::TaskAccepted {
            call_id: call_id.into(),
            idea_id: idea_id.into(),
        }
    }

    #[test]
    fn folds_accepted_and_unaccepted_proposals_in_order() {
        let events = vec![
            proposed("c1", "first", "x"),
            accepted("c1", "i1"),
            proposed("c2", "second", "y"),
        ];
        let tasks = proposed_tasks(&events);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].call_id, "c1");
        assert_eq!(tasks[0].idea_id, Some("i1".into()));
        assert_eq!(tasks[1].call_id, "c2");
        assert_eq!(tasks[1].idea_id, None);
    }

    #[test]
    fn task_text_is_title_blank_line_text() {
        let task = ProposedTask {
            call_id: "c".into(),
            title: "t".into(),
            text: "x".into(),
            idea_id: None,
        };
        assert_eq!(task.task_text(), "t\n\nx");
    }

    #[test]
    fn task_text_truncates_to_max_chars_not_bytes() {
        let task = ProposedTask {
            call_id: "c".into(),
            title: "t".into(),
            text: "é".repeat(MAX_TASK_CHARS),
            idea_id: None,
        };
        let text = task.task_text();
        assert_eq!(text.chars().count(), MAX_TASK_CHARS);
    }

    #[test]
    fn serializes_to_camel_case_with_null_idea_id() {
        let task = ProposedTask {
            call_id: "c".into(),
            title: "t".into(),
            text: "x".into(),
            idea_id: None,
        };
        let value = serde_json::to_value(vec![task]).unwrap();
        assert_eq!(
            value,
            serde_json::json!([{
                "callId": "c", "title": "t", "text": "x", "ideaId": null
            }])
        );
    }
}
