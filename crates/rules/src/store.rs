//! SQLite storage for rules.

use std::io;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

use crate::command::PatternError;
use crate::rule::{Access, Action, Origin, Rule, Rules, Scope, Target, TargetKind};

/// A rule store backed by one SQLite file.
pub struct RuleStore {
    db: Mutex<Connection>,
}

/// Errors from opening, querying, or interpreting the rule store.
#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(io::Error),
    Corrupt(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sqlite(error) => write!(f, "SQLite error: {error}"),
            StoreError::Io(error) => write!(f, "I/O error: {error}"),
            StoreError::Corrupt(message) => write!(f, "corrupt rule row: {message}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Sqlite(error) => Some(error),
            StoreError::Io(error) => Some(error),
            StoreError::Corrupt(_) => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A new rule before it is parsed and stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRule {
    pub scope: Scope,
    pub kind: TargetKind,
    pub pattern: String,
    pub access: Option<Access>,
    pub action: Action,
    pub note: Option<String>,
    pub origin: Origin,
}

impl RuleStore {
    /// Opens the store at `path`, creating parent directories when needed.
    pub fn open(path: &Path) -> Result<RuleStore, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(path)?;
        Self::prepare(db)
    }

    /// Opens an in-memory store, mainly for tests.
    pub fn open_in_memory() -> Result<RuleStore, StoreError> {
        let db = Connection::open_in_memory()?;
        Self::prepare(db)
    }

    /// Adds a rule.
    ///
    /// The target is parsed before writing so an invalid pattern never creates a row.
    pub fn add(&self, rule: NewRule) -> Result<Result<Rule, PatternError>, StoreError> {
        let target = match Target::parse(rule.kind, &rule.pattern, rule.access) {
            Ok(target) => target,
            Err(error) => return Ok(Err(error)),
        };

        let note = rule
            .note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .map(str::to_string);

        let conn = self.conn();
        let kind = target.kind();
        let pattern = target.pattern();
        let access = target.access().map(Access::as_str);
        let scope_label = rule.scope.label();
        let scope_id = match &rule.scope {
            Scope::Global => None,
            Scope::Project(id) => Some(id.as_str()),
            Scope::Session(id) => Some(id.as_str()),
        };
        let origin_session = match &rule.origin {
            // A kernel rule never reaches the store; the arm only keeps the match exhaustive.
            Origin::Human | Origin::Fennec => None,
            Origin::Answer { session_id } => Some(session_id.as_str()),
        };
        let created_at = now_ms();

        conn.execute(
            "INSERT INTO rules (
                scope,
                scope_id,
                target_kind,
                pattern,
                access,
                action,
                note,
                origin_session,
                created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                scope_label,
                scope_id,
                kind.as_str(),
                pattern,
                access,
                rule.action.as_str(),
                note.as_deref(),
                origin_session,
                created_at
            ],
        )?;

        let id = conn.last_insert_rowid();
        Ok(Ok(Rule {
            id,
            scope: rule.scope,
            target,
            action: rule.action,
            note,
            origin: rule.origin,
        }))
    }

    /// Deletes a rule by id.
    pub fn delete(&self, id: i64) -> Result<bool, StoreError> {
        let conn = self.conn();
        conn.execute("DELETE FROM rules WHERE id = ?1", params![id])?;
        Ok(conn.changes() > 0)
    }

    /// Moves a rule to a new scope.
    pub fn set_scope(&self, id: i64, scope: Scope) -> Result<bool, StoreError> {
        let conn = self.conn();
        let scope_label = scope.label();
        let scope_id = match &scope {
            Scope::Global => None,
            Scope::Project(id) => Some(id.as_str()),
            Scope::Session(id) => Some(id.as_str()),
        };
        conn.execute(
            "UPDATE rules SET scope = ?1, scope_id = ?2 WHERE id = ?3",
            params![scope_label, scope_id, id],
        )?;
        Ok(conn.changes() > 0)
    }

    /// Lists all rules, ordered by creation time then id.
    pub fn list(&self) -> Result<Vec<Rule>, StoreError> {
        let conn = self.conn();
        let rows = query_rows(
            &conn,
            "SELECT id, scope, scope_id, target_kind, pattern, access, action, note, \
             origin_session, created_at \
             FROM rules \
             ORDER BY created_at ASC, id ASC",
            params![],
        )?;
        rows_to_rules(rows)
    }

    /// Returns the rules visible for one turn: global, then project, then session.
    pub fn for_turn(
        &self,
        project_id: &str,
        session_id: Option<&str>,
    ) -> Result<Rules, StoreError> {
        let mut rules = self.query_scope("global", None)?;
        rules.extend(self.query_scope("project", Some(project_id))?);
        if let Some(session_id) = session_id {
            rules.extend(self.query_scope("session", Some(session_id))?);
        }
        Ok(Rules::new(rules))
    }

    /// Deletes all project rules for a project.
    pub fn delete_project(&self, project_id: &str) -> Result<usize, StoreError> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM rules WHERE scope = 'project' AND scope_id = ?1",
            params![project_id],
        )?;
        Ok(conn.changes() as usize)
    }

    /// Deletes all session grants for a session.
    pub fn delete_session(&self, session_id: &str) -> Result<usize, StoreError> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM rules WHERE scope = 'session' AND scope_id = ?1",
            params![session_id],
        )?;
        Ok(conn.changes() as usize)
    }

    fn prepare(db: Connection) -> Result<RuleStore, StoreError> {
        db.execute_batch("PRAGMA journal_mode=WAL;")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scope TEXT NOT NULL,
                scope_id TEXT,
                target_kind TEXT NOT NULL,
                pattern TEXT NOT NULL,
                access TEXT,
                action TEXT NOT NULL,
                note TEXT,
                origin_session TEXT,
                created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS rules_scope_scope_id
             ON rules (scope, scope_id);",
        )?;
        Ok(RuleStore { db: Mutex::new(db) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn query_scope(&self, scope: &str, scope_id: Option<&str>) -> Result<Vec<Rule>, StoreError> {
        let conn = self.conn();
        let rows = query_rows(
            &conn,
            "SELECT id, scope, scope_id, target_kind, pattern, access, action, note, \
             origin_session, created_at \
             FROM rules \
             WHERE scope = ?1 AND (?2 IS NULL OR scope_id = ?2) \
             ORDER BY created_at ASC, id ASC",
            params![scope, scope_id],
        )?;
        rows_to_rules(rows)
    }
}

struct Row {
    id: i64,
    scope: String,
    scope_id: Option<String>,
    kind: String,
    pattern: String,
    access: Option<String>,
    action: String,
    note: Option<String>,
    origin_session: Option<String>,
}

fn query_rows(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<Row>, StoreError> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, |row| {
        Ok(Row {
            id: row.get(0)?,
            scope: row.get(1)?,
            scope_id: row.get(2)?,
            kind: row.get(3)?,
            pattern: row.get(4)?,
            access: row.get(5)?,
            action: row.get(6)?,
            note: row.get(7)?,
            origin_session: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn rows_to_rules(rows: Vec<Row>) -> Result<Vec<Rule>, StoreError> {
    rows.into_iter()
        .map(row_to_rule)
        .collect::<Result<Vec<Rule>, StoreError>>()
}

fn row_to_rule(row: Row) -> Result<Rule, StoreError> {
    let scope = match row.scope.as_str() {
        "global" => Scope::Global,
        "project" => Scope::Project(row.scope_id.clone().unwrap_or_default()),
        "session" => Scope::Session(row.scope_id.clone().unwrap_or_default()),
        other => {
            return Err(StoreError::Corrupt(format!(
                "rule {} has unknown scope {other}",
                row.id
            )));
        }
    };

    let kind = TargetKind::parse(&row.kind).ok_or_else(|| {
        StoreError::Corrupt(format!(
            "rule {} has unknown target kind {}",
            row.id, row.kind
        ))
    })?;

    let action = Action::parse(&row.action).ok_or_else(|| {
        StoreError::Corrupt(format!("rule {} has unknown action {}", row.id, row.action))
    })?;

    let access = match &row.access {
        Some(access) => Some(Access::parse(access).ok_or_else(|| {
            StoreError::Corrupt(format!("rule {} has unknown access {access}", row.id))
        })?),
        None => None,
    };

    let target = Target::parse(kind, &row.pattern, access).map_err(|error| {
        StoreError::Corrupt(format!("rule {} has bad pattern: {error}", row.id))
    })?;

    let origin = match row.origin_session {
        Some(session_id) => Origin::Answer { session_id },
        None => Origin::Human,
    };

    Ok(Rule {
        id: row.id,
        scope,
        target,
        action,
        note: row.note,
        origin,
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(store: &RuleStore, rule: NewRule) -> Rule {
        store.add(rule).unwrap().unwrap()
    }

    #[test]
    fn add_then_list_round_trips_fields() {
        let store = RuleStore::open_in_memory().unwrap();
        let rule = add(
            &store,
            NewRule {
                scope: Scope::Session("s1".into()),
                kind: TargetKind::Path,
                pattern: "secrets/".into(),
                access: Some(Access::Read),
                action: Action::Block,
                note: Some("  keep  ".into()),
                origin: Origin::Answer {
                    session_id: "s1".into(),
                },
            },
        );

        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0], rule);
        assert_eq!(listed[0].note.as_deref(), Some("keep"));
        assert_eq!(listed[0].target.access(), Some(Access::Read));
    }

    #[test]
    fn invalid_pattern_writes_no_row() {
        let store = RuleStore::open_in_memory().unwrap();
        let result = store
            .add(NewRule {
                scope: Scope::Global,
                kind: TargetKind::Path,
                pattern: "..".into(),
                access: None,
                action: Action::Block,
                note: None,
                origin: Origin::Human,
            })
            .unwrap();
        assert!(result.is_err());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn delete_returns_true_then_false() {
        let store = RuleStore::open_in_memory().unwrap();
        let rule = add(
            &store,
            NewRule {
                scope: Scope::Global,
                kind: TargetKind::Tool,
                pattern: "web_search".into(),
                access: None,
                action: Action::Ask,
                note: None,
                origin: Origin::Human,
            },
        );
        assert!(store.delete(rule.id).unwrap());
        assert!(!store.delete(rule.id).unwrap());
    }

    #[test]
    fn for_turn_keeps_other_projects_and_sessions_out() {
        let store = RuleStore::open_in_memory().unwrap();
        add(
            &store,
            NewRule {
                scope: Scope::Global,
                kind: TargetKind::Tool,
                pattern: "global".into(),
                access: None,
                action: Action::Ask,
                note: None,
                origin: Origin::Human,
            },
        );
        add(
            &store,
            NewRule {
                scope: Scope::Project("a".into()),
                kind: TargetKind::Tool,
                pattern: "project_a".into(),
                access: None,
                action: Action::Ask,
                note: None,
                origin: Origin::Human,
            },
        );
        add(
            &store,
            NewRule {
                scope: Scope::Project("b".into()),
                kind: TargetKind::Tool,
                pattern: "project_b".into(),
                access: None,
                action: Action::Ask,
                note: None,
                origin: Origin::Human,
            },
        );
        add(
            &store,
            NewRule {
                scope: Scope::Session("s1".into()),
                kind: TargetKind::Tool,
                pattern: "session_s1".into(),
                access: None,
                action: Action::Ask,
                note: None,
                origin: Origin::Human,
            },
        );
        add(
            &store,
            NewRule {
                scope: Scope::Session("s2".into()),
                kind: TargetKind::Tool,
                pattern: "session_s2".into(),
                access: None,
                action: Action::Ask,
                note: None,
                origin: Origin::Human,
            },
        );

        let rules = store.for_turn("a", Some("s1")).unwrap();
        let patterns: Vec<String> = rules.all().iter().map(|r| r.target.pattern()).collect();
        assert_eq!(
            patterns,
            vec![
                "global".to_string(),
                "project_a".to_string(),
                "session_s1".to_string()
            ]
        );
    }

    #[test]
    fn delete_session_removes_only_that_session() {
        let store = RuleStore::open_in_memory().unwrap();
        add(
            &store,
            NewRule {
                scope: Scope::Session("s1".into()),
                kind: TargetKind::Tool,
                pattern: "s1".into(),
                access: None,
                action: Action::Allow,
                note: None,
                origin: Origin::Human,
            },
        );
        add(
            &store,
            NewRule {
                scope: Scope::Session("s2".into()),
                kind: TargetKind::Tool,
                pattern: "s2".into(),
                access: None,
                action: Action::Allow,
                note: None,
                origin: Origin::Human,
            },
        );

        assert_eq!(store.delete_session("s1").unwrap(), 1);
        let rules = store.list().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].target.pattern(), "s2");
    }

    #[test]
    fn set_scope_moves_rule_to_global() {
        let store = RuleStore::open_in_memory().unwrap();
        let rule = add(
            &store,
            NewRule {
                scope: Scope::Project("a".into()),
                kind: TargetKind::Tool,
                pattern: "web_search".into(),
                access: None,
                action: Action::Ask,
                note: None,
                origin: Origin::Human,
            },
        );

        assert!(store.set_scope(rule.id, Scope::Global).unwrap());
        let listed = store.list().unwrap();
        assert_eq!(listed[0].scope, Scope::Global);
    }
}
