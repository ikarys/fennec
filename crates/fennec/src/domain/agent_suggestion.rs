//! The agent switch an agent suggested to the human: the last `agent.suggested` since the human last wrote.

use super::event::{Event, Role};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSuggestion {
    pub call_id: String,
    pub agent: String,
    pub reason: String,
    pub brief: String,
}

pub fn last_suggestion(events: &[Event]) -> Option<AgentSuggestion> {
    let mut last = None;

    for event in events {
        match event {
            Event::MessageStart {
                role: Role::User,
                is_system_generated: false,
                ..
            } => last = None,
            Event::AgentSuggested {
                call_id,
                agent,
                reason,
                brief,
            } => {
                last = Some(AgentSuggestion {
                    call_id: call_id.clone(),
                    agent: agent.clone(),
                    reason: reason.clone(),
                    brief: brief.clone(),
                });
            }
            _ => {}
        }
    }

    last
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human_message() -> Event {
        Event::MessageStart {
            message_id: "m1".into(),
            role: Role::User,
            content: None,
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    fn system_generated_user_message() -> Event {
        Event::MessageStart {
            message_id: "m2".into(),
            role: Role::User,
            content: None,
            is_system_generated: true,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    fn suggestion(agent: &str) -> Event {
        Event::AgentSuggested {
            call_id: "c1".into(),
            agent: agent.into(),
            reason: "r".into(),
            brief: "b".into(),
        }
    }

    #[test]
    fn none_returns_none() {
        assert_eq!(last_suggestion(&[]), None);
    }

    #[test]
    fn suggestion_returns_some() {
        let s = last_suggestion(&[suggestion("plan")]).unwrap();
        assert_eq!(s.agent, "plan");
        assert_eq!(s.reason, "r");
        assert_eq!(s.brief, "b");
    }

    #[test]
    fn a_later_human_message_clears_it() {
        assert_eq!(
            last_suggestion(&[suggestion("plan"), human_message()]),
            None
        );
    }

    #[test]
    fn a_system_generated_user_message_does_not_clear_it() {
        assert!(last_suggestion(&[suggestion("plan"), system_generated_user_message()]).is_some());
    }

    #[test]
    fn the_last_of_two_wins() {
        let s = last_suggestion(&[suggestion("plan"), suggestion("builder")]).unwrap();
        assert_eq!(s.agent, "builder");
    }
}
