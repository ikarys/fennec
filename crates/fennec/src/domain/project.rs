//! Projects (a working directory) and their sessions, as listed to the human.

use super::event::DoneReason;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub workdir: String,
}

pub struct SessionSummary {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_done: Option<DoneReason>,
    pub parent_id: Option<String>,
    /// A workflow ended `done` and nothing integrated or discarded its work since.
    pub awaiting_integration: bool,
    /// The project a chat is linked to, for grouping only; None for an unlinked chat or a project session.
    pub linked_project_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Idle,
    Running,
    Complete,
    Stopped,
    Error,
    Truncated,
    #[serde(rename = "waiting_for_user")]
    WaitingForUser,
}

impl SessionSummary {
    /// Whether a turn runs is known by whoever runs turns, not by the log.
    pub fn state(&self, running: bool) -> SessionState {
        match (running, self.last_done) {
            (true, _) => SessionState::Running,
            (false, None) => SessionState::Idle,
            (false, Some(DoneReason::Complete)) => SessionState::Complete,
            (false, Some(DoneReason::Stopped)) => SessionState::Stopped,
            (false, Some(DoneReason::Error)) => SessionState::Error,
            (false, Some(DoneReason::Truncated)) => SessionState::Truncated,
            (false, Some(DoneReason::WaitingForUser)) => SessionState::WaitingForUser,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SessionState;

    #[test]
    fn waiting_for_user_keeps_its_wire_name() {
        let wire = serde_json::to_value(SessionState::WaitingForUser).unwrap();
        assert_eq!(wire, "waiting_for_user");
    }
}
