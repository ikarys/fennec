//! Projects (a working directory) and their sessions, as listed to the human.

use super::event::DoneReason;
use serde::Serialize;

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
    fn waiting_for_user_keeps_openfox_wire_name() {
        let wire = serde_json::to_value(SessionState::WaitingForUser).unwrap();
        assert_eq!(wire, "waiting_for_user");
    }
}
