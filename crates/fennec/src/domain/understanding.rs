//! Comprehension dialogues on a branch: the gate's view of whether the
//! human has understood the change at its current head.

use crate::domain::event::{Event, Role};
use serde::Serialize;

/// The `session_id` of the LAST `UnderstandingStarted` whose `head` equals
/// `head`; a dialogue on an older head does not count (the branch moved since).
pub fn dialogue_for<'a>(events: &'a [Event], head: &str) -> Option<&'a str> {
    events.iter().rev().find_map(|event| match event {
        Event::UnderstandingStarted {
            session_id,
            head: event_head,
        } if event_head == head => Some(session_id.as_str()),
        _ => None,
    })
}

/// The human's messages in a dialogue's log after its opening brief: the
/// first non-system user message is the brief the UI sent.
pub fn human_replies(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::MessageStart {
                    role: Role::User,
                    is_system_generated: false,
                    ..
                }
            )
        })
        .count()
        .saturating_sub(1)
}

/// The comprehension dialogue's state on a branch head.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Understanding {
    /// No dialogue on this head.
    None,
    /// A dialogue was opened, but the human has not answered yet.
    Started { session_id: String },
    /// The human answered at least once.
    Held { session_id: String },
}

/// The comprehension dialogue's state on `head`, from the reviewed session's
/// log and the linked dialogue's log.
pub fn understanding(
    reviewed: &[Event],
    head: &str,
    dialogue: impl FnOnce(&str) -> Vec<Event>,
) -> Understanding {
    match dialogue_for(reviewed, head) {
        None => Understanding::None,
        Some(session_id) => {
            let replies = human_replies(&dialogue(session_id));
            if replies > 0 {
                Understanding::Held {
                    session_id: session_id.to_string(),
                }
            } else {
                Understanding::Started {
                    session_id: session_id.to_string(),
                }
            }
        }
    }
}

impl Understanding {
    /// True only when the human has answered in the dialogue on this head.
    pub fn held(&self) -> bool {
        matches!(self, Understanding::Held { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(session_id: &str, head: &str) -> Event {
        Event::UnderstandingStarted {
            session_id: session_id.to_string(),
            head: head.to_string(),
        }
    }

    fn user_message(system_generated: bool) -> Event {
        Event::MessageStart {
            message_id: "m".to_string(),
            role: Role::User,
            content: Some("text".into()),
            is_system_generated: system_generated,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn no_link_is_none() {
        let understanding = understanding(&[], "head", |_| Vec::new());

        assert_eq!(understanding, Understanding::None);
    }

    #[test]
    fn older_head_is_ignored() {
        let reviewed = vec![link("dialogue", "old")];

        let understanding = understanding(&reviewed, "head", |_| vec![user_message(false)]);

        assert_eq!(understanding, Understanding::None);
    }

    #[test]
    fn last_link_wins() {
        let reviewed = vec![link("first", "head"), link("second", "head")];

        assert_eq!(dialogue_for(&reviewed, "head"), Some("second"));
    }

    #[test]
    fn brief_alone_is_started() {
        let reviewed = vec![link("dialogue", "head")];
        let dialogue = vec![user_message(false)];

        let understanding = understanding(&reviewed, "head", |_| dialogue);

        assert_eq!(
            understanding,
            Understanding::Started {
                session_id: "dialogue".to_string()
            }
        );
    }

    #[test]
    fn one_reply_is_held() {
        let reviewed = vec![link("dialogue", "head")];
        let dialogue = vec![user_message(false), user_message(false)];

        let understanding = understanding(&reviewed, "head", |_| dialogue);

        assert_eq!(
            understanding,
            Understanding::Held {
                session_id: "dialogue".to_string()
            }
        );
    }

    #[test]
    fn system_generated_user_messages_do_not_count() {
        let reviewed = vec![link("dialogue", "head")];
        let dialogue = vec![user_message(false), user_message(true)];

        let understanding = understanding(&reviewed, "head", |_| dialogue);

        assert_eq!(
            understanding,
            Understanding::Started {
                session_id: "dialogue".to_string()
            }
        );
    }
}
