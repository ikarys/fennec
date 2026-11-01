//! The rules the human stores, global (no project) or per project. Config rows, not events: what a rule did is logged in the session (`rule.blocked`).

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use crate::domain::id::new_id;
use rules::{Action, PatternError, Rule, Rules, Scope, Target, TargetKind};
use rusqlite::params;

pub(super) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL,
    pattern TEXT NOT NULL,
    action TEXT NOT NULL,
    note TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS rules_project_id ON rules(project_id);
"#;

pub struct NewRule {
    pub scope: Scope,
    pub kind: TargetKind,
    pub pattern: String,
    pub action: Action,
    pub note: Option<String>,
}

impl SqliteStore {
    pub fn add_rule(&self, rule: NewRule) -> Result<Result<Rule, PatternError>, StoreError> {
        // Validate before writing so an invalid pattern never leaves a row behind.
        let kind = rule.kind;
        let kind_text = kind_str(&kind);
        let target = match Target::parse(kind, &rule.pattern) {
            Ok(target) => target,
            Err(err) => return Ok(Err(err)),
        };

        // Trim notes so blank annotations are stored as absent, not empty strings.
        let note = rule
            .note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .map(str::to_owned);

        let id = new_id();
        let created_at = now_ms() as i64;
        let project_id = match &rule.scope {
            Scope::Global => None,
            Scope::Project(project_id) => Some(project_id.clone()),
        };
        let action_text = action_str(&rule.action);

        self.db.execute(
            "INSERT INTO rules
             (id, project_id, target_kind, pattern, action, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                project_id,
                kind_text,
                target.pattern(),
                action_text,
                note,
                created_at
            ],
        )?;

        Ok(Ok(Rule {
            id,
            scope: rule.scope,
            target,
            action: rule.action,
            note,
        }))
    }

    pub fn delete_rule(&self, id: &str) -> Result<bool, StoreError> {
        let deleted = self
            .db
            .execute("DELETE FROM rules WHERE id = ?1", params![id])?;
        Ok(deleted > 0)
    }

    pub fn global_rules(&self) -> Result<Vec<Rule>, StoreError> {
        self.query_rules("WHERE project_id IS NULL", params![])
    }

    pub fn project_rules(&self, project_id: &str) -> Result<Vec<Rule>, StoreError> {
        self.query_rules("WHERE project_id = ?1", params![project_id])
    }

    fn query_rules(
        &self,
        filter: &str,
        args: &[&dyn rusqlite::ToSql],
    ) -> Result<Vec<Rule>, StoreError> {
        let mut stmt = self.db.prepare(&format!(
            "SELECT id, project_id, target_kind, pattern, action, note FROM rules {filter} ORDER BY created_at, rowid"
        ))?;
        let rows = stmt
            .query_map(args, |row| {
                Ok(StoredRule {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    kind: row.get(2)?,
                    pattern: row.get(3)?,
                    action: row.get(4)?,
                    note: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(StoredRule::into_rule).collect()
    }

    pub fn rules_for(&self, project_id: &str) -> Result<Rules, StoreError> {
        // Global rules apply everywhere, so they must come before project rules.
        let mut rules = self.global_rules()?;
        rules.extend(self.project_rules(project_id)?);
        Ok(Rules::new(rules))
    }
}

fn kind_str(kind: &TargetKind) -> &'static str {
    match kind {
        TargetKind::Command => "command",
        TargetKind::Tool => "tool",
        TargetKind::Path => "path",
    }
}

fn action_str(action: &Action) -> &'static str {
    match action {
        Action::Block => "block",
        Action::Ask => "ask",
    }
}

fn parse_kind(kind: &str) -> Result<TargetKind, StoreError> {
    match kind {
        "command" => Ok(TargetKind::Command),
        "tool" => Ok(TargetKind::Tool),
        "path" => Ok(TargetKind::Path),
        other => Err(StoreError::Json(format!("unknown target kind: {other}"))),
    }
}

fn parse_action(action: &str) -> Result<Action, StoreError> {
    match action {
        "block" => Ok(Action::Block),
        "ask" => Ok(Action::Ask),
        other => Err(StoreError::Json(format!("unknown action: {other}"))),
    }
}

/// A row as stored; turned into a `Rule` outside the row closure so a bad row is a `StoreError`.
struct StoredRule {
    id: String,
    project_id: Option<String>,
    kind: String,
    pattern: String,
    action: String,
    note: Option<String>,
}

impl StoredRule {
    fn into_rule(self) -> Result<Rule, StoreError> {
        let scope = match self.project_id {
            None => Scope::Global,
            Some(project_id) => Scope::Project(project_id),
        };
        // Re-parsed: a stored pattern that no longer parses fails as data, not as a panic.
        let target = Target::parse(parse_kind(&self.kind)?, &self.pattern)
            .map_err(|e| StoreError::Json(format!("stored rule {}: {e}", self.id)))?;
        Ok(Rule {
            id: self.id,
            scope,
            target,
            action: parse_action(&self.action)?,
            note: self.note,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_for_returns_global_then_project() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/tmp/p").unwrap();
        let other = store.project_for("/tmp/other").unwrap();

        let _ = store
            .add_rule(NewRule {
                scope: Scope::Global,
                kind: TargetKind::Tool,
                pattern: "write".into(),
                action: Action::Block,
                note: None,
            })
            .unwrap()
            .unwrap();
        let _ = store
            .add_rule(NewRule {
                scope: Scope::Project(project.id.clone()),
                kind: TargetKind::Path,
                pattern: "src".into(),
                action: Action::Ask,
                note: Some("  careful  ".into()),
            })
            .unwrap()
            .unwrap();
        let _ = store
            .add_rule(NewRule {
                scope: Scope::Project(other.id.clone()),
                kind: TargetKind::Command,
                pattern: "rm -rf".into(),
                action: Action::Block,
                note: None,
            })
            .unwrap()
            .unwrap();

        let rules = store.rules_for(&project.id).unwrap();
        let all = rules.all();

        assert_eq!(all.len(), 2);
        assert!(matches!(all[0].scope, Scope::Global));
        assert!(matches!(all[1].scope, Scope::Project(_)));
        assert_eq!(all[1].note.as_deref(), Some("careful"));
    }

    #[test]
    fn invalid_command_pattern_is_not_stored() {
        let store = SqliteStore::in_memory().unwrap();

        let result = store
            .add_rule(NewRule {
                scope: Scope::Global,
                kind: TargetKind::Command,
                pattern: "-rf".into(),
                action: Action::Block,
                note: None,
            })
            .unwrap();

        assert!(result.is_err());
        assert!(store.global_rules().unwrap().is_empty());
    }

    #[test]
    fn delete_rule_reports_deleted_rows() {
        let store = SqliteStore::in_memory().unwrap();
        let rule = store
            .add_rule(NewRule {
                scope: Scope::Global,
                kind: TargetKind::Tool,
                pattern: "write".into(),
                action: Action::Block,
                note: None,
            })
            .unwrap()
            .unwrap();

        assert!(store.delete_rule(&rule.id).unwrap());
        assert!(!store.delete_rule(&rule.id).unwrap());
    }
}
