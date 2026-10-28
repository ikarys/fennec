//! The model calls a log holds, for the usage ledger: who owns a session and
//! every recorded `message.done` with stats.
use super::{SqliteStore, StoreError};
use crate::domain::stats::MessageStats;
use rusqlite::params;

#[derive(Debug, PartialEq)]
pub struct SessionOwner {
    pub project_id: String,
    pub project_name: String,
    pub parent_id: Option<String>,
}

pub struct RecordedCall {
    pub session_id: String,
    pub seq: u64,
    pub message_id: String,
    pub at_ms: i64,
    pub owner: SessionOwner,
    pub stats: MessageStats,
}

impl SqliteStore {
    /// The project that owns a session, plus its parent if it is a child.
    pub fn session_owner(&self, session_id: &str) -> Result<Option<SessionOwner>, StoreError> {
        let sql = "SELECT p.id, p.name, s.parent_id \
                   FROM sessions s \
                   JOIN projects p ON p.id = s.project_id \
                   WHERE s.id = ?1";
        let mut stmt = self.db.prepare_cached(sql)?;
        match stmt.query_row(params![session_id], |row| {
            Ok(SessionOwner {
                project_id: row.get(0)?,
                project_name: row.get(1)?,
                parent_id: row.get(2)?,
            })
        }) {
            Ok(owner) => Ok(Some(owner)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Oldest first. A row whose stats cannot be parsed is skipped.
    pub fn recorded_calls(&self) -> Result<Vec<RecordedCall>, StoreError> {
        let sql = "SELECT e.session_id, e.seq, e.timestamp, p.id, p.name, s.parent_id, \
                   json_extract(e.payload, '$.data.stats'), \
                   json_extract(e.payload, '$.data.messageId') \
                   FROM events e \
                   JOIN sessions s ON s.id = e.session_id \
                   JOIN projects p ON p.id = s.project_id \
                   WHERE e.event_type = 'message.done' \
                     AND json_extract(e.payload, '$.data.stats') IS NOT NULL \
                     AND (json_extract(e.payload, '$.data.partial') IS NULL \
                          OR json_extract(e.payload, '$.data.partial') = 0) \
                   ORDER BY e.id";
        let mut stmt = self.db.prepare_cached(sql)?;
        let rows = stmt.query_map(params![], |row| {
            let stats_json: String = row.get(6)?;
            let Ok(stats) = serde_json::from_str::<MessageStats>(&stats_json) else {
                return Ok(None);
            };
            Ok(Some(RecordedCall {
                session_id: row.get(0)?,
                seq: row.get::<_, i64>(1)? as u64,
                message_id: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                at_ms: row.get(2)?,
                owner: SessionOwner {
                    project_id: row.get(3)?,
                    project_name: row.get(4)?,
                    parent_id: row.get(5)?,
                },
                stats,
            }))
        })?;
        rows.filter_map(Result::transpose)
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::EventLog;
    use crate::domain::event::Event;

    fn stats() -> MessageStats {
        MessageStats {
            model: "test-model".into(),
            total_time: 2.0,
            prefill_tokens: 100,
            cached_prompt_tokens: 20,
            generation_tokens: 30,
            prefill_speed: 50.0,
            generation_speed: 15.0,
        }
    }

    #[test]
    fn recorded_calls_returns_complete_stats_with_owner() {
        let store = SqliteStore::in_memory().expect("store");
        let project = store.project_for("/tmp/p").expect("project");
        let session = store.create_session(&project, "t").expect("session");

        store
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "m1".into(),
                    stats: Some(stats()),
                    partial: false,
                },
            )
            .expect("append");

        let calls = store.recorded_calls().expect("recorded_calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].session_id, session);
        assert_eq!(calls[0].seq, 2); // event 1 is the session's creation
        assert_eq!(calls[0].message_id, "m1");
        assert_eq!(calls[0].owner.project_id, project.id);
        assert_eq!(calls[0].owner.project_name, project.name);
        assert_eq!(calls[0].owner.parent_id, None);
        assert_eq!(calls[0].stats.model, "test-model");
    }

    #[test]
    fn recorded_calls_skips_partial_and_statsless() {
        let store = SqliteStore::in_memory().expect("store");
        let project = store.project_for("/tmp/p").expect("project");
        let session = store.create_session(&project, "t").expect("session");

        store
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "complete".into(),
                    stats: Some(stats()),
                    partial: false,
                },
            )
            .expect("append");
        store
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "partial".into(),
                    stats: Some(stats()),
                    partial: true,
                },
            )
            .expect("append");
        store
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "no-stats".into(),
                    stats: None,
                    partial: false,
                },
            )
            .expect("append");

        let calls = store.recorded_calls().expect("recorded_calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].seq, 2);
    }

    #[test]
    fn session_owner_reports_child_parent() {
        let store = SqliteStore::in_memory().expect("store");
        let project = store.project_for("/tmp/p").expect("project");
        let parent = store.create_session(&project, "parent").expect("parent");
        let child = store.create_child_session(&parent, "child").expect("child");

        let owner = store.session_owner(&child).expect("owner").expect("owner");
        assert_eq!(owner.project_id, project.id);
        assert_eq!(owner.project_name, project.name);
        assert_eq!(owner.parent_id.as_deref(), Some(parent.as_str()));

        assert_eq!(store.session_owner("missing").expect("owner"), None);
    }
}
