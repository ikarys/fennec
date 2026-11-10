use crate::event::BacklogEvent;
use crate::sheet::{Author, SheetField, TaskSheet};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IdeaState {
    Inbox,
    Framing,
    Ready,
    Started,
    Dropped,
    Completed,
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
    pub started_at: Option<u64>,
    pub autonomous: bool,
    pub attachments: Vec<crate::attachment::IdeaAttachment>,
    /// The task sheet, once the idea was turned into a task.
    pub sheet: Option<TaskSheet>,
    /// The sheet fields whose last change came from a model: shown as indicative.
    pub model_fields: BTreeSet<SheetField>,
}

/// One launch of the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueRun {
    pub started_at: u64,
    pub autonomous: bool,
    pub finished_at: Option<u64>,
    pub reason: Option<String>,
    pub idea_ids: Vec<String>,
    pub read: bool,
}

impl QueueRun {
    /// A run is open until a `QueueFinished` event closes it.
    pub fn is_open(&self) -> bool {
        self.finished_at.is_none()
    }
}

/// The current backlog, folded from append-only events.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Backlog {
    pub ideas: Vec<Idea>,
    pub order: Vec<String>,
    pub last_run: Option<QueueRun>,
}

/// Rebuild the current backlog from `(timestamp_ms, event)` in log order.
pub fn fold(events: &[(u64, BacklogEvent)]) -> Backlog {
    let mut ideas: Vec<Idea> = Vec::new();
    let mut order: Vec<String> = Vec::new();
    let mut last_run: Option<QueueRun> = None;

    for (timestamp, event) in events {
        match event {
            BacklogEvent::Reordered { ids } => {
                order = ids.clone();
                continue;
            }
            BacklogEvent::QueueStarted { autonomous } => {
                last_run = Some(QueueRun {
                    started_at: *timestamp,
                    autonomous: *autonomous,
                    finished_at: None,
                    reason: None,
                    idea_ids: Vec::new(),
                    read: false,
                });
                continue;
            }
            BacklogEvent::QueueFinished { reason } => {
                if let Some(run) = last_run.as_mut()
                    && run.is_open()
                {
                    run.finished_at = Some(*timestamp);
                    run.reason = Some(reason.clone());
                }
                continue;
            }
            BacklogEvent::ReportRead {} => {
                if let Some(run) = last_run.as_mut() {
                    run.read = true;
                }
                continue;
            }
            _ => {}
        }

        let Some(id) = event.idea_id() else {
            continue;
        };

        let Some(index) = ideas.iter().position(|idea| idea.id == id) else {
            if let BacklogEvent::Added {
                text, attachments, ..
            } = event
            {
                ideas.push(Idea {
                    id: id.to_string(),
                    text: text.clone(),
                    state: IdeaState::Inbox,
                    session_id: None,
                    plan: None,
                    created_at: *timestamp,
                    updated_at: *timestamp,
                    halted: None,
                    started_at: None,
                    autonomous: false,
                    attachments: attachments.clone(),
                    sheet: None,
                    model_fields: BTreeSet::new(),
                });
            }
            continue;
        };

        let idea = &mut ideas[index];

        // A dropped idea stays dropped and a completed idea stays done: a late
        // event never resurrects either. A done idea may still be removed.
        if idea.state == IdeaState::Dropped
            || (idea.state == IdeaState::Completed
                && !matches!(event, BacklogEvent::Dropped { .. }))
        {
            continue;
        }

        match event {
            BacklogEvent::Added { .. } => continue,
            BacklogEvent::Edited { text, .. } => idea.text = text.clone(),
            BacklogEvent::Framing { session_id, .. } => {
                idea.session_id = Some(session_id.clone());
                idea.state = IdeaState::Framing;
            }
            BacklogEvent::Ready { plan, .. } => {
                idea.plan = Some(plan.clone());
                idea.state = IdeaState::Ready;
                // A new plan puts a halted idea back in the queue.
                idea.halted = None;
            }
            BacklogEvent::SheetEdited { sheet, by, .. } => {
                let previous = idea.sheet.take().unwrap_or_default();
                for field in SheetField::ALL {
                    if !previous.field_eq(sheet, field) {
                        match by {
                            Author::Agent => {
                                idea.model_fields.insert(field);
                            }
                            Author::Human => {
                                idea.model_fields.remove(&field);
                            }
                        }
                    }
                }
                idea.sheet = Some(sheet.clone());
                // A changed sheet never leaves a stale plan queued: the human marks it ready again.
                if idea.state == IdeaState::Ready {
                    idea.state = if idea.session_id.is_some() {
                        IdeaState::Framing
                    } else {
                        IdeaState::Inbox
                    };
                }
            }
            BacklogEvent::Started { autonomous, .. } => {
                idea.state = IdeaState::Started;
                // A fresh start clears a previous night-mode halt.
                idea.halted = None;
                idea.started_at = Some(*timestamp);
                idea.autonomous = *autonomous;

                if let Some(run) = last_run.as_mut()
                    && run.is_open()
                    && !run.idea_ids.iter().any(|existing| existing.as_str() == id)
                {
                    run.idea_ids.push(id.to_string());
                }
            }
            BacklogEvent::Dropped { .. } => idea.state = IdeaState::Dropped,
            BacklogEvent::Completed { .. } => {
                idea.state = IdeaState::Completed;
                // Completion is the human's final word: no halt remains.
                idea.halted = None;
            }
            BacklogEvent::Halted { reason, .. } => idea.halted = Some(reason.clone()),
            BacklogEvent::Reordered { .. }
            | BacklogEvent::QueueStarted { .. }
            | BacklogEvent::QueueFinished { .. }
            | BacklogEvent::ReportRead {} => {}
        }

        idea.updated_at = *timestamp;
    }

    ideas.retain(|idea| idea.state != IdeaState::Dropped);
    ideas.sort_by_key(|idea| Reverse(idea.created_at));

    Backlog {
        ideas,
        order,
        last_run,
    }
}

/// The idea a framing session belongs to, if any.
pub fn idea_for_session<'a>(ideas: &'a [Idea], session_id: &str) -> Option<&'a Idea> {
    ideas
        .iter()
        .find(|idea| idea.session_id.as_deref() == Some(session_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::BacklogEvent;

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

    fn edited(id: &str, text: &str) -> BacklogEvent {
        BacklogEvent::Edited {
            idea_id: id.into(),
            text: text.into(),
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

    fn sheet_edited(id: &str, sheet: TaskSheet, by: Author) -> BacklogEvent {
        BacklogEvent::SheetEdited {
            idea_id: id.into(),
            sheet,
            by,
        }
    }

    fn started(id: &str, autonomous: bool) -> BacklogEvent {
        BacklogEvent::Started {
            idea_id: id.into(),
            autonomous,
        }
    }

    fn dropped(id: &str) -> BacklogEvent {
        BacklogEvent::Dropped { idea_id: id.into() }
    }

    fn completed(id: &str) -> BacklogEvent {
        BacklogEvent::Completed { idea_id: id.into() }
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

    fn queue_started(autonomous: bool) -> BacklogEvent {
        BacklogEvent::QueueStarted { autonomous }
    }

    fn queue_finished(reason: &str) -> BacklogEvent {
        BacklogEvent::QueueFinished {
            reason: reason.into(),
        }
    }

    fn report_read() -> BacklogEvent {
        BacklogEvent::ReportRead {}
    }

    #[test]
    fn added_then_framed_then_ready() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, framing("i1", "s1")),
            ev(3, ready("i1", "p")),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.id, "i1");
        assert_eq!(idea.state, IdeaState::Ready);
        assert_eq!(idea.session_id.as_deref(), Some("s1"));
        assert_eq!(idea.plan.as_deref(), Some("p"));
        assert_eq!(idea.created_at, 1);
        assert_eq!(idea.updated_at, 3);
        assert_eq!(idea.started_at, None);
        assert!(!idea.autonomous);
    }

    #[test]
    fn added_then_edited_updates_text_and_timestamp() {
        let events = vec![ev(1, added("i1", "old")), ev(2, edited("i1", "new"))];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.text, "new");
        assert_eq!(idea.updated_at, 2);
    }

    #[test]
    fn edited_after_completed_keeps_text() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, completed("i1")),
            ev(3, edited("i1", "new")),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.state, IdeaState::Completed);
        assert_eq!(idea.text, "t");
        assert_eq!(idea.updated_at, 2);
    }

    #[test]
    fn added_carries_its_attachments() {
        let events = vec![ev(
            1,
            BacklogEvent::Added {
                idea_id: "i1".into(),
                text: "t".into(),
                attachments: vec![crate::attachment::IdeaAttachment {
                    filename: "a.png".into(),
                    mime_type: "image/png".into(),
                    size: 3,
                    data: "data:image/png;base64,AAA=".into(),
                }],
            },
        )];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas[0].attachments.len(), 1);
        assert_eq!(backlog.ideas[0].attachments[0].filename, "a.png");
    }

    #[test]
    fn dropped_ideas_are_excluded() {
        let events = vec![ev(1, added("i1", "t")), ev(2, dropped("i1"))];

        let backlog = fold(&events);

        assert!(backlog.ideas.is_empty());
    }

    #[test]
    fn completed_ideas_stay_in_the_list_and_clear_halted() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, ready("i1", "p")),
            ev(3, halted("i1", "r")),
            ev(4, completed("i1")),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.state, IdeaState::Completed);
        assert_eq!(idea.halted, None);
        assert_eq!(idea.updated_at, 4);
    }

    #[test]
    fn completed_ideas_are_not_excluded_like_dropped() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, completed("i1")),
            ev(3, added("i2", "t2")),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 2);
        assert_eq!(backlog.ideas[0].id, "i2");
        assert_eq!(backlog.ideas[1].id, "i1");
        assert_eq!(backlog.ideas[1].state, IdeaState::Completed);
    }

    #[test]
    fn completed_ideas_stay_completed() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, completed("i1")),
            ev(3, ready("i1", "p")),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        assert_eq!(backlog.ideas[0].state, IdeaState::Completed);
        assert_eq!(backlog.ideas[0].plan, None);
    }

    #[test]
    fn a_completed_idea_can_be_removed() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, completed("i1")),
            ev(3, dropped("i1")),
        ];

        assert!(fold(&events).ideas.is_empty());
    }

    #[test]
    fn completed_after_drop_does_not_resurrect() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, dropped("i1")),
            ev(3, completed("i1")),
        ];

        let backlog = fold(&events);

        assert!(backlog.ideas.is_empty());
    }

    #[test]
    fn events_for_unknown_id_are_ignored() {
        let events = vec![
            ev(1, framing("missing", "s1")),
            ev(2, ready("missing", "p")),
            ev(3, added("i1", "t")),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        assert_eq!(backlog.ideas[0].id, "i1");
        assert_eq!(backlog.ideas[0].state, IdeaState::Inbox);
    }

    #[test]
    fn ready_after_drop_does_not_resurrect() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, dropped("i1")),
            ev(3, ready("i1", "p")),
        ];

        let backlog = fold(&events);

        assert!(backlog.ideas.is_empty());
    }

    #[test]
    fn newest_created_ideas_come_first() {
        let events = vec![ev(1, added("old", "old")), ev(2, added("new", "new"))];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas[0].id, "new");
        assert_eq!(backlog.ideas[1].id, "old");
    }

    #[test]
    fn started_after_ready() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, ready("i1", "p")),
            ev(3, started("i1", false)),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.state, IdeaState::Started);
        assert_eq!(idea.plan.as_deref(), Some("p"));
        assert_eq!(idea.updated_at, 3);
        assert_eq!(idea.started_at, Some(3));
        assert!(!idea.autonomous);
    }

    #[test]
    fn an_inbox_idea_can_be_started() {
        let events = vec![ev(1, added("i1", "t")), ev(2, started("i1", false))];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.state, IdeaState::Started);
        assert_eq!(idea.updated_at, 2);
        assert_eq!(idea.started_at, Some(2));
    }

    #[test]
    fn a_new_start_clears_halted() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, ready("i1", "p")),
            ev(3, halted("i1", "r")),
            ev(4, started("i1", false)),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.state, IdeaState::Started);
        assert_eq!(idea.halted, None);
        assert_eq!(idea.started_at, Some(4));
    }

    #[test]
    fn halted_keeps_the_state() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, framing("i1", "s1")),
            ev(3, ready("i1", "p")),
            ev(4, halted("i1", "r")),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.ideas.len(), 1);
        let idea = &backlog.ideas[0];
        assert_eq!(idea.state, IdeaState::Ready);
        assert_eq!(idea.halted.as_deref(), Some("r"));
    }

    #[test]
    fn reordered_replaces_order() {
        let events = vec![
            ev(1, added("i1", "t1")),
            ev(2, added("i2", "t2")),
            ev(3, reordered(&["i2", "i1"])),
        ];

        let backlog = fold(&events);

        assert_eq!(backlog.order, vec!["i2".to_string(), "i1".to_string()]);
    }

    #[test]
    fn queue_started_opens_a_run() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, ready("i1", "p")),
            ev(3, queue_started(true)),
            ev(4, started("i1", true)),
        ];

        let backlog = fold(&events);

        let run = backlog.last_run.expect("run should be open");
        assert_eq!(run.started_at, 3);
        assert!(run.autonomous);
        assert_eq!(run.finished_at, None);
        assert_eq!(run.reason, None);
        assert_eq!(run.idea_ids, vec!["i1".to_string()]);
        assert!(!run.read);
        assert!(run.is_open());
    }

    #[test]
    fn started_records_idea_in_open_run_without_duplicates() {
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, ready("i1", "p")),
            ev(3, queue_started(false)),
            ev(4, started("i1", false)),
            ev(5, started("i1", false)),
        ];

        let backlog = fold(&events);

        let run = backlog.last_run.expect("run should be open");
        assert_eq!(run.idea_ids, vec!["i1".to_string()]);
    }

    #[test]
    fn queue_finished_closes_the_open_run() {
        let events = vec![ev(1, queue_started(true)), ev(2, queue_finished("done"))];

        let backlog = fold(&events);

        let run = backlog.last_run.expect("run should exist");
        assert_eq!(run.started_at, 1);
        assert!(run.autonomous);
        assert_eq!(run.finished_at, Some(2));
        assert_eq!(run.reason.as_deref(), Some("done"));
        assert!(!run.is_open());
    }

    #[test]
    fn queue_finished_is_ignored_when_no_run_is_open() {
        let events = vec![ev(1, queue_finished("done"))];

        let backlog = fold(&events);

        assert_eq!(backlog.last_run, None);
    }

    #[test]
    fn report_read_marks_the_last_run_read() {
        let events = vec![ev(1, queue_started(false)), ev(2, report_read())];

        let backlog = fold(&events);

        let run = backlog.last_run.expect("run should exist");
        assert!(run.read);
    }

    #[test]
    fn events_without_idea_id_do_not_touch_ideas() {
        let events = vec![
            ev(1, reordered(&["i1"])),
            ev(2, queue_started(false)),
            ev(3, queue_finished("done")),
            ev(4, report_read()),
        ];

        let backlog = fold(&events);

        assert!(backlog.ideas.is_empty());
        assert_eq!(backlog.order, vec!["i1".to_string()]);
        let run = backlog.last_run.expect("run should exist");
        assert!(!run.is_open());
        assert!(run.read);
    }

    #[test]
    fn idea_for_session_finds_the_idea() {
        let events = vec![ev(1, added("i1", "t")), ev(2, framing("i1", "s1"))];

        let backlog = fold(&events);

        assert_eq!(
            idea_for_session(&backlog.ideas, "s1").map(|idea| idea.id.as_str()),
            Some("i1")
        );
        assert_eq!(idea_for_session(&backlog.ideas, "s2"), None);
    }

    #[test]
    fn a_sheet_save_sets_the_sheet() {
        let sheet = TaskSheet {
            objective: "o".into(),
            ..Default::default()
        };
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, sheet_edited("i1", sheet.clone(), Author::Human)),
        ];

        let backlog = fold(&events);

        let idea = &backlog.ideas[0];
        assert_eq!(idea.sheet, Some(sheet));
        assert!(idea.model_fields.is_empty());
        assert_eq!(idea.updated_at, 2);
    }

    #[test]
    fn an_agent_save_marks_the_fields_it_changed() {
        let sheet = TaskSheet {
            objective: "o".into(),
            context: "c".into(),
            ..Default::default()
        };
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, sheet_edited("i1", sheet, Author::Agent)),
        ];

        let backlog = fold(&events);

        assert_eq!(
            backlog.ideas[0].model_fields,
            BTreeSet::from([SheetField::Objective, SheetField::Context,])
        );
    }

    #[test]
    fn a_human_change_clears_the_mark_and_an_unchanged_resend_keeps_it() {
        let agent_sheet = TaskSheet {
            objective: "o".into(),
            context: "c".into(),
            ..Default::default()
        };
        let human_sheet = TaskSheet {
            objective: "new".into(),
            context: "c".into(),
            ..Default::default()
        };
        let events = vec![
            ev(1, added("i1", "t")),
            ev(2, sheet_edited("i1", agent_sheet, Author::Agent)),
            ev(3, sheet_edited("i1", human_sheet, Author::Human)),
        ];

        let backlog = fold(&events);

        assert_eq!(
            backlog.ideas[0].model_fields,
            BTreeSet::from([SheetField::Context])
        );
    }

    #[test]
    fn a_sheet_save_on_a_ready_idea_unreadies_it() {
        let sheet = TaskSheet::default();
        let with_session = vec![
            ev(1, added("i1", "t")),
            ev(2, framing("i1", "s1")),
            ev(3, ready("i1", "p")),
            ev(4, sheet_edited("i1", sheet.clone(), Author::Human)),
        ];
        let without_session = vec![
            ev(1, added("i1", "t")),
            ev(2, ready("i1", "p")),
            ev(3, sheet_edited("i1", sheet, Author::Human)),
        ];

        assert_eq!(fold(&with_session).ideas[0].state, IdeaState::Framing);
        assert_eq!(fold(&without_session).ideas[0].state, IdeaState::Inbox);
    }
}
