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
    Frame,
    ResumeFraming,
    Start,
    Open,
    Answer,
    Review,
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
            Status::Integrated => Stage::Done,
        }
    }

    /// The single next action for this status.
    pub fn action(self) -> Action {
        match self {
            Status::Inbox => Action::Frame,
            Status::Framing => Action::ResumeFraming,
            Status::Queued => Action::Start,
            Status::Running => Action::Open,
            Status::Question => Action::Answer,
            Status::Review => Action::Review,
            Status::Halted | Status::Failed | Status::Integrated => Action::Open,
        }
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
    fn dropped_maps_to_inbox() {
        assert_eq!(status(&idea(IdeaState::Dropped, None), None), Status::Inbox);
    }

    #[test]
    fn stage_and_action_match_the_human_view() {
        assert_eq!(Status::Inbox.stage(), Stage::Drafts);
        assert_eq!(Status::Inbox.action(), Action::Frame);

        assert_eq!(Status::Framing.stage(), Stage::Drafts);
        assert_eq!(Status::Framing.action(), Action::ResumeFraming);

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
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&Status::Question).unwrap(),
            r#""question""#
        );
    }

    #[test]
    fn stage_and_action_serialize_camel_case() {
        assert_eq!(serde_json::to_string(&Stage::Yours).unwrap(), r#""yours""#);
        assert_eq!(
            serde_json::to_string(&Action::ResumeFraming).unwrap(),
            r#""resumeFraming""#
        );
    }
}
