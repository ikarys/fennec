//! Storage adapter: the SQLite store, `projects`/`sessions`/`events`. The
//! schema is frozen: stored logs depend on it. Implements the agent's `EventLog`.

mod budgets;
pub mod calls;
mod ideas;
mod plugins;
mod project_events;
mod settings;

use crate::agent::ports::EventLog;
use crate::chat;
use crate::domain::clock::now_ms;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;
use crate::domain::project::{Project, SessionSummary};
use rusqlite::{Connection, OptionalExtension, params};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Json(String),
    NoParent(String),
    /// An API key in clear: it is sealed before it may be stored.
    PlainSecret,
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> StoreError {
        StoreError::Sqlite(e)
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "event store: {e}"),
            StoreError::Json(e) => write!(f, "settings: {e}"),
            StoreError::NoParent(id) => write!(f, "no such session: {id}"),
            StoreError::PlainSecret => {
                write!(f, "settings: an API key must be sealed before it is stored")
            }
        }
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
       updated_at TEXT NOT NULL,
       favorite INTEGER NOT NULL DEFAULT 0
     );
     CREATE TABLE IF NOT EXISTS sessions (
       id TEXT PRIMARY KEY,
       project_id TEXT NOT NULL,
       workdir TEXT NOT NULL,
       title TEXT,
       parent_id TEXT,
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
     CREATE INDEX IF NOT EXISTS idx_events_session_seq ON events(session_id, seq);
     CREATE TABLE IF NOT EXISTS settings (
       key TEXT PRIMARY KEY,
       value TEXT NOT NULL,
       updated_at TEXT NOT NULL
     );";

impl SqliteStore {
    pub fn open(path: &Path) -> Result<SqliteStore> {
        let db = Connection::open(path)?;
        // WAL: the web server reads a log while a turn's thread appends to it.
        // NORMAL is durable in WAL mode except across a power loss, where
        // the last events may be lost: one fsync per checkpoint instead of
        // one per event.
        db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        SqliteStore::init(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<SqliteStore> {
        SqliteStore::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<SqliteStore> {
        db.execute_batch(SCHEMA)?;
        db.execute_batch(ideas::SCHEMA)?;
        db.execute_batch(plugins::SCHEMA)?;
        // Old databases predate child sessions; add the column then.
        let has_parent = db
            .prepare("PRAGMA table_info(sessions)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .into_iter()
            .any(|name| name.is_ok_and(|n| n == "parent_id"));
        if !has_parent {
            db.execute("ALTER TABLE sessions ADD COLUMN parent_id TEXT", [])?;
        }
        // Old databases predate favourites; add the column then.
        let has_favorite = db
            .prepare("PRAGMA table_info(projects)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .into_iter()
            .any(|name| name.is_ok_and(|n| n == "favorite"));
        if !has_favorite {
            db.execute(
                "ALTER TABLE projects ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        Ok(SqliteStore { db })
    }

    /// The project for `workdir`, created on first use.
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

    /// The chat's scratch project, created on first use: a fixed id and a
    /// scratch workdir, so a session can run without a repository.
    pub fn chat_project(&self, workdir: &str) -> Result<Project> {
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT OR IGNORE INTO projects (id, name, workdir, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![chat::ID, chat::NAME, workdir, now],
        )?;
        Ok(self.db.query_row(
            "SELECT id, name, workdir FROM projects WHERE id = ?1",
            [chat::ID],
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

    /// Favourites first, then most recently active (the newest event of its
    /// top-level sessions, else their creation), then by name.
    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.db.prepare(
            "SELECT p.id, p.name, p.workdir FROM projects p
             ORDER BY p.favorite DESC,
                      (SELECT MAX(COALESCE(
                          (SELECT MAX(e.timestamp) FROM events e WHERE e.session_id = s.id),
                          CAST(s.created_at AS INTEGER)))
                       FROM sessions s WHERE s.project_id = p.id AND s.parent_id IS NULL) DESC,
                      p.name",
        )?;
        let rows = stmt.query_map([], project_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Pin or unpin a project. Returns whether a row matched.
    pub fn set_favorite(&self, project_id: &str, favorite: bool) -> Result<bool> {
        let rows = self.db.execute(
            "UPDATE projects SET favorite = ?1 WHERE id = ?2",
            params![favorite as i64, project_id],
        )?;
        Ok(rows > 0)
    }

    /// The ids of pinned projects.
    pub fn favorite_projects(&self) -> Result<std::collections::HashSet<String>> {
        let mut stmt = self
            .db
            .prepare("SELECT id FROM projects WHERE favorite = 1")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn create_session(&self, project: &Project, title: &str) -> Result<String> {
        self.insert_session(&project.id, &project.workdir, title, None)
    }

    /// A child session of `parent_id`: same project and workdir, hidden from
    /// the lists.
    pub fn create_child_session(&self, parent_id: &str, title: &str) -> Result<String> {
        let (project_id, workdir) = self
            .db
            .query_row(
                "SELECT project_id, workdir FROM sessions WHERE id = ?1",
                [parent_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(StoreError::NoParent(parent_id.to_string()))?;
        self.insert_session(&project_id, &workdir, title, Some(parent_id))
    }

    /// The generated title replaces the placeholder; `updated_at` is left
    /// untouched so the list order does not jump.
    pub fn rename_session(&self, id: &str, title: &str) -> Result<()> {
        self.db.execute(
            "UPDATE sessions SET title = ?1 WHERE id = ?2",
            params![title, id],
        )?;
        Ok(())
    }

    fn insert_session(
        &self,
        project_id: &str,
        workdir: &str,
        title: &str,
        parent_id: Option<&str>,
    ) -> Result<String> {
        let id = new_id();
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT INTO sessions (id, project_id, workdir, title, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![id, project_id, workdir, title, parent_id, now],
        )?;
        self.insert_event(
            &id,
            &Event::SessionInitialized {
                workdir: workdir.to_string(),
            },
        )?;
        Ok(id)
    }

    fn decode_events(rows: Vec<(u64, String)>) -> Vec<(u64, Event)> {
        rows.into_iter()
            .filter_map(|(seq, payload)| {
                // A payload this version cannot read is skipped, not fatal,
                // as in `events`.
                serde_json::from_str(&payload)
                    .ok()
                    .map(|event| (seq, event))
            })
            .collect()
    }

    /// One page of the log before `seq`, ascending, with sequence numbers: at
    /// least the last `limit` events, extended back to the start of the turn
    /// they begin in, so the UI never folds half a message or misses that a
    /// turn is running. A page can exceed `limit` when a turn is long.
    pub fn events_before(
        &self,
        session_id: &str,
        seq: u64,
        limit: u64,
    ) -> Result<Vec<(u64, Event)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT seq, payload FROM events
                 WHERE session_id = ?1 AND seq < ?2 AND seq >= COALESCE((
                   SELECT MAX(t.seq) FROM events t
                   WHERE t.session_id = ?1 AND t.event_type = 'running.changed'
                     AND json_extract(t.payload, '$.data.isRunning') = 1
                     AND t.seq <= (
                       SELECT MIN(seq) FROM (
                         SELECT seq FROM events
                         WHERE session_id = ?1 AND seq < ?2
                         ORDER BY seq DESC LIMIT ?3
                       )
                     )
                 ), 1)
                 ORDER BY seq",
        )?;
        // `u64::MAX` (no `before`) would cast to a negative i64 and match
        // nothing; the log's seqs fit in i64.
        let bound = seq.min(i64::MAX as u64) as i64;
        let rows = stmt.query_map(params![session_id, bound, limit as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?;
        Ok(Self::decode_events(
            rows.collect::<rusqlite::Result<Vec<(u64, String)>>>()?,
        ))
    }

    /// The `workflow.*` events of the latest run started before `seq`
    /// (inclusive of its `workflow.started`), ascending; empty when there is
    /// no `workflow.started` before `seq`.
    pub fn workflow_run_before(&self, session_id: &str, seq: u64) -> Result<Vec<(u64, Event)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT seq, payload FROM events
                 WHERE session_id = ?1
                   AND seq < ?2
                   AND event_type LIKE 'workflow.%'
                   AND seq >= (
                     SELECT MAX(seq) FROM events
                     WHERE session_id = ?1
                       AND event_type = 'workflow.started'
                       AND seq < ?2
                   )
                 ORDER BY seq",
        )?;
        // `u64::MAX` (no `before`) would cast to a negative i64 and match
        // nothing; the log's seqs fit in i64.
        let bound = seq.min(i64::MAX as u64) as i64;
        let rows = stmt.query_map(params![session_id, bound], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?;
        Ok(Self::decode_events(
            rows.collect::<rusqlite::Result<Vec<(u64, String)>>>()?,
        ))
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

    /// The session's newest sequence number, 0 when the log is empty. The log
    /// is append-only, so it watermarks a fold: an unchanged max seq means an
    /// unchanged log.
    pub fn last_seq(&self, session_id: &str) -> Result<u64> {
        Ok(self.db.query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM events WHERE session_id = ?1",
            [session_id],
            |r| r.get::<_, i64>(0),
        )? as u64)
    }

    /// The session's `running.changed` events at/after `since_ms`, in log order.
    pub fn running_changes(&self, session_id: &str, since_ms: u64) -> Result<Vec<(u64, bool)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT timestamp, payload FROM events WHERE session_id = ?1 AND event_type = 'running.changed' AND timestamp >= ?2 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![session_id, since_ms as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<(u64, String)>>>()?
            .into_iter()
            .filter_map(|(timestamp, payload)| {
                serde_json::from_str::<serde_json::Value>(&payload)
                    .ok()
                    .and_then(|value| value["data"]["isRunning"].as_bool())
                    .map(|is_running| (timestamp, is_running))
            })
            .collect())
    }

    /// Top-level sessions, newest first; `project_id` filters when given.
    /// Child sessions are hidden from the lists.
    pub fn sessions(&self, project_id: Option<&str>) -> Result<Vec<SessionSummary>> {
        let mut stmt = self.db.prepare(&format!(
            "{SESSION_SELECT} WHERE s.parent_id IS NULL AND (?1 IS NULL OR s.project_id = ?1)
             ORDER BY CAST(s.created_at AS INTEGER) DESC"
        ))?;
        let rows = stmt.query_map([project_id], session_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Any session by id, children included.
    pub fn session(&self, id: &str) -> Result<Option<SessionSummary>> {
        let mut stmt = self
            .db
            .prepare(&format!("{SESSION_SELECT} WHERE s.id = ?1"))?;
        let mut rows = stmt.query_map([id], session_row)?;
        Ok(rows.next().transpose()?)
    }

    /// The last agent a session switched to (`mode.changed`), or None. A
    /// targeted read, not the full log: listing a project must not deserialize
    /// every session's events.
    pub fn last_mode(&self, session_id: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row(
                "SELECT json_extract(payload, '$.data.mode') FROM events
                 WHERE session_id = ?1 AND event_type = 'mode.changed'
                 ORDER BY seq DESC LIMIT 1",
                [session_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// The id followed by all its descendants, children of children included.
    pub fn session_tree(&self, id: &str) -> Result<Vec<String>> {
        let mut stmt = self.db.prepare_cached(
            "WITH RECURSIVE tree(id) AS (
               SELECT id FROM sessions WHERE id = ?1
               UNION ALL
               SELECT s.id FROM sessions s JOIN tree t ON s.parent_id = t.id
             )
             SELECT id FROM tree",
        )?;
        let rows = stmt.query_map(params![id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    /// One transaction keeps a batch of deletes atomic with their events.
    pub fn delete_sessions(&self, ids: &[String]) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        for id in ids {
            tx.execute(
                "DELETE FROM events WHERE session_id = ?1",
                params![id.as_str()],
            )?;
            tx.execute("DELETE FROM sessions WHERE id = ?1", params![id.as_str()])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Deletes a project and everything attached to it: its sessions with
    /// their events, its ideas backlog events, and the project row.
    pub fn delete_project(&self, id: &str) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM events WHERE session_id IN (SELECT id FROM sessions WHERE project_id = ?1)",
            params![id],
        )?;
        tx.execute("DELETE FROM sessions WHERE project_id = ?1", params![id])?;
        // The cascade would do this too, but not if foreign keys are off on
        // this connection.
        tx.execute(
            "DELETE FROM project_events WHERE project_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        // The budget row is a setting keyed by the project id.
        tx.execute(
            "DELETE FROM settings WHERE key = 'budget:' || ?1",
            params![id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Ends the turns a crash left open: sessions whose last `running.changed`
    /// says running get `isRunning: false`, so the UI stops waiting on a turn
    /// no process runs. Called at server start, before any turn exists.
    /// Returns how many were closed.
    // ponytail: a CLI turn running in another process at that moment is closed too.
    pub fn close_dangling_turns(&self) -> Result<usize> {
        let ids = self
            .db
            .prepare(
                "SELECT e.session_id FROM events e
                 WHERE e.event_type = 'running.changed'
                   AND json_extract(e.payload, '$.data.isRunning') = 1
                   AND e.seq = (SELECT MAX(seq) FROM events
                                WHERE session_id = e.session_id AND event_type = 'running.changed')",
            )?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        for id in &ids {
            self.end_cut_turn(id)?;
        }
        Ok(ids.len())
    }

    /// Ends a turn that was cut (a crash, a panic in its thread) like any other.
    pub fn end_cut_turn(&self, session_id: &str) -> Result<()> {
        // The cut turn ends like any other, with a done: otherwise the last
        // done in the log is an earlier one (a workflow's human node waiting
        // for the user), and the run looks waiting instead of interrupted.
        let message_id: String = self.db.query_row(
            "SELECT COALESCE(MAX(json_extract(payload, '$.data.messageId')), '')
             FROM events WHERE session_id = ?1 AND seq = (SELECT MAX(seq) FROM events
                 WHERE session_id = ?1 AND event_type = 'message.start')",
            params![session_id],
            |r| r.get(0),
        )?;
        self.insert_event(
            session_id,
            &Event::ChatDone {
                message_id,
                reason: DoneReason::Error,
                duration_ms: None,
            },
        )?;
        self.insert_event(session_id, &Event::RunningChanged { is_running: false })?;
        Ok(())
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

const SESSION_SELECT: &str = "SELECT s.id, s.project_id, COALESCE(s.title, ''), CAST(s.created_at AS INTEGER),
    COALESCE((SELECT MAX(timestamp) FROM events e WHERE e.session_id = s.id), CAST(s.created_at AS INTEGER)),
    (SELECT json_extract(payload, '$.data.reason') FROM events e
       WHERE e.session_id = s.id AND e.event_type = 'chat.done' ORDER BY seq DESC LIMIT 1),
    s.parent_id,
    COALESCE((SELECT e.event_type = 'workflow.finished' AND json_extract(e.payload, '$.data.outcome') = 'done'
       FROM events e WHERE e.session_id = s.id
         AND e.event_type IN ('workflow.finished', 'integration.applied', 'worktree.discarded')
       ORDER BY seq DESC LIMIT 1), 0)
  FROM sessions s";

fn session_row(r: &rusqlite::Row) -> rusqlite::Result<SessionSummary> {
    let last_done: Option<String> = r.get(5)?;
    Ok(SessionSummary {
        id: r.get(0)?,
        project_id: r.get(1)?,
        title: r.get(2)?,
        created_at: r.get::<_, i64>(3)? as u64,
        updated_at: r.get::<_, i64>(4)? as u64,
        last_done: last_done
            .and_then(|s| serde_json::from_value::<DoneReason>(serde_json::Value::String(s)).ok()),
        parent_id: r.get(6)?,
        awaiting_integration: r.get::<_, i64>(7)? != 0,
    })
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
    use crate::domain::workflow::run::RunOutcome;

    #[test]
    fn projects_list_the_most_recently_active_first() {
        let store = SqliteStore::in_memory().unwrap();
        let idle = store.project_for("/a").unwrap();
        let busy = store.project_for("/b").unwrap();
        store.project_for("/c").unwrap();
        store.create_session(&idle, "t").unwrap();
        let sid = store.create_session(&busy, "t").unwrap();
        let later = store.session(&sid).unwrap().unwrap().updated_at + 1;
        store
            .db
            .execute(
                "INSERT INTO events (session_id, seq, timestamp, event_type, payload) VALUES (?1, 99, ?2, 'x', '{}')",
                rusqlite::params![sid, later as i64],
            )
            .unwrap();
        let workdirs: Vec<_> = store
            .projects()
            .unwrap()
            .into_iter()
            .map(|p| p.workdir)
            .collect();
        assert_eq!(workdirs, ["/b", "/a", "/c"]);
    }

    #[test]
    fn favourite_projects_come_first_and_unpinning_restores_order() {
        let store = SqliteStore::in_memory().unwrap();
        let idle = store.project_for("/a").unwrap();
        let busy = store.project_for("/b").unwrap();
        store.project_for("/c").unwrap();
        store.create_session(&idle, "t").unwrap();
        let sid = store.create_session(&busy, "t").unwrap();
        let later = store.session(&sid).unwrap().unwrap().updated_at + 1;
        store
            .db
            .execute(
                "INSERT INTO events (session_id, seq, timestamp, event_type, payload) VALUES (?1, 99, ?2, 'x', '{}')",
                rusqlite::params![sid, later as i64],
            )
            .unwrap();

        assert!(store.set_favorite(&idle.id, true).unwrap());
        let workdirs: Vec<_> = store
            .projects()
            .unwrap()
            .into_iter()
            .map(|p| p.workdir)
            .collect();
        assert_eq!(workdirs, ["/a", "/b", "/c"]);

        assert!(store.set_favorite(&idle.id, false).unwrap());
        let workdirs: Vec<_> = store
            .projects()
            .unwrap()
            .into_iter()
            .map(|p| p.workdir)
            .collect();
        assert_eq!(workdirs, ["/b", "/a", "/c"]);

        assert!(!store.set_favorite("nope", true).unwrap());
    }

    #[test]
    fn chat_project_creates_a_fixed_row_and_is_idempotent() {
        let store = SqliteStore::in_memory().unwrap();
        let chat = store.chat_project("/scratch/chat").unwrap();
        assert_eq!(chat.id, chat::ID);
        assert_eq!(chat.name, chat::NAME);
        assert_eq!(chat.workdir, "/scratch/chat");
        // A second call returns the same row, not a duplicate.
        assert_eq!(store.chat_project("/scratch/chat").unwrap().id, chat.id);
        // It is reachable by id and marked as the chat.
        let by_id = store.project(chat::ID).unwrap().unwrap();
        assert!(chat::is_chat(&by_id));
    }

    #[test]
    fn a_finished_workflow_awaits_integration_until_discarded() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "t").unwrap();
        assert!(!store.session(&sid).unwrap().unwrap().awaiting_integration);
        let finished = Event::WorkflowFinished {
            run_id: "r".into(),
            outcome: RunOutcome::Done,
            reason: None,
            stats: None,
        };
        store.append(&sid, &finished).unwrap();
        assert!(store.session(&sid).unwrap().unwrap().awaiting_integration);
        let discarded = Event::WorktreeDiscarded {
            branch: "b".into(),
            commits: vec![],
        };
        store.append(&sid, &discarded).unwrap();
        assert!(!store.session(&sid).unwrap().unwrap().awaiting_integration);
    }

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
            tokens_per_second: None,
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
                    duration_ms: None,
                },
            )
            .unwrap();
        let row = store.session(&sid).unwrap().unwrap();
        assert_eq!(row.last_done, Some(DoneReason::Stopped));
        assert_eq!(store.events_after(&sid, 2).unwrap().len(), 1);
    }

    #[test]
    fn last_seq_tracks_the_newest_event() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "s").unwrap();
        // A new session holds its init event at seq 1.
        assert_eq!(store.last_seq(&sid).unwrap(), 1);
        store
            .append(&sid, &Event::RunningChanged { is_running: true })
            .unwrap();
        store
            .append(&sid, &Event::RunningChanged { is_running: false })
            .unwrap();
        assert_eq!(store.last_seq(&sid).unwrap(), 3);
    }

    #[test]
    fn close_dangling_turns_ends_only_open_turns() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let open = store.create_session(&project, "open").unwrap();
        let ended = store.create_session(&project, "ended").unwrap();
        store
            .append(&open, &Event::RunningChanged { is_running: true })
            .unwrap();
        for running in [true, false] {
            store
                .append(
                    &ended,
                    &Event::RunningChanged {
                        is_running: running,
                    },
                )
                .unwrap();
        }

        assert_eq!(store.close_dangling_turns().unwrap(), 1);
        assert_eq!(store.last_seq(&open).unwrap(), 4);
        let row = store.session(&open).unwrap().unwrap();
        assert_eq!(row.last_done, Some(DoneReason::Error));
        assert_eq!(store.last_seq(&ended).unwrap(), 3);
        assert_eq!(store.close_dangling_turns().unwrap(), 0);
    }

    #[test]
    fn events_before_starts_pages_at_a_turn_start() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "t").unwrap();
        for turn in 0..2 {
            store
                .append(&sid, &Event::RunningChanged { is_running: true })
                .unwrap();
            for i in 0..3 {
                store
                    .append(
                        &sid,
                        &Event::MessageDelta {
                            message_id: "m".into(),
                            content: format!("t{turn}c{i}"),
                            tokens_per_second: None,
                        },
                    )
                    .unwrap();
            }
            store
                .append(&sid, &Event::RunningChanged { is_running: false })
                .unwrap();
        }

        let page = store.events_before(&sid, u64::MAX, 2).unwrap();
        assert_eq!(
            page.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            vec![7, 8, 9, 10, 11]
        );

        let page = store.events_before(&sid, 7, 2).unwrap();
        assert_eq!(
            page.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            vec![2, 3, 4, 5, 6]
        );

        let page = store.events_before(&sid, 2, 10).unwrap();
        assert_eq!(page.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![1]);

        let page = store.events_before(&sid, u64::MAX, 100).unwrap();
        assert_eq!(page.len(), 11);
        assert_eq!(
            page.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            (1..=11).collect::<Vec<_>>()
        );

        assert!(store.events_before(&sid, 1, 10).unwrap().is_empty());
        assert!(store.events_before("missing", 10, 10).unwrap().is_empty());
    }

    #[test]
    fn workflow_run_before_returns_only_the_latest_run_before_seq() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "t").unwrap();

        fn workflow_started(run_id: &str) -> Event {
            let workflow: crate::domain::workflow::definition::Workflow =
                serde_json::from_value(serde_json::json!({
                    "id": "w",
                    "name": "w",
                    "nodes": [],
                    "edges": []
                }))
                .unwrap();
            Event::WorkflowStarted {
                run_id: run_id.into(),
                workflow,
                task: "task".into(),
                step_by_step: false,
                started_at: None,
            }
        }

        store.append(&sid, &workflow_started("r1")).unwrap();
        store
            .append(
                &sid,
                &Event::MessageDelta {
                    message_id: "m".into(),
                    content: "between".into(),
                    tokens_per_second: None,
                },
            )
            .unwrap();
        store.append(&sid, &workflow_started("r2")).unwrap();
        store
            .append(
                &sid,
                &Event::MessageDelta {
                    message_id: "m".into(),
                    content: "after".into(),
                    tokens_per_second: None,
                },
            )
            .unwrap();

        // seqs: 1 from create_session, 2 r1, 3 delta, 4 r2, 5 delta.
        let run = store.workflow_run_before(&sid, u64::MAX).unwrap();
        assert_eq!(run.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![4]);

        let run = store.workflow_run_before(&sid, 4).unwrap();
        assert_eq!(run.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![2]);

        assert!(store.workflow_run_before(&sid, 2).unwrap().is_empty());
    }

    #[test]
    fn child_sessions_are_hidden_from_lists_but_found_by_id() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let parent = store.create_session(&project, "parent").unwrap();
        let child = store.create_child_session(&parent, "child").unwrap();

        let listed = store.sessions(Some(&project.id)).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, parent);
        assert!(listed[0].parent_id.is_none());

        let found = store.session(&child).unwrap().unwrap();
        assert_eq!(found.parent_id.as_deref(), Some(parent.as_str()));
        assert_eq!(found.project_id, project.id);

        assert!(store.create_child_session("missing", "x").is_err());
    }

    #[test]
    fn deleting_a_session_tree_removes_its_children_and_events() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let parent = store.create_session(&project, "parent").unwrap();
        let child = store.create_child_session(&parent, "child").unwrap();
        let grandchild = store.create_child_session(&child, "grandchild").unwrap();
        let unrelated = store.create_session(&project, "unrelated").unwrap();

        let tree = store.session_tree(&parent).unwrap();
        assert_eq!(tree.len(), 3);
        assert!(tree.contains(&parent));
        assert!(tree.contains(&child));
        assert!(tree.contains(&grandchild));

        store.delete_sessions(&tree).unwrap();
        assert!(store.session(&parent).unwrap().is_none());
        assert!(store.session(&child).unwrap().is_none());
        assert!(store.session(&grandchild).unwrap().is_none());
        assert!(store.events_after(&child, 0).unwrap().is_empty());
        assert!(store.session(&unrelated).unwrap().is_some());
    }

    #[test]
    fn delete_project_removes_its_sessions_events_and_ideas() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/del").unwrap();
        let other = store.project_for("/other").unwrap();
        let sid = store.create_session(&project, "s").unwrap();
        store
            .append_idea_event(
                &project.id,
                &backlog::BacklogEvent::Added {
                    idea_id: "i".into(),
                    text: "t".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();
        let other_sid = store.create_session(&other, "o").unwrap();
        store
            .append_idea_event(
                &other.id,
                &backlog::BacklogEvent::Added {
                    idea_id: "i2".into(),
                    text: "t".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();

        store.delete_project(&project.id).unwrap();

        assert!(store.project(&project.id).unwrap().is_none());
        assert!(store.session(&sid).unwrap().is_none());
        assert!(store.events_after(&sid, 0).unwrap().is_empty());
        assert_eq!(
            store
                .db
                .query_row(
                    "SELECT COUNT(*) FROM project_events WHERE project_id = ?1",
                    [project.id.as_str()],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert!(store.project(&other.id).unwrap().is_some());
        assert!(store.session(&other_sid).unwrap().is_some());
        assert_eq!(
            store
                .db
                .query_row(
                    "SELECT COUNT(*) FROM project_events WHERE project_id = ?1",
                    [other.id.as_str()],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn opening_an_old_database_adds_the_parent_id_column() {
        let path = std::env::temp_dir().join(format!("fennec-store-{}", new_id()));
        {
            let db = rusqlite::Connection::open(&path).unwrap();
            db.execute_batch(
                "CREATE TABLE sessions (
                   id TEXT PRIMARY KEY,
                   project_id TEXT NOT NULL,
                   workdir TEXT NOT NULL,
                   title TEXT,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );",
            )
            .unwrap();
        }
        let store = SqliteStore::open(&path).unwrap();
        let project = store.project_for("/w").unwrap();
        let parent = store.create_session(&project, "p").unwrap();
        let child = store.create_child_session(&parent, "c").unwrap();
        assert!(store.session(&child).unwrap().unwrap().parent_id.is_some());
        let _ = std::fs::remove_file(&path);
    }
}
