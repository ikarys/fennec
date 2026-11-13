//! MCP OAuth credentials in the existing `settings` table. The values are
//! sealed JSON: the store never sees clear tokens.

use super::{SqliteStore, StoreError};
use crate::domain::clock::now_ms;
use mcp_lite::oauth::McpCredentials;
use rusqlite::{OptionalExtension, params};

fn key(server: &str) -> String {
    format!("mcp_oauth:{server}")
}

impl SqliteStore {
    /// The stored OAuth credentials for one MCP server, if any.
    pub fn mcp_credentials(
        &self,
        server: &str,
    ) -> std::result::Result<Option<McpCredentials>, StoreError> {
        let k = key(server);
        let json: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![k],
                |r| r.get(0),
            )
            .optional()?;
        match json {
            Some(json) => Ok(Some(
                serde_json::from_str(&json).map_err(|e| StoreError::Json(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// Upserts the sealed OAuth credentials for one MCP server.
    pub fn save_mcp_credentials(
        &self,
        server: &str,
        credentials: &McpCredentials,
    ) -> std::result::Result<(), StoreError> {
        let k = key(server);
        let json =
            serde_json::to_string(credentials).map_err(|e| StoreError::Json(e.to_string()))?;
        let now = now_ms().to_string();
        self.db.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![k, json, now],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::SqliteStore;
    use mcp_lite::oauth::McpCredentials;

    fn credentials() -> McpCredentials {
        McpCredentials {
            client_id: "client".into(),
            client_secret: Some("sealed-secret".into()),
            token_endpoint: "https://id.example/token".into(),
            resource: Some("https://mcp.example".into()),
            access_token: "sealed-access".into(),
            refresh_token: Some("sealed-refresh".into()),
            expires_at_ms: Some(1_000),
        }
    }

    #[test]
    fn mcp_credentials_round_trip() {
        let store = SqliteStore::in_memory().unwrap();
        assert_eq!(store.mcp_credentials("notion").unwrap(), None);

        store
            .save_mcp_credentials("notion", &credentials())
            .unwrap();
        assert_eq!(
            store.mcp_credentials("notion").unwrap(),
            Some(credentials())
        );

        store.save_mcp_credentials("other", &credentials()).unwrap();
        assert_eq!(
            store.mcp_credentials("notion").unwrap(),
            Some(credentials())
        );
    }
}
