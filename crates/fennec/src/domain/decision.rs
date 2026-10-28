//! A design decision an agent recorded with `record_decision` (fennec's own),
//! so the human can see why the generated code is shaped as it is.

use super::event::Event;

// Nothing on the server reads decisions back yet: the UI folds the same
// events (web/src/lib/fold.ts). The Rust fold is the reference for the first
// server-side reader (review gate, compaction facts).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub call_id: String,
    pub title: String,
    pub choice: String,
    pub reason: String,
    pub alternatives: Vec<String>,
}

/// Every decision recorded in the log, in order. None is ever superseded:
/// a later decision that reverses an earlier one is a decision of its own.
#[allow(dead_code)]
pub fn decisions_in_force(events: &[Event]) -> Vec<Decision> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::DecisionRecorded {
                call_id,
                title,
                choice,
                reason,
                alternatives,
            } => Some(Decision {
                call_id: call_id.clone(),
                title: title.clone(),
                choice: choice.clone(),
                reason: reason.clone(),
                alternatives: alternatives.clone(),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(call_id: &str, title: &str) -> Event {
        Event::DecisionRecorded {
            call_id: call_id.into(),
            title: title.into(),
            choice: "c".into(),
            reason: "r".into(),
            alternatives: vec!["a".into()],
        }
    }

    #[test]
    fn collects_every_decision_in_order() {
        let events = vec![
            recorded("c1", "first"),
            Event::CriteriaSet { criteria: vec![] },
            recorded("c2", "second"),
        ];
        let titles: Vec<_> = decisions_in_force(&events)
            .into_iter()
            .map(|d| d.title)
            .collect();
        assert_eq!(titles, ["first", "second"]);
    }

    #[test]
    fn no_decision_in_an_empty_log() {
        assert!(decisions_in_force(&[]).is_empty());
    }
}
