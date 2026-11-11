use crate::fold::{Idea, IdeaState};
use serde::Serialize;

/// What the caller knows about the idea's live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Running,
    WaitingForUser,
    Finished,
    Failed,
    Integrated,
}

/// What the human sees for one idea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Inbox,
    Framing,
    Queued,
    Running,
    Question,
    Review,
    Halted,
    Failed,
    Integrated,
    Completed,
}

/// The top-level section an idea belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Yours,
    Running,
    Queue,
    Drafts,
    Done,
}

/// The one thing to do next for an idea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// A draft turned into a task, or a task's sheet opened.
    Task,
    Start,
    Open,
    Answer,
    Review,
}

/// The secondary actions offered in an idea's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MenuAction {
    /// A draft framed with the agent in a plan session (`idea.framing`).
    Frame,
    Restart,
    Reframe,
    StartUnframed,
    Complete,
    Drop,
    /// A done task taken off the board (`idea.dropped`); its session stays.
    Remove,
    /// A draft's text rewritten by the human (`idea.edited`).
    EditText,
    /// The plan that will be built rewritten by the human (`idea.ready`).
    EditPlan,
}

/// Display order for stages. `Yours` first because it waits for the human.
pub const STAGES: [Stage; 5] = [
    Stage::Yours,
    Stage::Running,
    Stage::Queue,
    Stage::Drafts,
    Stage::Done,
];

/// Derive the human-facing status from stored idea state and live progress.
pub fn status(idea: &Idea, progress: Option<Progress>) -> Status {
    match idea.state {
        IdeaState::Inbox => Status::Inbox,
        IdeaState::Framing => Status::Framing,
        IdeaState::Ready => {
            if idea.halted.is_some() {
                Status::Halted
            } else {
                Status::Queued
            }
        }
        // Integration is final and wins over a stale halt.
        IdeaState::Started => match progress {
            Some(Progress::Integrated) => Status::Integrated,
            _ if idea.halted.is_some() => Status::Halted,
            Some(Progress::Running) => Status::Running,
            Some(Progress::WaitingForUser) => Status::Question,
            Some(Progress::Finished) => Status::Review,
            // No progress means the session is gone: treat it as failed.
            Some(Progress::Failed) | None => Status::Failed,
        },
        // Completion is final and wins over any live progress.
        IdeaState::Completed => Status::Completed,
        // Dropped ideas are excluded by fold, but map to a neutral state.
        IdeaState::Dropped => Status::Inbox,
    }
}

impl Status {
    /// The stage this status belongs to.
    ///
    /// `Yours` is everything that waits for the human's hand, so it is shown
    /// first.
    pub fn stage(self) -> Stage {
        match self {
            Status::Review | Status::Question | Status::Halted | Status::Failed => Stage::Yours,
            Status::Running => Stage::Running,
            Status::Queued => Stage::Queue,
            Status::Inbox | Status::Framing => Stage::Drafts,
            Status::Integrated | Status::Completed => Stage::Done,
        }
    }

    /// The single next action for this status.
    pub fn action(self) -> Action {
        match self {
            Status::Inbox | Status::Framing => Action::Task,
            Status::Queued => Action::Start,
            Status::Running => Action::Open,
            Status::Question => Action::Answer,
            Status::Review => Action::Review,
            Status::Halted | Status::Failed | Status::Integrated | Status::Completed => {
                Action::Open
            }
        }
    }

    /// The edit shown as its own button beside the primary action: a draft's text.
    pub fn edit(self) -> Option<MenuAction> {
        match self {
            Status::Inbox => Some(MenuAction::EditText),
            _ => None,
        }
    }

    /// The secondary actions offered in an idea's menu.
    ///
    /// `Reframe` is only offered where the fold accepts a `Framing` event for
    /// the idea's state; the explicit list keeps the UI from offering a step
    /// the server would ignore.
    pub fn menu(self) -> &'static [MenuAction] {
        match self {
            Status::Inbox => &[
                MenuAction::Frame,
                MenuAction::StartUnframed,
                MenuAction::Drop,
            ],
            // A failed idea is retried by framing it again.
            Status::Framing => &[MenuAction::Complete, MenuAction::Reframe, MenuAction::Drop],
            Status::Queued => &[MenuAction::EditPlan, MenuAction::Reframe, MenuAction::Drop],
            // A halted or failed idea can be relaunched from its plan before it
            // is framed again.
            Status::Halted | Status::Failed => &[
                MenuAction::Restart,
                MenuAction::EditPlan,
                MenuAction::Reframe,
                MenuAction::Complete,
                MenuAction::Drop,
            ],
            Status::Review | Status::Question => &[MenuAction::Complete, MenuAction::Drop],
            Status::Running => &[],
            Status::Integrated | Status::Completed => &[MenuAction::Remove],
        }
    }

    /// The menu of an idea with a task sheet: its plan is rendered from the
    /// sheet, so it is edited there, never as raw text.
    pub fn sheet_menu(self) -> Vec<MenuAction> {
        self.menu()
            .iter()
            .copied()
            .filter(|action| *action != MenuAction::EditPlan)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idea(state: IdeaState, halted: Option<&str>) -> Idea {
        Idea {
            id: "i1".into(),
            text: "t".into(),
            state,
            session_id: Some("s1".into()),
            plan: Some("p".into()),
            created_at: 1,
            updated_at: 1,
            halted: halted.map(str::to_string),
            started_at: None,
            autonomous: false,
            attachments: Vec::new(),
            sheet: None,
            model_fields: Default::default(),
        }
    }

    #[test]
    fn status_maps_inbox_and_framing() {
        assert_eq!(status(&idea(IdeaState::Inbox, None), None), Status::Inbox);
        assert_eq!(
            status(&idea(IdeaState::Framing, None), None),
            Status::Framing
        );
    }

    #[test]
    fn status_maps_ready_queued_and_halted() {
        assert_eq!(status(&idea(IdeaState::Ready, None), None), Status::Queued);
        assert_eq!(
            status(&idea(IdeaState::Ready, Some("r")), None),
            Status::Halted
        );
    }

    #[test]
    fn status_maps_started_progress() {
        let idea = idea(IdeaState::Started, None);

        assert_eq!(status(&idea, Some(Progress::Running)), Status::Running);
        assert_eq!(
            status(&idea, Some(Progress::WaitingForUser)),
            Status::Question
        );
        assert_eq!(status(&idea, Some(Progress::Finished)), Status::Review);
        assert_eq!(status(&idea, Some(Progress::Failed)), Status::Failed);
        assert_eq!(status(&idea, None), Status::Failed);
    }

    #[test]
    fn integrated_wins_over_halted() {
        let idea = idea(IdeaState::Started, Some("r"));

        assert_eq!(
            status(&idea, Some(Progress::Integrated)),
            Status::Integrated
        );
    }

    #[test]
    fn completed_wins_over_progress() {
        let idea = idea(IdeaState::Completed, Some("r"));

        assert_eq!(status(&idea, Some(Progress::Running)), Status::Completed);
        assert_eq!(status(&idea, Some(Progress::Integrated)), Status::Completed);
        assert_eq!(status(&idea, None), Status::Completed);
    }

    #[test]
    fn dropped_maps_to_inbox() {
        assert_eq!(status(&idea(IdeaState::Dropped, None), None), Status::Inbox);
    }

    #[test]
    fn stage_and_action_match_the_human_view() {
        assert_eq!(Status::Inbox.stage(), Stage::Drafts);
        assert_eq!(Status::Inbox.action(), Action::Task);

        assert_eq!(Status::Framing.stage(), Stage::Drafts);
        assert_eq!(Status::Framing.action(), Action::Task);

        assert_eq!(Status::Queued.stage(), Stage::Queue);
        assert_eq!(Status::Queued.action(), Action::Start);

        assert_eq!(Status::Running.stage(), Stage::Running);
        assert_eq!(Status::Running.action(), Action::Open);

        assert_eq!(Status::Question.stage(), Stage::Yours);
        assert_eq!(Status::Question.action(), Action::Answer);

        assert_eq!(Status::Review.stage(), Stage::Yours);
        assert_eq!(Status::Review.action(), Action::Review);

        assert_eq!(Status::Halted.stage(), Stage::Yours);
        assert_eq!(Status::Halted.action(), Action::Open);

        assert_eq!(Status::Failed.stage(), Stage::Yours);
        assert_eq!(Status::Failed.action(), Action::Open);

        assert_eq!(Status::Integrated.stage(), Stage::Done);
        assert_eq!(Status::Integrated.action(), Action::Open);

        assert_eq!(Status::Completed.stage(), Stage::Done);
        assert_eq!(Status::Completed.action(), Action::Open);
    }

    #[test]
    fn a_sheet_hides_the_raw_plan_edit() {
        assert_eq!(
            Status::Queued.sheet_menu(),
            [MenuAction::Reframe, MenuAction::Drop]
        );
        assert_eq!(Status::Inbox.sheet_menu(), Status::Inbox.menu());
    }

    #[test]
    fn menu_lists_match_the_states() {
        assert_eq!(
            Status::Inbox.menu(),
            &[
                MenuAction::Frame,
                MenuAction::StartUnframed,
                MenuAction::Drop
            ]
        );
        assert_eq!(Status::Inbox.edit(), Some(MenuAction::EditText));
        assert_eq!(Status::Queued.edit(), None);
        assert_eq!(
            Status::Framing.menu(),
            &[MenuAction::Complete, MenuAction::Reframe, MenuAction::Drop]
        );
        assert_eq!(
            Status::Queued.menu(),
            &[MenuAction::EditPlan, MenuAction::Reframe, MenuAction::Drop]
        );
        assert_eq!(
            Status::Halted.menu(),
            &[
                MenuAction::Restart,
                MenuAction::EditPlan,
                MenuAction::Reframe,
                MenuAction::Complete,
                MenuAction::Drop,
            ]
        );
        assert_eq!(
            Status::Failed.menu(),
            &[
                MenuAction::Restart,
                MenuAction::EditPlan,
                MenuAction::Reframe,
                MenuAction::Complete,
                MenuAction::Drop,
            ]
        );
        assert_eq!(
            Status::Review.menu(),
            &[MenuAction::Complete, MenuAction::Drop]
        );
        assert_eq!(
            Status::Question.menu(),
            &[MenuAction::Complete, MenuAction::Drop]
        );
        assert_eq!(Status::Running.menu(), &[]);
        assert_eq!(Status::Integrated.menu(), &[MenuAction::Remove]);
        assert_eq!(Status::Completed.menu(), &[MenuAction::Remove]);
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&Status::Question).unwrap(),
            r#""question""#
        );
        assert_eq!(
            serde_json::to_string(&Status::Completed).unwrap(),
            r#""completed""#
        );
    }

    #[test]
    fn stage_and_action_serialize_camel_case() {
        assert_eq!(serde_json::to_string(&Stage::Yours).unwrap(), r#""yours""#);
        assert_eq!(
            serde_json::to_string(&MenuAction::StartUnframed).unwrap(),
            r#""startUnframed""#
        );
        assert_eq!(
            serde_json::to_string(&MenuAction::Restart).unwrap(),
            r#""restart""#
        );
        assert_eq!(
            serde_json::to_string(&MenuAction::Complete).unwrap(),
            r#""complete""#
        );
    }
}
