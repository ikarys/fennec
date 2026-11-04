//! The human's plugin activations, one row per (project, plugin): the
//! definition's path, a hash of its code, and the network access and enabled
//! flag the human chose on the Settings page. A clone of a repository has the
//! definitions but no rows here, so nothing runs until the human activates it.

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use crate::domain::plugin::PluginActivation;
use rusqlite::params;

pub(super) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS plugin_activations (
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    network INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, name),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
"#;

impl SqliteStore {
    /// Upserts one plugin's activation.
    pub fn save_plugin_activation(
        &self,
        project_id: &str,
        activation: &PluginActivation,
    ) -> Result<(), StoreError> {
        self.db.execute(
            "INSERT INTO plugin_activations
                 (project_id, name, path, code_hash, network, enabled, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(project_id, name) DO UPDATE SET
                 path = excluded.path,
                 code_hash = excluded.code_hash,
                 network = excluded.network,
                 enabled = excluded.enabled,
                 updated_at = excluded.updated_at",
            params![
                project_id,
                activation.name,
                activation.path,
                activation.code_hash,
                activation.network as i64,
                activation.enabled as i64,
                now_ms() as i64,
            ],
        )?;
        Ok(())
    }

    /// Every activation of a project, in name order.
    pub fn plugin_activations(
        &self,
        project_id: &str,
    ) -> Result<Vec<PluginActivation>, StoreError> {
        let mut stmt = self.db.prepare(
            "SELECT name, path, code_hash, network, enabled
             FROM plugin_activations WHERE project_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map(params![project_id], activation_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::Sqlite)
    }
}

fn activation_row(row: &rusqlite::Row) -> rusqlite::Result<PluginActivation> {
    Ok(PluginActivation {
        name: row.get(0)?,
        path: row.get(1)?,
        code_hash: row.get(2)?,
        network: row.get::<_, i64>(3)? != 0,
        enabled: row.get::<_, i64>(4)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::super::SqliteStore;
    use crate::domain::plugin::PluginActivation;

    fn activation(name: &str) -> PluginActivation {
        PluginActivation {
            name: name.into(),
            path: format!(".agents/plugins/{name}"),
            code_hash: "hash".into(),
            network: false,
            enabled: true,
        }
    }

    #[test]
    fn activations_round_trip_and_default_to_empty() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();

        assert!(store.plugin_activations(&project.id).unwrap().is_empty());

        let a = activation("counter");
        store.save_plugin_activation(&project.id, &a).unwrap();

        let all = store.plugin_activations(&project.id).unwrap();
        assert_eq!(all, vec![a.clone()]);
    }

    #[test]
    fn a_saved_activation_is_updated_not_duplicated() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();

        store
            .save_plugin_activation(&project.id, &activation("counter"))
            .unwrap();

        let mut updated = activation("counter");
        updated.network = true;
        updated.code_hash = "new-hash".into();
        store.save_plugin_activation(&project.id, &updated).unwrap();

        let all = store.plugin_activations(&project.id).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].network);
        assert_eq!(all[0].code_hash, "new-hash");
    }
}
