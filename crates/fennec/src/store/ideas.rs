//! Append-only idea events per project, folded into the current idea list.

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use crate::domain::idea::{Idea, IdeaEvent, fold};
use rusqlite::params;

pub(super) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS project_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS project_events_project_id_id ON project_events (project_id, id);
"#;

impl SqliteStore {
    pub fn append_idea_event(&self, project_id: &str, event: &IdeaEvent) -> Result<(), StoreError> {
        let payload =
            serde_json::to_string(event).map_err(|err| StoreError::Json(err.to_string()))?;
        self.db.execute(
            "INSERT INTO project_events (project_id, timestamp, event_type, payload) VALUES (?1, ?2, ?3, ?4)",
            params![project_id, now_ms() as i64, event.kind(), payload],
        )?;
        Ok(())
    }

    pub fn ideas(&self, project_id: &str) -> Result<Vec<Idea>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT timestamp, payload FROM project_events WHERE project_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let timestamp: i64 = row.get(0)?;
            let payload: String = row.get(1)?;
            Ok((timestamp, payload))
        })?;

        let mut events = Vec::new();
        for row in rows {
            let (timestamp, payload) = row?;
            // Skip payloads that fail to parse: a later version may add event types this build does not know.
            let Ok(event) = serde_json::from_str::<IdeaEvent>(&payload) else {
                continue;
            };
            events.push((timestamp as u64, event));
        }

        Ok(fold(&events))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::idea::IdeaState;

    #[test]
    fn idea_events_are_listed_per_project() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/tmp/p").unwrap();
        let other = store.project_for("/tmp/other").unwrap();

        store
            .append_idea_event(
                &project.id,
                &IdeaEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &project.id,
                &IdeaEvent::Framing {
                    idea_id: "i1".into(),
                    session_id: "s1".into(),
                },
            )
            .unwrap();

        let ideas = store.ideas(&project.id).unwrap();
        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].state, IdeaState::Framing);
        assert_eq!(ideas[0].session_id.as_deref(), Some("s1"));

        assert!(store.ideas(&other.id).unwrap().is_empty());
    }
}
