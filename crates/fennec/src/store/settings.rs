//! The `settings` table: one row per named setting, JSON values.

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use crate::domain::preferences::Preferences;
use rusqlite::{OptionalExtension, params};

const KEY: &str = "preferences";
const SEEN_VERSION: &str = "seen_version";

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
            Some(json) => {
                let mut prefs: Preferences =
                    serde_json::from_str(&json).map_err(|e| StoreError::Json(e.to_string()))?;
                prefs.migrate();
                Ok(prefs)
            }
            None => Ok(Preferences::default()),
        }
    }

    /// Upserts the stored preferences.
    pub fn save_preferences(&self, prefs: &Preferences) -> std::result::Result<(), StoreError> {
        // A key in clear never reaches the database.
        if prefs.has_plain_keys() {
            return Err(StoreError::PlainSecret);
        }

        let json = serde_json::to_string(prefs).map_err(|e| StoreError::Json(e.to_string()))?;
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![KEY, json, now],
        )?;
        Ok(())
    }

    /// The fennec version whose changelog the human last saw; `None` before the first look.
    pub fn seen_version(&self) -> std::result::Result<Option<String>, StoreError> {
        let value: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![SEEN_VERSION],
                |r| r.get(0),
            )
            .optional()?;
        Ok(value)
    }

    /// Records the version whose changelog the human saw.
    pub fn set_seen_version(&self, version: &str) -> std::result::Result<(), StoreError> {
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![SEEN_VERSION, version, now],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{SqliteStore, StoreError};
    use crate::domain::preferences::{
        ApiKey, LOCAL_PROVIDER, ModelPrefs, ModelRef, Preferences, Provider,
    };
    use std::collections::BTreeMap;

    #[test]
    fn preferences_round_trip_and_default_when_absent() {
        let store = SqliteStore::in_memory().unwrap();
        assert_eq!(store.preferences().unwrap(), Preferences::default());

        let mut models = BTreeMap::new();
        models.insert(
            "m".into(),
            ModelPrefs {
                price: None,
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.5),
                reasoning_effort: None,
            },
        );
        let mut prefs = Preferences {
            selected: Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "m".into(),
            }),
            providers: vec![Provider {
                id: LOCAL_PROVIDER.to_string(),
                name: String::new(),
                base_url: String::new(),
                api_key: None,
                models,
            }],
            ..Default::default()
        };
        prefs.display.reasoning_open = false;

        store.save_preferences(&prefs).unwrap();
        assert_eq!(store.preferences().unwrap(), prefs);

        prefs.selected = Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "n".into(),
        });
        store.save_preferences(&prefs).unwrap();
        assert_eq!(
            store.preferences().unwrap().selected,
            Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "n".into(),
            })
        );
    }

    #[test]
    fn a_legacy_row_loads_migrated() {
        let store = SqliteStore::in_memory().unwrap();
        let json = r#"{"model":"m","models":{"m":{"maxTokens":5}}}"#;
        store
            .db
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('preferences', ?1, '0')",
                [json],
            )
            .unwrap();

        let prefs = store.preferences().unwrap();
        assert_eq!(
            prefs.selected,
            Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "m".into(),
            })
        );
        assert_eq!(
            prefs
                .provider(LOCAL_PROVIDER)
                .unwrap()
                .models
                .get("m")
                .unwrap()
                .max_tokens,
            Some(5)
        );

        store.save_preferences(&prefs).unwrap();
        assert_eq!(store.preferences().unwrap(), prefs);
    }

    #[test]
    fn a_plain_key_is_refused() {
        let store = SqliteStore::in_memory().unwrap();
        let before = Preferences::default();
        store.save_preferences(&before).unwrap();

        let mut prefs = Preferences::default();
        prefs.providers.push(Provider {
            id: LOCAL_PROVIDER.to_string(),
            name: String::new(),
            base_url: String::new(),
            api_key: Some(ApiKey::Plain("secret".into())),
            models: BTreeMap::new(),
        });

        match store.save_preferences(&prefs) {
            Err(StoreError::PlainSecret) => {}
            _ => panic!("expected PlainSecret"),
        }
        assert_eq!(store.preferences().unwrap(), before);
    }

    #[test]
    fn seen_version_absent_round_trip_and_overwrite() {
        let store = SqliteStore::in_memory().unwrap();
        assert_eq!(store.seen_version().unwrap(), None);
        store.set_seen_version("0.1.0").unwrap();
        assert_eq!(store.seen_version().unwrap(), Some("0.1.0".to_string()));
        store.set_seen_version("0.2.0").unwrap();
        assert_eq!(store.seen_version().unwrap(), Some("0.2.0".to_string()));
    }
}
