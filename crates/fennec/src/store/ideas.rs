//! Append-only backlog events per project, folded into the current backlog.

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use backlog::{Backlog, BacklogEvent, Idea, fold};
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
    pub fn append_idea_event(
        &self,
        project_id: &str,
        event: &BacklogEvent,
    ) -> Result<(), StoreError> {
        let payload =
            serde_json::to_string(event).map_err(|err| StoreError::Json(err.to_string()))?;
        self.db.execute(
            "INSERT INTO project_events (project_id, timestamp, event_type, payload) VALUES (?1, ?2, ?3, ?4)",
            params![project_id, now_ms() as i64, event.kind(), payload],
        )?;
        Ok(())
    }

    pub fn backlog(&self, project_id: &str) -> Result<Backlog, StoreError> {
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
            let Ok(event) = serde_json::from_str::<BacklogEvent>(&payload) else {
                continue;
            };
            events.push((timestamp as u64, event));
        }

        Ok(fold(&events))
    }

    pub fn ideas(&self, project_id: &str) -> Result<Vec<Idea>, StoreError> {
        Ok(self.backlog(project_id)?.ideas)
    }

    pub fn open_queue_runs(&self) -> Result<Vec<String>, StoreError> {
        let mut open = Vec::new();
        for project in self.projects()? {
            if self
                .backlog(&project.id)?
                .last_run
                .is_some_and(|run| run.is_open())
            {
                open.push(project.id);
            }
        }
        Ok(open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backlog::IdeaState;

    #[test]
    fn idea_events_are_listed_per_project() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/tmp/p").unwrap();
        let other = store.project_for("/tmp/other").unwrap();

        store
            .append_idea_event(
                &project.id,
                &BacklogEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &project.id,
                &BacklogEvent::Framing {
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

    #[test]
    fn open_queue_runs_lists_only_open_runs() {
        let store = SqliteStore::in_memory().unwrap();
        let open = store.project_for("/tmp/open").unwrap();
        let closed = store.project_for("/tmp/closed").unwrap();

        store
            .append_idea_event(&open.id, &BacklogEvent::QueueStarted { autonomous: false })
            .unwrap();
        store
            .append_idea_event(
                &closed.id,
                &BacklogEvent::QueueStarted { autonomous: false },
            )
            .unwrap();
        store
            .append_idea_event(
                &closed.id,
                &BacklogEvent::QueueFinished {
                    reason: "done".into(),
                },
            )
            .unwrap();

        assert_eq!(store.open_queue_runs().unwrap(), vec![open.id]);
    }
}
