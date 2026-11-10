//! Storage adapter: the SQLite store, `projects`/`sessions`/`events`. The
//! schema is frozen: stored logs depend on it. Implements the agent's `EventLog`.

mod budgets;
pub mod calls;
mod events;
mod ideas;
mod mcp_credentials;
mod migrations;
mod plugins;
mod project_events;
mod projects;
mod sessions;
mod settings;

use rusqlite::Connection;
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

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS projects (
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
       linked_project_id TEXT,
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
     CREATE INDEX IF NOT EXISTS idx_events_session_type ON events(session_id, event_type, seq);
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
        // journal_size_limit: the WAL is truncated back to 64 MB after a checkpoint instead
        // of keeping its high-water size.
        db.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA journal_size_limit = 67108864;",
        )?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        SqliteStore::init(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<SqliteStore> {
        SqliteStore::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<SqliteStore> {
        db.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrations::migrate(&db)?;
        Ok(SqliteStore { db })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;

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
        assert_eq!(
            store.session(&child).unwrap().unwrap().linked_project_id,
            None
        );
        let _ = std::fs::remove_file(&path);
    }
}
