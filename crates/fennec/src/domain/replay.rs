//! The human's replay of one of their messages
//! (`POST /api/sessions/:id/replay`, without event deletion).

use super::attachment::Attachment;
use super::compaction::model_view;
use super::event::{Event, Role};

/// The text and images of `message_id` when the human may replay it: a
/// message they typed (not system-generated) in the model's current view.
pub fn replayable(events: &[Event], message_id: &str) -> Option<(String, Vec<Attachment>)> {
    typed_in_view(events)
        .find(|(id, _, _)| id.as_str() == message_id)
        .map(|(_, content, attachments)| (content.clone().unwrap_or_default(), attachments.clone()))
}

/// The ids of the messages the human may replay, in log order: the UI shows
/// the replay action on those only, so it never offers what the server refuses.
pub fn replayable_ids(events: &[Event]) -> Vec<String> {
    typed_in_view(events).map(|(id, _, _)| id.clone()).collect()
}

fn typed_in_view(
    events: &[Event],
) -> impl Iterator<Item = (&String, &Option<String>, &Vec<Attachment>)> {
    model_view(events)
        .into_iter()
        .filter_map(|event| match event {
            Event::MessageStart {
                message_id,
                role: Role::User,
                is_system_generated: false,
                content,
                attachments,
                ..
            } => Some((message_id, content, attachments)),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str, content: &str, system_generated: bool) -> Event {
        Event::MessageStart {
            message_id: id.into(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: system_generated,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    fn assistant(id: &str, content: &str) -> Event {
        Event::MessageStart {
            message_id: id.into(),
            role: Role::Assistant,
            content: Some(content.into()),
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn a_typed_user_message_is_replayable() {
        let attachment = Attachment {
            id: "a".into(),
            filename: "a.png".into(),
            mime_type: "image/png".into(),
            size: 1,
            data: "data:image/png;base64,AAAA".into(),
        };
        let events = vec![Event::MessageStart {
            message_id: "u1".into(),
            role: Role::User,
            content: Some("look".into()),
            is_system_generated: false,
            metadata: None,
            attachments: vec![attachment.clone()],
        }];
        assert_eq!(
            replayable(&events, "u1"),
            Some(("look".into(), vec![attachment]))
        );
    }

    #[test]
    fn a_system_generated_message_is_not_replayable() {
        let events = vec![user("u1", "reminder", true)];
        assert_eq!(replayable(&events, "u1"), None);
    }

    #[test]
    fn an_assistant_message_is_not_replayable() {
        let events = vec![assistant("a1", "answer")];
        assert_eq!(replayable(&events, "a1"), None);
    }

    #[test]
    fn a_message_before_the_last_compaction_is_not_replayable() {
        let events = vec![
            user("u0", "old", false),
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "s".into(),
            },
            user("u1", "new", false),
        ];
        assert_eq!(replayable(&events, "u0"), None);
    }

    #[test]
    fn the_replayable_ids_are_the_typed_messages_of_the_current_view() {
        let events = vec![
            user("u0", "old", false),
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "s".into(),
            },
            user("u1", "new", false),
            user("u2", "reminder", true),
            assistant("a1", "answer"),
            user("u3", "again", false),
        ];
        assert_eq!(
            replayable_ids(&events),
            vec!["u1".to_string(), "u3".to_string()]
        );
    }

    #[test]
    fn a_message_cut_off_by_a_rewind_is_not_replayable() {
        let events = vec![
            user("u1", "first", false),
            user("u2", "second", false),
            Event::SessionRewound {
                message_id: "u1".into(),
            },
        ];
        assert_eq!(replayable(&events, "u1"), None);
        assert_eq!(replayable(&events, "u2"), None);
    }
}
