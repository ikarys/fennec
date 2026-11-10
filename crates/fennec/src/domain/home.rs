//! Recent-session ordering for the home page.

use crate::domain::project::SessionSummary;

pub fn recent_sessions(mut sessions: Vec<SessionSummary>, limit: usize) -> Vec<SessionSummary> {
    // Newest activity first; created_at only orders sessions updated together.
    sessions.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    sessions.truncate(limit);
    sessions
}

#[cfg(test)]
mod tests {
    use super::recent_sessions;
    use crate::domain::project::SessionSummary;

    fn session(id: &str, created_at: u64, updated_at: u64) -> SessionSummary {
        SessionSummary {
            id: id.into(),
            project_id: "p".into(),
            title: id.into(),
            created_at,
            updated_at,
            last_done: None,
            parent_id: None,
            awaiting_integration: false,
            linked_project_id: None,
        }
    }

    #[test]
    fn orders_by_updated_at_desc() {
        let out = recent_sessions(
            vec![
                session("a", 1, 10),
                session("b", 2, 20),
                session("c", 3, 30),
            ],
            10,
        );
        let ids: Vec<&str> = out.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["c", "b", "a"]);
    }

    #[test]
    fn tie_breaks_by_created_at_desc() {
        let out = recent_sessions(
            vec![
                session("a", 1, 10),
                session("b", 3, 10),
                session("c", 2, 10),
            ],
            10,
        );
        let ids: Vec<&str> = out.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn truncates_to_limit() {
        let out = recent_sessions(
            vec![session("a", 1, 1), session("b", 2, 2), session("c", 3, 3)],
            2,
        );
        let ids: Vec<&str> = out.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["c", "b"]);
    }
}
