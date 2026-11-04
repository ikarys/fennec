//! SQLite storage for artifacts, versions, and annotations.

use std::fmt;
use std::io;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

use crate::annotation::{Anchor, Annotation, AnnotationError, validate};
use crate::artifact::{Artifact, NewArtifact, Source, Version, title_for};
use crate::format::{Format, MAX_CONTENT_BYTES};

/// In-process SQLite store for canvas data.
#[derive(Debug)]
pub struct CanvasStore {
    db: Mutex<Connection>,
}

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(io::Error),
    Corrupt(String),
    TooLarge,
    Empty,
    NotFound,
    Annotation(AnnotationError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Corrupt(msg) => write!(f, "corrupt: {msg}"),
            Self::TooLarge => write!(f, "content too large"),
            Self::Empty => write!(f, "content empty"),
            Self::NotFound => write!(f, "not found"),
            Self::Annotation(e) => write!(f, "annotation: {e}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Annotation(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<AnnotationError> for StoreError {
    fn from(e: AnnotationError) -> Self {
        Self::Annotation(e)
    }
}

impl CanvasStore {
    /// Open a store at a file path, creating parent directories.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::prepare(conn)
    }

    /// Open an in-memory store, mainly for tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        Self::prepare(conn)
    }

    fn prepare(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS artifacts(
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id TEXT NOT NULL,
                 session_id TEXT NOT NULL,
                 title TEXT NOT NULL,
                 format TEXT NOT NULL,
                 source_path TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS artifacts_session_idx ON artifacts(session_id);
             CREATE INDEX IF NOT EXISTS artifacts_project_idx ON artifacts(project_id);
             CREATE TABLE IF NOT EXISTS versions(
                 artifact_id INTEGER NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
                 number INTEGER NOT NULL,
                 content BLOB NOT NULL,
                 created_at INTEGER NOT NULL,
                 PRIMARY KEY(artifact_id, number)
             );
             CREATE TABLE IF NOT EXISTS annotations(
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 artifact_id INTEGER NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
                 version INTEGER NOT NULL,
                 kind TEXT NOT NULL,
                 quote TEXT,
                 x REAL,
                 y REAL,
                 width REAL,
                 height REAL,
                 note TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Unix milliseconds, as SQLite stores integers.
    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn check_content(content: &[u8]) -> Result<(), StoreError> {
        if content.is_empty() {
            Err(StoreError::Empty)
        } else if content.len() > MAX_CONTENT_BYTES {
            Err(StoreError::TooLarge)
        } else {
            Ok(())
        }
    }

    fn artifact_from_row(row: &rusqlite::Row) -> rusqlite::Result<Artifact> {
        let format = Format::parse(&row.get::<_, String>("format")?)
            .ok_or_else(|| corrupt("unknown format".to_string()))?;
        let source_path: Option<String> = row.get("source_path")?;
        let source = match source_path {
            Some(path) => Source::File { path },
            None => Source::Message,
        };
        Ok(Artifact {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            session_id: row.get("session_id")?,
            title: row.get("title")?,
            format,
            source,
            created_at: row.get::<_, i64>("created_at")? as u64,
            updated_at: row.get::<_, i64>("updated_at")? as u64,
            latest_version: row.get("latest_version")?,
        })
    }

    fn annotation_from_row(row: &rusqlite::Row) -> rusqlite::Result<Annotation> {
        let kind: String = row.get("kind")?;
        let quote: Option<String> = row.get("quote")?;
        let x: Option<f64> = row.get("x")?;
        let y: Option<f64> = row.get("y")?;
        let width: Option<f64> = row.get("width")?;
        let height: Option<f64> = row.get("height")?;
        let anchor = match kind.as_str() {
            "whole" => Anchor::Whole,
            "quote" => Anchor::Quote {
                text: quote.unwrap_or_default(),
            },
            "region" => Anchor::Region {
                x: x.unwrap_or(0.0),
                y: y.unwrap_or(0.0),
                width: width.unwrap_or(0.0),
                height: height.unwrap_or(0.0),
            },
            other => return Err(corrupt(format!("unknown anchor kind {other}"))),
        };
        Ok(Annotation {
            id: row.get("id")?,
            artifact_id: row.get("artifact_id")?,
            version: row.get("version")?,
            anchor,
            note: row.get("note")?,
            created_at: row.get::<_, i64>("created_at")? as u64,
        })
    }

    fn list<P: rusqlite::Params>(&self, sql: &str, params: P) -> Result<Vec<Artifact>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params, Self::artifact_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Create an artifact with its first version.
    pub fn create(&self, new: NewArtifact, content: &[u8]) -> Result<Artifact, StoreError> {
        Self::check_content(content)?;
        let title = title_for(&new.source, new.format, content);
        let now = Self::now_ms();
        let source_path = match &new.source {
            Source::File { path } => Some(path.clone()),
            Source::Message => None,
        };

        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO artifacts(project_id, session_id, title, format, source_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new.project_id,
                new.session_id,
                title,
                new.format.as_str(),
                source_path,
                now,
                now
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO versions(artifact_id, number, content, created_at) VALUES (?1, 1, ?2, ?3)",
            params![id, content, now],
        )?;
        tx.commit()?;

        Ok(Artifact {
            id,
            project_id: new.project_id,
            session_id: new.session_id,
            title,
            format: new.format,
            source: new.source,
            created_at: now as u64,
            updated_at: now as u64,
            latest_version: 1,
        })
    }

    /// Find an artifact by session and source file path.
    pub fn find_file(&self, session_id: &str, path: &str) -> Result<Option<Artifact>, StoreError> {
        let conn = self.lock();
        let sql = "SELECT a.*,
                 (SELECT MAX(v.number) FROM versions v WHERE v.artifact_id = a.id) AS latest_version
                 FROM artifacts a
                 WHERE a.session_id = ?1 AND a.source_path = ?2
                 LIMIT 1";
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query_map(params![session_id, path], |row| {
            Self::artifact_from_row(row)
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Store a new version if the content differs from the latest version.
    pub fn record_version(&self, id: i64, content: &[u8]) -> Result<Option<u32>, StoreError> {
        Self::check_content(content)?;
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let latest: Option<u32> = tx.query_row(
            "SELECT MAX(number) FROM versions WHERE artifact_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        let latest = match latest {
            Some(latest) => latest,
            None => {
                tx.rollback()?;
                return Err(StoreError::NotFound);
            }
        };

        if latest > 0 {
            let same: bool = tx.query_row(
                "SELECT content = ?2 FROM versions WHERE artifact_id = ?1 AND number = ?3",
                params![id, content, latest],
                |row| row.get(0),
            )?;
            if same {
                tx.rollback()?;
                return Ok(None);
            }
        }

        let number = latest + 1;
        let now = Self::now_ms();
        tx.execute(
            "INSERT INTO versions(artifact_id, number, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, number, content, now],
        )?;
        tx.execute(
            "UPDATE artifacts SET updated_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        tx.commit()?;
        Ok(Some(number))
    }

    /// Get one artifact by id.
    pub fn artifact(&self, id: i64) -> Result<Option<Artifact>, StoreError> {
        let conn = self.lock();
        let sql = "SELECT a.*,
                 (SELECT MAX(v.number) FROM versions v WHERE v.artifact_id = a.id) AS latest_version
                 FROM artifacts a
                 WHERE a.id = ?1";
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query_map(params![id], Self::artifact_from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// List artifacts in a session, most recently updated first.
    pub fn list_session(&self, session_id: &str) -> Result<Vec<Artifact>, StoreError> {
        self.list(
            "SELECT a.*,
             (SELECT MAX(v.number) FROM versions v WHERE v.artifact_id = a.id) AS latest_version
             FROM artifacts a
             WHERE a.session_id = ?1
             ORDER BY a.updated_at DESC, a.id DESC",
            params![session_id],
        )
    }

    /// List artifacts in a project, most recently updated first.
    pub fn list_project(&self, project_id: &str) -> Result<Vec<Artifact>, StoreError> {
        self.list(
            "SELECT a.*,
             (SELECT MAX(v.number) FROM versions v WHERE v.artifact_id = a.id) AS latest_version
             FROM artifacts a
             WHERE a.project_id = ?1
             ORDER BY a.updated_at DESC, a.id DESC",
            params![project_id],
        )
    }

    /// List version metadata for an artifact.
    pub fn versions(&self, id: i64) -> Result<Vec<Version>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT number, length(content), created_at
             FROM versions
             WHERE artifact_id = ?1
             ORDER BY number ASC",
        )?;
        let rows = stmt.query_map(params![id], |row| {
            Ok(Version {
                number: row.get(0)?,
                size: row.get::<_, i64>(1)? as u64,
                created_at: row.get::<_, i64>(2)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Get version content.
    pub fn content(&self, id: i64, number: u32) -> Result<Option<Vec<u8>>, StoreError> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT content FROM versions WHERE artifact_id = ?1 AND number = ?2")?;
        let mut rows = stmt.query_map(params![id, number], |row| row.get::<_, Vec<u8>>(0))?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Delete one artifact; versions and annotations cascade.
    pub fn delete(&self, id: i64) -> Result<bool, StoreError> {
        let conn = self.lock();
        let changed = conn.execute("DELETE FROM artifacts WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    /// Delete all artifacts in a session.
    pub fn delete_session(&self, session_id: &str) -> Result<usize, StoreError> {
        let conn = self.lock();
        Ok(conn.execute(
            "DELETE FROM artifacts WHERE session_id = ?1",
            params![session_id],
        )?)
    }

    /// Delete all artifacts in a project.
    pub fn delete_project(&self, project_id: &str) -> Result<usize, StoreError> {
        let conn = self.lock();
        Ok(conn.execute(
            "DELETE FROM artifacts WHERE project_id = ?1",
            params![project_id],
        )?)
    }

    /// Add a validated annotation to an existing version.
    pub fn annotate(
        &self,
        id: i64,
        version: u32,
        anchor: Anchor,
        note: &str,
    ) -> Result<Annotation, StoreError> {
        validate(&anchor, note)?;
        let conn = self.lock();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM versions WHERE artifact_id = ?1 AND number = ?2)",
            params![id, version],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StoreError::NotFound);
        }

        let now = Self::now_ms();
        let note = note.trim();
        let (kind, quote, x, y, width, height) = match &anchor {
            Anchor::Whole => ("whole".to_string(), None, None, None, None, None),
            Anchor::Quote { text } => (
                "quote".to_string(),
                Some(text.trim().to_string()),
                None,
                None,
                None,
                None,
            ),
            Anchor::Region {
                x,
                y,
                width,
                height,
            } => (
                "region".to_string(),
                None,
                Some(*x),
                Some(*y),
                Some(*width),
                Some(*height),
            ),
        };

        conn.execute(
            "INSERT INTO annotations(artifact_id, version, kind, quote, x, y, width, height, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![id, version, kind, quote, x, y, width, height, note, now],
        )?;
        let annotation_id = conn.last_insert_rowid();
        Ok(Annotation {
            id: annotation_id,
            artifact_id: id,
            version,
            anchor,
            note: note.to_string(),
            created_at: now as u64,
        })
    }

    /// List annotations for an artifact.
    pub fn annotations(&self, id: i64) -> Result<Vec<Annotation>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, artifact_id, version, kind, quote, x, y, width, height, note, created_at
             FROM annotations
             WHERE artifact_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![id], Self::annotation_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// A row fennec cannot read back, as rusqlite's error so the row mappers can return it.
fn corrupt(message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(StoreError::Corrupt(message)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Source;
    use crate::format::Format;

    fn store() -> CanvasStore {
        CanvasStore::open_in_memory().unwrap()
    }

    fn new_artifact(session: &str, source: Source) -> NewArtifact {
        NewArtifact {
            project_id: "p".into(),
            session_id: session.into(),
            format: Format::Html,
            source,
        }
    }

    #[test]
    fn create_and_artifact_round_trip() {
        let s = store();
        let created = s
            .create(
                new_artifact("s1", Source::Message),
                b"<html><title>Hello</title></html>",
            )
            .unwrap();
        assert_eq!(created.title, "Hello");
        assert_eq!(created.latest_version, 1);

        let loaded = s.artifact(created.id).unwrap().unwrap();
        assert_eq!(loaded, created);
    }

    #[test]
    fn record_version_identical_and_different() {
        let s = store();
        let a = s
            .create(new_artifact("s1", Source::Message), b"one")
            .unwrap();

        assert_eq!(s.record_version(a.id, b"one").unwrap(), None);
        assert_eq!(s.record_version(a.id, b"two").unwrap(), Some(2));

        let loaded = s.artifact(a.id).unwrap().unwrap();
        assert_eq!(loaded.latest_version, 2);
    }

    #[test]
    fn versions_sizes_and_old_content() {
        let s = store();
        let a = s
            .create(new_artifact("s1", Source::Message), b"one")
            .unwrap();
        s.record_version(a.id, b"two two").unwrap();

        let versions = s.versions(a.id).unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].number, 1);
        assert_eq!(versions[0].size, 3);
        assert_eq!(versions[1].number, 2);
        assert_eq!(versions[1].size, 7);

        assert_eq!(s.content(a.id, 1).unwrap(), Some(b"one".to_vec()));
        assert_eq!(s.content(a.id, 2).unwrap(), Some(b"two two".to_vec()));
        assert_eq!(s.content(a.id, 3).unwrap(), None);
    }

    #[test]
    fn find_file_returns_same_artifact() {
        let s = store();
        let a = s
            .create(
                new_artifact(
                    "s1",
                    Source::File {
                        path: "a/b.html".into(),
                    },
                ),
                b"<html></html>",
            )
            .unwrap();

        let found = s.find_file("s1", "a/b.html").unwrap().unwrap();
        assert_eq!(found.id, a.id);
        assert_eq!(s.find_file("s2", "a/b.html").unwrap(), None);
    }

    #[test]
    fn list_session_orders_by_updated_desc() {
        let s = store();
        let a = s.create(new_artifact("s1", Source::Message), b"a").unwrap();
        let b = s.create(new_artifact("s1", Source::Message), b"b").unwrap();
        // Timestamps are milliseconds: the new version must land in a later one.
        std::thread::sleep(std::time::Duration::from_millis(2));
        s.record_version(a.id, b"a2").unwrap();

        let list = s.list_session("s1").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, a.id);
        assert_eq!(list[1].id, b.id);
    }

    #[test]
    fn delete_cascades() {
        let s = store();
        let a = s
            .create(new_artifact("s1", Source::Message), b"one")
            .unwrap();
        s.annotate(a.id, 1, Anchor::Whole, "note").unwrap();

        assert!(s.delete(a.id).unwrap());
        assert_eq!(s.artifact(a.id).unwrap(), None);
        assert_eq!(s.content(a.id, 1).unwrap(), None);
        assert_eq!(s.annotations(a.id).unwrap(), Vec::new());
    }

    #[test]
    fn delete_session_and_project_only_touch_their_rows() {
        let s = store();
        let a = s.create(new_artifact("s1", Source::Message), b"a").unwrap();
        let b = s.create(new_artifact("s2", Source::Message), b"b").unwrap();

        assert_eq!(s.delete_session("s1").unwrap(), 1);
        assert_eq!(s.artifact(a.id).unwrap(), None);
        assert_eq!(s.artifact(b.id).unwrap(), Some(b.clone()));

        assert_eq!(s.delete_project("p").unwrap(), 1);
        assert_eq!(s.artifact(b.id).unwrap(), None);
    }

    #[test]
    fn annotate_validates_and_requires_version() {
        let s = store();
        let a = s
            .create(new_artifact("s1", Source::Message), b"one")
            .unwrap();

        assert!(matches!(
            s.annotate(a.id, 1, Anchor::Whole, "   ").unwrap_err(),
            StoreError::Annotation(AnnotationError::EmptyNote)
        ));
        assert!(matches!(
            s.annotate(a.id, 99, Anchor::Whole, "note").unwrap_err(),
            StoreError::NotFound
        ));

        let annotation = s
            .annotate(
                a.id,
                1,
                Anchor::Quote {
                    text: " one ".into(),
                },
                "  note  ",
            )
            .unwrap();
        assert_eq!(annotation.note, "note");
        assert_eq!(
            s.annotations(a.id).unwrap()[0].anchor,
            Anchor::Quote { text: "one".into() }
        );
    }

    #[test]
    fn empty_and_too_large_refused() {
        let s = store();
        let new = new_artifact("s1", Source::Message);

        assert!(matches!(
            s.create(new.clone(), b"").unwrap_err(),
            StoreError::Empty
        ));
        assert!(matches!(
            s.create(new.clone(), &vec![0u8; MAX_CONTENT_BYTES + 1])
                .unwrap_err(),
            StoreError::TooLarge
        ));

        let a = s.create(new, b"ok").unwrap();
        assert!(matches!(
            s.record_version(a.id, b"").unwrap_err(),
            StoreError::Empty
        ));
        assert!(matches!(
            s.record_version(a.id, &vec![0u8; MAX_CONTENT_BYTES + 1])
                .unwrap_err(),
            StoreError::TooLarge
        ));
    }
}
