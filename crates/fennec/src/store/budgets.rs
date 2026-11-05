//! The per-project monthly budgets: one row of the `settings` table each.

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use rusqlite::{OptionalExtension, params};

impl SqliteStore {
    /// The project's monthly budget in dollars, `None` when the human set none.
    pub fn project_budget(&self, project_id: &str) -> std::result::Result<Option<f64>, StoreError> {
        let key = format!("budget:{project_id}");
        let json: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        match json {
            Some(json) => {
                let limit: f64 =
                    serde_json::from_str(&json).map_err(|e| StoreError::Json(e.to_string()))?;
                Ok(Some(limit))
            }
            None => Ok(None),
        }
    }

    /// Sets the project's monthly budget; `None` deletes the row.
    pub fn set_project_budget(
        &self,
        project_id: &str,
        limit: Option<f64>,
    ) -> std::result::Result<(), StoreError> {
        let key = format!("budget:{project_id}");
        match limit {
            Some(limit) => {
                let json =
                    serde_json::to_string(&limit).map_err(|e| StoreError::Json(e.to_string()))?;
                let now = now_ms().to_string();
                self.db.execute(
                    "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                    params![key, json, now],
                )?;
            }
            None => {
                self.db
                    .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::SqliteStore;

    #[test]
    fn project_budget_round_trips_and_deletes() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/budget").unwrap();

        assert_eq!(store.project_budget(&project.id).unwrap(), None);

        store.set_project_budget(&project.id, Some(100.0)).unwrap();
        assert_eq!(store.project_budget(&project.id).unwrap(), Some(100.0));

        store.set_project_budget(&project.id, Some(200.0)).unwrap();
        assert_eq!(store.project_budget(&project.id).unwrap(), Some(200.0));

        store.set_project_budget(&project.id, None).unwrap();
        assert_eq!(store.project_budget(&project.id).unwrap(), None);
    }
}
