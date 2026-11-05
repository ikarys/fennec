//! The events of a project's sessions, read for the indicators view.

use super::{SqliteStore, StoreError};
use crate::domain::event::Event;
use rusqlite::params_from_iter;

/// One event of any session of a project, with its session and time.
pub struct ProjectEvent {
    pub session_id: String,
    pub at_ms: u64,
    pub event: Event,
}

impl SqliteStore {
    /// The events of the given wire types across a project's sessions, in log order; undecodable payloads skipped.
    pub fn project_events(
        &self,
        project_id: &str,
        types: &[&str],
    ) -> std::result::Result<Vec<ProjectEvent>, StoreError> {
        if types.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = (2..=types.len() + 1)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT e.session_id, e.timestamp, e.payload
             FROM events e
             JOIN sessions s ON s.id = e.session_id
             WHERE s.project_id = ?1 AND e.event_type IN ({placeholders})
             ORDER BY e.id"
        );
        let params = std::iter::once(project_id).chain(types.iter().copied());
        let mut stmt = self.db.prepare(&query)?;
        let rows = stmt.query_map(params_from_iter(params), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut events = Vec::new();
        for row in rows {
            let (session_id, timestamp, payload) = row?;
            // The indicators only trust decodable recorded events.
            let Ok(event) = serde_json::from_str::<Event>(&payload) else {
                continue;
            };
            events.push(ProjectEvent {
                session_id,
                at_ms: timestamp.max(0) as u64,
                event,
            });
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::super::SqliteStore;
    use crate::agent::ports::EventLog;
    use crate::domain::event::Event;

    fn integration_event() -> Event {
        Event::IntegrationApplied {
            branch: "fennec/a".into(),
            base: "main".into(),
            commits: vec!["abc".into()],
            head: "def".into(),
            message: String::new(),
            acknowledged: vec!["sensitiveTouched".into()],
            protected_tests: Vec::new(),
        }
    }

    fn understanding_event() -> Event {
        Event::UnderstandingStarted {
            session_id: "s".into(),
            head: "def".into(),
        }
    }

    #[test]
    fn project_events_filters_project_and_types() {
        let store = SqliteStore::in_memory().unwrap();
        let a = store.project_for("/tmp/a").unwrap();
        let b = store.project_for("/tmp/b").unwrap();
        let session_a = store.create_session(&a, "t").unwrap();
        let session_b = store.create_session(&b, "t").unwrap();

        EventLog::append(&store, &session_a, &integration_event()).unwrap();
        EventLog::append(&store, &session_a, &understanding_event()).unwrap();
        EventLog::append(&store, &session_b, &understanding_event()).unwrap();

        let events = store
            .project_events(&a.id, &["understanding.started"])
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].session_id, session_a);
        assert!(matches!(
            events[0].event,
            Event::UnderstandingStarted { .. }
        ));
        assert!(store.project_events(&a.id, &[]).unwrap().is_empty());
    }
}
