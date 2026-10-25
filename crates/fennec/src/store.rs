//! Storage adapter: openfox's SQLite store (`db/index.ts`, `events/store.ts`),
//! same `projects`/`sessions`/`events` schema minus the columns fennec does
//! not use yet. Implements the agent's `EventLog`.

use crate::agent::ports::EventLog;
use crate::domain::clock::now_ms;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;
use crate::domain::project::{Project, SessionSummary};
use rusqlite::{Connection, OptionalExtension, params};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub struct StoreError(rusqlite::Error);

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> StoreError {
        StoreError(e)
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "event store: {}", self.0)
    }
}

impl From<StoreError> for String {
    fn from(e: StoreError) -> String {
        e.to_string()
    }
}

type Result<T> = std::result::Result<T, StoreError>;

pub struct SqliteStore {
    db: Connection,
}

const SCHEMA: &str = "PRAGMA foreign_keys = ON;
     CREATE TABLE IF NOT EXISTS projects (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       workdir TEXT NOT NULL UNIQUE,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE TABLE IF NOT EXISTS sessions (
       id TEXT PRIMARY KEY,
       project_id TEXT NOT NULL,
       workdir TEXT NOT NULL,
       title TEXT,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );
     CREATE TABLE IF NOT EXISTS events (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       session_id TEXT NOT NULL,
       seq INTEGER NOT NULL,
       timestamp INTEGER NOT NULL,
       event_type TEXT NOT NULL,
       payload TEXT NOT NULL,
       UNIQUE(session_id, seq),
       FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
     );
     CREATE INDEX IF NOT EXISTS idx_events_session_seq ON events(session_id, seq);";

impl SqliteStore {
    pub fn open(path: &Path) -> Result<SqliteStore> {
        let db = Connection::open(path)?;
        // WAL: the web server reads a log while a turn's thread appends to it.
        db.execute_batch("PRAGMA journal_mode = WAL;")?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        SqliteStore::init(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<SqliteStore> {
        SqliteStore::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<SqliteStore> {
        db.execute_batch(SCHEMA)?;
        Ok(SqliteStore { db })
    }

    /// The project for `workdir`, created on first use (openfox does the same
    /// when a directory is opened).
    pub fn project_for(&self, workdir: &str) -> Result<Project> {
        let name = Path::new(workdir)
            .file_name()
            .map_or_else(|| workdir.to_string(), |n| n.to_string_lossy().into_owned());
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT OR IGNORE INTO projects (id, name, workdir, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![new_id(), name, workdir, now],
        )?;
        Ok(self.db.query_row(
            "SELECT id, name, workdir FROM projects WHERE workdir = ?1",
            [workdir],
            project_row,
        )?)
    }

    pub fn project(&self, id: &str) -> Result<Option<Project>> {
        Ok(self
            .db
            .query_row(
                "SELECT id, name, workdir FROM projects WHERE id = ?1",
                [id],
                project_row,
            )
            .optional()?)
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .db
            .prepare("SELECT id, name, workdir FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], project_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn create_session(&self, project: &Project, title: &str) -> Result<String> {
        let id = new_id();
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT INTO sessions (id, project_id, workdir, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, project.id, project.workdir, title, now],
        )?;
        self.insert_event(
            &id,
            &Event::SessionInitialized {
                workdir: project.workdir.clone(),
            },
        )?;
        Ok(id)
    }

    /// Raw stored payloads after `seq`, for streaming them as they are.
    pub fn events_after(&self, session_id: &str, seq: u64) -> Result<Vec<(u64, String)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT seq, payload FROM events WHERE session_id = ?1 AND seq > ?2 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![session_id, seq as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Sessions, newest first; `project_id` filters when given.
    pub fn sessions(&self, project_id: Option<&str>) -> Result<Vec<SessionSummary>> {
        let mut stmt = self.db.prepare(
            "SELECT s.id, s.project_id, COALESCE(s.title, ''), CAST(s.created_at AS INTEGER),
               COALESCE((SELECT MAX(timestamp) FROM events e WHERE e.session_id = s.id), CAST(s.created_at AS INTEGER)),
               (SELECT json_extract(payload, '$.data.reason') FROM events e
                  WHERE e.session_id = s.id AND e.event_type = 'chat.done' ORDER BY seq DESC LIMIT 1)
             FROM sessions s WHERE ?1 IS NULL OR s.project_id = ?1
             ORDER BY CAST(s.created_at AS INTEGER) DESC",
        )?;
        let rows = stmt.query_map([project_id], |r| {
            let last_done: Option<String> = r.get(5)?;
            Ok(SessionSummary {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                created_at: r.get::<_, i64>(3)? as u64,
                updated_at: r.get::<_, i64>(4)? as u64,
                last_done: last_done.and_then(|s| {
                    serde_json::from_value::<DoneReason>(serde_json::Value::String(s)).ok()
                }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn session(&self, id: &str) -> Result<Option<SessionSummary>> {
        Ok(self.sessions(None)?.into_iter().find(|s| s.id == id))
    }

    fn insert_event(&self, session_id: &str, event: &Event) -> Result<u64> {
        let payload = serde_json::to_string(event).expect("events always serialize");
        Ok(self.db.query_row(
            "INSERT INTO events (session_id, seq, timestamp, event_type, payload)
             VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM events WHERE session_id = ?1), ?2, ?3, ?4)
             RETURNING seq",
            params![session_id, now_ms() as i64, event.kind(), payload],
            |row| row.get::<_, i64>(0).map(|s| s as u64),
        )?)
    }
}

impl EventLog for SqliteStore {
    fn append(&self, session_id: &str, event: &Event) -> std::result::Result<u64, String> {
        Ok(self.insert_event(session_id, event)?)
    }

    fn events(&self, session_id: &str) -> std::result::Result<Vec<Event>, String> {
        let mut out = Vec::new();
        for (_, payload) in self.events_after(session_id, 0)? {
            // A payload this version cannot read is skipped, not fatal: the
            // log may hold event types ported later.
            if let Ok(event) = serde_json::from_str(&payload) {
                out.push(event);
            }
        }
        Ok(out)
    }
}

fn project_row(r: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        workdir: r.get(2)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_round_trip_in_order_and_sessions_report_how_they_ended() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        assert_eq!(project.name, "w");
        assert_eq!(store.project_for("/w").unwrap().id, project.id);
        let sid = store.create_session(&project, "t").unwrap();
        let ev = Event::MessageDelta {
            message_id: "m".into(),
            content: "hi".into(),
        };
        assert_eq!(store.append(&sid, &ev).unwrap(), 2);
        let all = store.events(&sid).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1], ev);
        let wire = serde_json::to_value(&ev).unwrap();
        assert_eq!(wire["type"], "message.delta");
        assert_eq!(wire["data"]["messageId"], "m");
        store
            .append(
                &sid,
                &Event::ChatDone {
                    message_id: "m".into(),
                    reason: DoneReason::Stopped,
                },
            )
            .unwrap();
        let row = store.session(&sid).unwrap().unwrap();
        assert_eq!(row.last_done, Some(DoneReason::Stopped));
        assert_eq!(store.events_after(&sid, 2).unwrap().len(), 1);
    }
}
