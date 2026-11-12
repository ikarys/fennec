use crate::fold::{Backlog, Idea, IdeaState};
use std::fmt;

/// The ideas that can run from the queue, in queue order.
///
/// Halted ideas are included because they still need to be shown in the queue.
/// An idea readied from its sheet has no session yet: starting it opens one.
pub fn queue_order(backlog: &Backlog) -> Vec<&Idea> {
    let eligible: Vec<&Idea> = backlog
        .ideas
        .iter()
        .filter(|idea| idea.state == IdeaState::Ready && idea.plan.is_some())
        .collect();

    let mut result: Vec<&Idea> = Vec::new();
    let mut used: Vec<&str> = Vec::new();

    // The human's explicit order wins first. Unknown ids are skipped.
    for id in &backlog.order {
        if let Some(idea) = eligible.iter().find(|idea| idea.id == *id)
            && !used.contains(&idea.id.as_str())
        {
            result.push(idea);
            used.push(idea.id.as_str());
        }
    }

    // Everything else keeps the stable backlog order: oldest first, then id.
    let mut rest: Vec<&Idea> = eligible
        .into_iter()
        .filter(|idea| !used.contains(&idea.id.as_str()))
        .collect();
    rest.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    result.extend(rest);

    result
}

/// The next idea to start: the first queueable idea that is not halted.
pub fn next_ready(backlog: &Backlog) -> Option<&Idea> {
    queue_order(backlog)
        .into_iter()
        .find(|idea| idea.halted.is_none())
}

/// Optional limits for one queue run. Zero means no cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub max_minutes: u32,
    pub max_tokens: u64,
}

/// Why a queue run stopped early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapHit {
    Duration { minutes: u32 },
    Tokens { spent: u64, cap: u64 },
}

impl fmt::Display for CapHit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duration { minutes } => write!(f, "stopped after {minutes} minutes (cap)"),
            Self::Tokens { spent, cap } => write!(f, "stopped after {spent} tokens (cap {cap})"),
        }
    }
}

/// Check duration first, then tokens.
pub fn cap_reached(elapsed_ms: u64, tokens_spent: Option<u64>, caps: Caps) -> Option<CapHit> {
    if caps.max_minutes > 0 && elapsed_ms >= caps.max_minutes as u64 * 60_000 {
        return Some(CapHit::Duration {
            minutes: caps.max_minutes,
        });
    }

    if caps.max_tokens > 0
        && let Some(spent) = tokens_spent
        && spent > caps.max_tokens
    {
        return Some(CapHit::Tokens {
            spent,
            cap: caps.max_tokens,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BacklogEvent;
    use crate::fold::fold;

    fn ev(ts: u64, event: BacklogEvent) -> (u64, BacklogEvent) {
        (ts, event)
    }

    fn added(id: &str, text: &str) -> BacklogEvent {
        BacklogEvent::Added {
            idea_id: id.into(),
            text: text.into(),
            attachments: Vec::new(),
        }
    }

    fn framing(id: &str, session_id: &str) -> BacklogEvent {
        BacklogEvent::Framing {
            idea_id: id.into(),
            session_id: session_id.into(),
        }
    }

    fn ready(id: &str, plan: &str) -> BacklogEvent {
        BacklogEvent::Ready {
            idea_id: id.into(),
            plan: plan.into(),
        }
    }

    fn halted(id: &str, reason: &str) -> BacklogEvent {
        BacklogEvent::Halted {
            idea_id: id.into(),
            reason: reason.into(),
        }
    }

    fn reordered(ids: &[&str]) -> BacklogEvent {
        BacklogEvent::Reordered {
            ids: ids.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    fn ids<'a>(ideas: &[&'a Idea]) -> Vec<&'a str> {
        ideas.iter().map(|idea| idea.id.as_str()).collect()
    }

    #[test]
    fn next_ready_picks_oldest_ready_even_without_session() {
        let events = vec![
            ev(0, added("no_session", "no session")),
            ev(1, added("old", "old")),
            ev(2, framing("old", "s_old")),
            ev(3, ready("old", "p_old")),
            ev(4, added("new", "new")),
            ev(5, framing("new", "s_new")),
            ev(6, ready("new", "p_new")),
            ev(7, ready("no_session", "p_no_session")),
        ];

        let backlog = fold(&events);

        assert_eq!(
            next_ready(&backlog).map(|idea| idea.id.as_str()),
            Some("no_session")
        );
    }

    #[test]
    fn queue_order_respects_reordered_order() {
        let events = vec![
            ev(1, added("old", "old")),
            ev(2, framing("old", "s_old")),
            ev(3, ready("old", "p_old")),
            ev(4, added("new", "new")),
            ev(5, framing("new", "s_new")),
            ev(6, ready("new", "p_new")),
            ev(7, reordered(&["new", "old"])),
        ];

        let backlog = fold(&events);

        assert_eq!(ids(&queue_order(&backlog)), vec!["new", "old"]);
    }

    #[test]
    fn queue_order_includes_halted_but_next_ready_skips_it() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, framing("i1", "s1")),
            ev(3, ready("i1", "p")),
            ev(4, halted("i1", "r")),
        ];

        let backlog = fold(&events);

        assert_eq!(ids(&queue_order(&backlog)), vec!["i1"]);
        assert_eq!(next_ready(&backlog), None);
    }

    #[test]
    fn queue_order_rest_is_oldest_first_then_id() {
        let events = vec![
            ev(1, added("b", "b")),
            ev(1, framing("b", "s_b")),
            ev(1, ready("b", "p_b")),
            ev(1, added("a", "a")),
            ev(1, framing("a", "s_a")),
            ev(1, ready("a", "p_a")),
            ev(0, added("old", "old")),
            ev(0, framing("old", "s_old")),
            ev(0, ready("old", "p_old")),
        ];

        let backlog = fold(&events);

        assert_eq!(ids(&queue_order(&backlog)), vec!["old", "a", "b"]);
    }

    #[test]
    fn cap_reached_with_no_caps() {
        assert_eq!(
            cap_reached(
                1_000,
                Some(100),
                Caps {
                    max_minutes: 0,
                    max_tokens: 0
                }
            ),
            None
        );
    }

    #[test]
    fn cap_reached_at_duration_limit() {
        let hit = cap_reached(
            60_000,
            None,
            Caps {
                max_minutes: 1,
                max_tokens: 0,
            },
        );

        assert_eq!(hit, Some(CapHit::Duration { minutes: 1 }));
        assert_eq!(hit.unwrap().to_string(), "stopped after 1 minutes (cap)");
    }

    #[test]
    fn cap_reached_past_duration_limit() {
        let hit = cap_reached(
            61_000,
            None,
            Caps {
                max_minutes: 1,
                max_tokens: 0,
            },
        );

        assert_eq!(hit, Some(CapHit::Duration { minutes: 1 }));
    }

    #[test]
    fn cap_reached_at_token_limit_is_not_a_hit() {
        assert_eq!(
            cap_reached(
                0,
                Some(100),
                Caps {
                    max_minutes: 0,
                    max_tokens: 100
                }
            ),
            None
        );
    }

    #[test]
    fn cap_reached_past_token_limit() {
        let hit = cap_reached(
            0,
            Some(101),
            Caps {
                max_minutes: 0,
                max_tokens: 100,
            },
        );

        assert_eq!(
            hit,
            Some(CapHit::Tokens {
                spent: 101,
                cap: 100
            })
        );
        assert_eq!(
            hit.unwrap().to_string(),
            "stopped after 101 tokens (cap 100)"
        );
    }

    #[test]
    fn cap_reached_with_zero_caps_and_tokens_none() {
        assert_eq!(
            cap_reached(
                1_000,
                None,
                Caps {
                    max_minutes: 0,
                    max_tokens: 0
                }
            ),
            None
        );
    }
}
