//! Projects: lookup and creation, favourites, recency, and the project delete cascade.

use super::{Result, SqliteStore};
use crate::chat;
use crate::domain::clock::now_ms;
use crate::domain::id::new_id;
use crate::domain::project::Project;
use rusqlite::{OptionalExtension, params};
use std::path::Path;

impl SqliteStore {
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

    /// A project's last activity: the newest event of its top-level sessions, else their creation.
    const RECENT_ACTIVITY: &str = "(SELECT MAX(COALESCE(
        (SELECT e.timestamp FROM events e WHERE e.session_id = s.id
            ORDER BY e.seq DESC LIMIT 1),
        CAST(s.created_at AS INTEGER)))
     FROM sessions s WHERE s.project_id = p.id AND s.parent_id IS NULL)";

    /// Favourites first, then most recently active (the newest event of its
    /// top-level sessions, else their creation), then by name.
    pub fn projects(&self) -> Result<Vec<Project>> {
        let recent_activity = Self::RECENT_ACTIVITY;
        let mut stmt = self.db.prepare(&format!(
            "SELECT p.id, p.name, p.workdir FROM projects p
             ORDER BY p.favorite DESC,
                      {recent_activity} DESC,
                      p.name"
        ))?;
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

    /// Project ids by recent activity alone, favourites ignored: the "+" menu's order.
    pub fn recent_project_ids(&self) -> Result<Vec<String>> {
        let recent_activity = Self::RECENT_ACTIVITY;
        let mut stmt = self.db.prepare(&format!(
            "SELECT p.id FROM projects p
             ORDER BY {recent_activity} DESC, p.name"
        ))?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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
        // A chat linked to the project falls back to unlinked; the chat stays.
        tx.execute(
            "UPDATE sessions SET linked_project_id = NULL WHERE linked_project_id = ?1",
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
    fn recent_project_ids_ignore_favourites() {
        let store = SqliteStore::in_memory().unwrap();
        let idle = store.project_for("/a").unwrap();
        let busy = store.project_for("/b").unwrap();
        let none = store.project_for("/c").unwrap();
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
        assert_eq!(
            store.recent_project_ids().unwrap(),
            [busy.id, idle.id, none.id]
        );
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
    fn deleting_a_project_unlinks_a_linked_chat_and_keeps_the_chat() {
        let store = SqliteStore::in_memory().unwrap();
        let chat = store.chat_project("/tmp/scratch-link").unwrap();
        let target = store.project_for("/tmp/link-target-del").unwrap();
        let session = store.create_session(&chat, "chat").unwrap();
        assert!(store.link_session(&session, Some(&target.id)).unwrap());
        store.delete_project(&target.id).unwrap();
        let row = store.session(&session).unwrap().unwrap();
        assert_eq!(row.linked_project_id, None);
        assert!(store.project(&chat.id).unwrap().is_some());
    }
}
