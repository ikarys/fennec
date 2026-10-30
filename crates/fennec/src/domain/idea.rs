//! Ideas are kept by fennec as a per-project backlog so a human's jotting is not lost mid-run.
//! The human frames an idea in a plan-mode session and later decides it is ready with an approved plan, or drops it.
//! Storage is append-only events; `fold` rebuilds the current list from the log.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IdeaEvent {
    #[serde(rename = "idea.added", rename_all = "camelCase")]
    Added { idea_id: String, text: String },
    #[serde(rename = "idea.framing", rename_all = "camelCase")]
    Framing { idea_id: String, session_id: String },
    #[serde(rename = "idea.ready", rename_all = "camelCase")]
    Ready { idea_id: String, plan: String },
    #[serde(rename = "idea.started", rename_all = "camelCase")]
    Started { idea_id: String },
    #[serde(rename = "idea.dropped", rename_all = "camelCase")]
    Dropped { idea_id: String },
    #[serde(rename = "idea.halted", rename_all = "camelCase")]
    Halted { idea_id: String, reason: String },
}

impl IdeaEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Added { .. } => "idea.added",
            Self::Framing { .. } => "idea.framing",
            Self::Ready { .. } => "idea.ready",
            Self::Started { .. } => "idea.started",
            Self::Dropped { .. } => "idea.dropped",
            Self::Halted { .. } => "idea.halted",
        }
    }

    pub fn idea_id(&self) -> &str {
        match self {
            Self::Added { idea_id, .. }
            | Self::Framing { idea_id, .. }
            | Self::Ready { idea_id, .. }
            | Self::Started { idea_id }
            | Self::Dropped { idea_id }
            | Self::Halted { idea_id, .. } => idea_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IdeaState {
    Inbox,
    Framing,
    Ready,
    Started,
    Dropped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Idea {
    pub id: String,
    pub text: String,
    pub state: IdeaState,
    pub session_id: Option<String>,
    pub plan: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub halted: Option<String>,
}

/// The current ideas, newest first, from `(timestamp_ms, event)` in log order.
pub fn fold(events: &[(u64, IdeaEvent)]) -> Vec<Idea> {
    let mut ideas: Vec<Idea> = Vec::new();

    for (timestamp, event) in events {
        let id = event.idea_id();
        let Some(index) = ideas.iter().position(|idea| idea.id == id) else {
            if let IdeaEvent::Added { text, .. } = event {
                ideas.push(Idea {
                    id: id.to_string(),
                    text: text.clone(),
                    state: IdeaState::Inbox,
                    session_id: None,
                    plan: None,
                    created_at: *timestamp,
                    updated_at: *timestamp,
                    halted: None,
                });
            }
            continue;
        };

        let idea = &mut ideas[index];
        // A dropped idea stays dropped: a late event never resurrects it.
        if idea.state == IdeaState::Dropped {
            continue;
        }
        match event {
            IdeaEvent::Added { .. } => continue,
            IdeaEvent::Framing { session_id, .. } => {
                idea.session_id = Some(session_id.clone());
                idea.state = IdeaState::Framing;
            }
            IdeaEvent::Ready { plan, .. } => {
                idea.plan = Some(plan.clone());
                idea.state = IdeaState::Ready;
                // A new plan puts a halted idea back in the queue.
                idea.halted = None;
            }
            // The human may start an idea at any stage; the session is set by
            // the framing event before it when the idea had none.
            IdeaEvent::Started { .. } => {
                idea.state = IdeaState::Started;
                // A fresh start clears a previous night-mode halt.
                idea.halted = None;
            }
            IdeaEvent::Dropped { .. } => idea.state = IdeaState::Dropped,
            IdeaEvent::Halted { reason, .. } => idea.halted = Some(reason.clone()),
        }
        idea.updated_at = *timestamp;
    }

    ideas.retain(|idea| idea.state != IdeaState::Dropped);
    ideas.sort_by_key(|idea| std::cmp::Reverse(idea.created_at));
    ideas
}

/// The idea a framing session belongs to, if any.
pub fn idea_for_session<'a>(ideas: &'a [Idea], session_id: &str) -> Option<&'a Idea> {
    ideas
        .iter()
        .find(|idea| idea.session_id.as_deref() == Some(session_id))
}

/// The oldest ready idea that can be started: it needs a framing session and a plan.
pub fn next_ready(ideas: &[Idea]) -> Option<&Idea> {
    ideas
        .iter()
        .filter(|idea| idea.state == IdeaState::Ready)
        .filter(|idea| idea.session_id.is_some() && idea.plan.is_some())
        .filter(|idea| idea.halted.is_none())
        .min_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.cmp(&b.id))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_then_framed_then_ready() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
            (
                2,
                IdeaEvent::Framing {
                    idea_id: "i1".into(),
                    session_id: "s1".into(),
                },
            ),
            (
                3,
                IdeaEvent::Ready {
                    idea_id: "i1".into(),
                    plan: "p".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].id, "i1");
        assert_eq!(ideas[0].state, IdeaState::Ready);
        assert_eq!(ideas[0].session_id.as_deref(), Some("s1"));
        assert_eq!(ideas[0].plan.as_deref(), Some("p"));
        assert_eq!(ideas[0].created_at, 1);
        assert_eq!(ideas[0].updated_at, 3);
    }

    #[test]
    fn dropped_ideas_are_excluded() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
            (
                2,
                IdeaEvent::Dropped {
                    idea_id: "i1".into(),
                },
            ),
        ];

        assert!(fold(&events).is_empty());
    }

    #[test]
    fn events_for_unknown_id_are_ignored() {
        let events = vec![
            (
                1,
                IdeaEvent::Framing {
                    idea_id: "missing".into(),
                    session_id: "s1".into(),
                },
            ),
            (
                2,
                IdeaEvent::Ready {
                    idea_id: "missing".into(),
                    plan: "p".into(),
                },
            ),
            (
                3,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].id, "i1");
        assert_eq!(ideas[0].state, IdeaState::Inbox);
    }

    #[test]
    fn ready_after_drop_does_not_resurrect() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
            (
                2,
                IdeaEvent::Dropped {
                    idea_id: "i1".into(),
                },
            ),
            (
                3,
                IdeaEvent::Ready {
                    idea_id: "i1".into(),
                    plan: "p".into(),
                },
            ),
        ];

        assert!(fold(&events).is_empty());
    }

    #[test]
    fn newest_created_ideas_come_first() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "old".into(),
                    text: "old".into(),
                },
            ),
            (
                2,
                IdeaEvent::Added {
                    idea_id: "new".into(),
                    text: "new".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(ideas[0].id, "new");
        assert_eq!(ideas[1].id, "old");
    }

    #[test]
    fn idea_event_serde_round_trip() {
        let event = IdeaEvent::Added {
            idea_id: "i1".into(),
            text: "t".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"type":"idea.added","data":{"ideaId":"i1","text":"t"}}"#
        );

        let decoded: IdeaEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn started_after_ready() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
            (
                2,
                IdeaEvent::Ready {
                    idea_id: "i1".into(),
                    plan: "p".into(),
                },
            ),
            (
                3,
                IdeaEvent::Started {
                    idea_id: "i1".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].state, IdeaState::Started);
        assert_eq!(ideas[0].plan.as_deref(), Some("p"));
        assert_eq!(ideas[0].updated_at, 3);
    }

    #[test]
    fn an_inbox_idea_can_be_started() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
            (
                2,
                IdeaEvent::Started {
                    idea_id: "i1".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].state, IdeaState::Started);
        assert_eq!(ideas[0].updated_at, 2);
    }

    #[test]
    fn next_ready_picks_oldest_ready_and_skips_missing_session() {
        let events = vec![
            (
                0,
                IdeaEvent::Added {
                    idea_id: "no_session".into(),
                    text: "no session".into(),
                },
            ),
            (
                1,
                IdeaEvent::Added {
                    idea_id: "old".into(),
                    text: "old".into(),
                },
            ),
            (
                2,
                IdeaEvent::Framing {
                    idea_id: "old".into(),
                    session_id: "s_old".into(),
                },
            ),
            (
                3,
                IdeaEvent::Ready {
                    idea_id: "old".into(),
                    plan: "p_old".into(),
                },
            ),
            (
                4,
                IdeaEvent::Added {
                    idea_id: "new".into(),
                    text: "new".into(),
                },
            ),
            (
                5,
                IdeaEvent::Framing {
                    idea_id: "new".into(),
                    session_id: "s_new".into(),
                },
            ),
            (
                6,
                IdeaEvent::Ready {
                    idea_id: "new".into(),
                    plan: "p_new".into(),
                },
            ),
            (
                7,
                IdeaEvent::Ready {
                    idea_id: "no_session".into(),
                    plan: "p_no_session".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(next_ready(&ideas).map(|idea| idea.id.as_str()), Some("old"));
    }

    #[test]
    fn started_event_serde_wire() {
        let event = IdeaEvent::Started {
            idea_id: "i1".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"type":"idea.started","data":{"ideaId":"i1"}}"#);

        let decoded: IdeaEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn halted_keeps_the_state_and_next_ready_skips_it() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
            (
                2,
                IdeaEvent::Framing {
                    idea_id: "i1".into(),
                    session_id: "s1".into(),
                },
            ),
            (
                3,
                IdeaEvent::Ready {
                    idea_id: "i1".into(),
                    plan: "p".into(),
                },
            ),
            (
                4,
                IdeaEvent::Halted {
                    idea_id: "i1".into(),
                    reason: "r".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].state, IdeaState::Ready);
        assert_eq!(ideas[0].halted.as_deref(), Some("r"));
        assert!(next_ready(&ideas).is_none());
    }

    #[test]
    fn a_new_start_clears_halted() {
        let events = vec![
            (
                1,
                IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            ),
            (
                2,
                IdeaEvent::Ready {
                    idea_id: "i1".into(),
                    plan: "p".into(),
                },
            ),
            (
                3,
                IdeaEvent::Halted {
                    idea_id: "i1".into(),
                    reason: "r".into(),
                },
            ),
            (
                4,
                IdeaEvent::Started {
                    idea_id: "i1".into(),
                },
            ),
        ];

        let ideas = fold(&events);

        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].state, IdeaState::Started);
        assert_eq!(ideas[0].halted, None);
    }

    #[test]
    fn halted_event_serde_wire() {
        let event = IdeaEvent::Halted {
            idea_id: "i1".into(),
            reason: "r".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"type":"idea.halted","data":{"ideaId":"i1","reason":"r"}}"#
        );

        let decoded: IdeaEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, event);
    }
}
