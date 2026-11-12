//! Sessions: creation, listing, linking, and the session delete cascade.

use super::{Result, SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use crate::domain::event::{DoneReason, Event};
use crate::domain::id::new_id;
use crate::domain::project::{Project, SessionSummary};
use rusqlite::{OptionalExtension, params};

impl SqliteStore {
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

    /// Links a chat to a project, or unlinks it with `None`: a reference for grouping only.
    /// Returns whether a session matched.
    pub fn link_session(&self, id: &str, project_id: Option<&str>) -> Result<bool> {
        let rows = self.db.execute(
            "UPDATE sessions SET linked_project_id = ?1 WHERE id = ?2",
            params![project_id, id],
        )?;
        Ok(rows > 0)
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
}

const SESSION_SELECT: &str = "SELECT s.id, s.project_id, COALESCE(s.title, ''), CAST(s.created_at AS INTEGER),
    COALESCE((SELECT MAX(timestamp) FROM events e WHERE e.session_id = s.id), CAST(s.created_at AS INTEGER)),
    (SELECT json_extract(payload, '$.data.reason') FROM events e
       WHERE e.session_id = s.id AND e.event_type = 'chat.done' ORDER BY seq DESC LIMIT 1),
    s.parent_id,
    COALESCE((SELECT e.event_type = 'workflow.finished' AND json_extract(e.payload, '$.data.outcome') = 'done'
       FROM events e WHERE e.session_id = s.id
         AND e.event_type IN ('workflow.finished', 'integration.applied', 'worktree.discarded')
       ORDER BY seq DESC LIMIT 1), 0),
    s.linked_project_id
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
        linked_project_id: r.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::EventLog;
    use crate::domain::workflow::run::RunOutcome;

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
    fn link_session_links_and_unlinks_a_session() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/link").unwrap();
        let target = store.project_for("/link-target").unwrap();
        let session = store.create_session(&project, "t").unwrap();
        assert_eq!(
            store.session(&session).unwrap().unwrap().linked_project_id,
            None
        );
        assert!(store.link_session(&session, Some(&target.id)).unwrap());
        assert_eq!(
            store.session(&session).unwrap().unwrap().linked_project_id,
            Some(target.id.clone())
        );
        assert!(store.link_session(&session, None).unwrap());
        assert_eq!(
            store.session(&session).unwrap().unwrap().linked_project_id,
            None
        );
        assert!(!store.link_session("missing", Some(&target.id)).unwrap());
    }
}
