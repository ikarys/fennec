//! The `settings` table: one row per named setting, JSON values.

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use crate::domain::preferences::Preferences;
use rusqlite::{OptionalExtension, params};

const KEY: &str = "preferences";

impl SqliteStore {
    /// The stored preferences, or the defaults when none are stored yet.
    pub fn preferences(&self) -> std::result::Result<Preferences, StoreError> {
        let json: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![KEY],
                |r| r.get(0),
            )
            .optional()?;
        match json {
            Some(json) => serde_json::from_str(&json).map_err(|e| StoreError::Json(e.to_string())),
            None => Ok(Preferences::default()),
        }
    }

    /// Upserts the stored preferences.
    pub fn save_preferences(&self, prefs: &Preferences) -> std::result::Result<(), StoreError> {
        let json = serde_json::to_string(prefs).map_err(|e| StoreError::Json(e.to_string()))?;
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![KEY, json, now],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::SqliteStore;
    use crate::domain::preferences::{ModelPrefs, Preferences};

    #[test]
    fn preferences_round_trip_and_default_when_absent() {
        let store = SqliteStore::in_memory().unwrap();
        assert_eq!(store.preferences().unwrap(), Preferences::default());

        let mut prefs = Preferences {
            model: Some("m".into()),
            ..Default::default()
        };
        prefs.models.insert(
            "m".into(),
            ModelPrefs {
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.5),
            },
        );
        prefs.display.reasoning_open = false;

        store.save_preferences(&prefs).unwrap();
        assert_eq!(store.preferences().unwrap(), prefs);

        prefs.model = Some("n".into());
        store.save_preferences(&prefs).unwrap();
        assert_eq!(store.preferences().unwrap().model.as_deref(), Some("n"));
    }
}
