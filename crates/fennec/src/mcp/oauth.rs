//! OAuth 2.1 authorization for HTTP MCP servers: storage and sealing of MCP
//! OAuth credentials; the protocol lives in `mcp_lite::oauth`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use llm_providers_connector::MasterKey;
use mcp_lite::oauth::{self, McpCredentials};

use crate::domain::clock::now_ms;
use crate::domain::mcp::{McpServer, McpTransport};
use crate::locks::lock;
use crate::mcp::INFO;
use crate::sandbox::Sandbox;
use crate::store::SqliteStore;

/// The sealed-credential store and master key location for MCP OAuth.
#[derive(Debug, Clone)]
pub struct Vault {
    pub db: PathBuf,
    pub secret: PathBuf,
}

/// A pending authorization, keyed by the PKCE `state`.
// ponytail: in memory, lost on restart; the human clicks again.
static PENDING: LazyLock<Mutex<HashMap<String, (String, oauth::Pending)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl Vault {
    /// The vault next to fennec's store, sealed with its `secret.key`.
    pub fn for_settings(settings: &crate::config::Settings) -> Vault {
        Vault {
            db: settings.db_path.clone(),
            secret: settings.secret_path(),
        }
    }

    /// Return an HTTP server with a clear bearer token from stored credentials,
    /// refreshing first when needed. Stdio servers and servers without
    /// credentials are returned unchanged.
    pub fn authorize(&self, server: &McpServer) -> Result<McpServer, String> {
        if server.transport != McpTransport::Http {
            return Ok(server.clone());
        }
        let Some(mut credentials) = self.load_credentials(&server.name)? else {
            return Ok(server.clone());
        };
        if credentials.needs_refresh(now_ms()) {
            if credentials.refresh_token.is_none() {
                return Err(format!(
                    "mcp {}: the authorization expired; authorize it again on the Settings page",
                    server.name
                ));
            }
            credentials = self.refresh(&server.name, &credentials)?;
        }
        let access_token = self.open_secret(&credentials.access_token)?;
        let mut out = server.clone();
        out.auth_token = Some(access_token);
        Ok(out)
    }

    fn load_credentials(&self, server: &str) -> Result<Option<McpCredentials>, String> {
        let store = self.store()?;
        store.mcp_credentials(server).map_err(|e| e.to_string())
    }

    fn save_credentials(&self, server: &str, credentials: &McpCredentials) -> Result<(), String> {
        let store = self.store()?;
        store
            .save_mcp_credentials(server, credentials)
            .map_err(|e| e.to_string())
    }

    fn store(&self) -> Result<SqliteStore, String> {
        SqliteStore::open(&self.db).map_err(|e| e.to_string())
    }

    fn master_key(&self) -> Result<MasterKey, String> {
        MasterKey::load_or_create(&self.secret)
    }

    fn open_secret(&self, value: &str) -> Result<String, String> {
        self.master_key()?.open(value)
    }

    /// Seal every secret field of clear credentials.
    fn seal_credentials(&self, credentials: &McpCredentials) -> Result<McpCredentials, String> {
        let key = self.master_key()?;
        map_secrets(credentials, |value| Ok(key.seal(value)))
    }

    /// Open every secret field of sealed credentials.
    fn open_credentials(&self, credentials: &McpCredentials) -> Result<McpCredentials, String> {
        let key = self.master_key()?;
        map_secrets(credentials, |value| key.open(value))
    }

    fn refresh(
        &self,
        server: &str,
        credentials: &McpCredentials,
    ) -> Result<McpCredentials, String> {
        let clear = self.open_credentials(credentials)?;
        let refreshed = oauth::refresh(&clear).map_err(|e| e.to_string())?;
        let sealed = self.seal_credentials(&refreshed)?;
        self.save_credentials(server, &sealed)?;
        Ok(sealed)
    }
}

/// Start the OAuth flow for one saved HTTP MCP server and return the URL the
/// human opens in a browser.
pub fn begin(server: &McpServer, redirect: &str) -> Result<String, String> {
    let authorization = oauth::begin(server, redirect, &INFO).map_err(|e| e.to_string())?;
    lock(&PENDING).insert(
        authorization.state.clone(),
        (server.name.clone(), authorization.pending),
    );
    Ok(authorization.url)
}

/// Complete the OAuth flow after the browser redirects back: exchange the code,
/// store sealed credentials, and run the shared connect/cache logic.
pub fn finish(state: &str, code: &str, vault: &Vault) -> Result<McpServer, String> {
    let (server_name, pending) = take_pending(state)
        .ok_or("unknown or expired authorization; start again from the Settings page")?;
    let clear = oauth::finish(pending, code).map_err(|e| e.to_string())?;
    let credentials = vault.seal_credentials(&clear)?;
    vault.save_credentials(&server_name, &credentials)?;

    let store = vault.store()?;
    let mut prefs = store.preferences().map_err(|e| e.to_string())?;
    let index = prefs
        .mcp
        .iter()
        .position(|server| server.name == server_name)
        .ok_or("no such mcp server")?;
    let sandbox = Sandbox::for_store(&prefs.sandbox, &vault.db);
    crate::mcp::connect_and_cache(&store, &mut prefs, index, &sandbox, vault)
        .map_err(|(_, message)| message)
}

/// The credentials with `f` applied to the client secret and both tokens.
fn map_secrets(
    credentials: &McpCredentials,
    f: impl Fn(&str) -> Result<String, String>,
) -> Result<McpCredentials, String> {
    Ok(McpCredentials {
        client_secret: credentials.client_secret.as_deref().map(&f).transpose()?,
        access_token: f(&credentials.access_token)?,
        refresh_token: credentials.refresh_token.as_deref().map(&f).transpose()?,
        ..credentials.clone()
    })
}

fn take_pending(state: &str) -> Option<(String, oauth::Pending)> {
    lock(&PENDING).remove(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_servers_are_unchanged_by_the_vault() {
        let vault = Vault {
            db: std::env::temp_dir().join("fennec-oauth-vault-test.db"),
            secret: std::env::temp_dir().join("fennec-oauth-vault-test.key"),
        };
        let server = McpServer {
            name: "stdio".into(),
            transport: McpTransport::Stdio,
            command: Some("x".into()),
            ..Default::default()
        };
        let out = vault.authorize(&server).unwrap();
        assert_eq!(out, server);
    }

    #[test]
    fn seal_and_open_credentials_round_trip() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let vault = Vault {
            db: std::env::temp_dir().join(format!("fennec-oauth-vault-seal-test-{nanos}.db")),
            secret: std::env::temp_dir().join(format!("fennec-oauth-vault-seal-test-{nanos}.key")),
        };
        let clear = McpCredentials {
            client_id: "client".into(),
            client_secret: Some("client-secret".into()),
            token_endpoint: "https://id.example/token".into(),
            resource: Some("resource".into()),
            access_token: "access-token".into(),
            refresh_token: Some("refresh-token".into()),
            expires_at_ms: Some(123),
        };
        let sealed = vault.seal_credentials(&clear).unwrap();
        assert_ne!(sealed.access_token, clear.access_token);
        assert_ne!(sealed.client_secret.as_deref(), Some("client-secret"));
        assert_ne!(sealed.refresh_token.as_deref(), Some("refresh-token"));
        let opened = vault.open_credentials(&sealed).unwrap();
        assert_eq!(opened, clear);
        let _ = std::fs::remove_file(&vault.db);
        let _ = std::fs::remove_file(&vault.secret);
    }

    fn token_server(reply: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            while !buf.ends_with(b"\r\n\r\n") {
                if stream.read(&mut byte).unwrap() == 0 {
                    break;
                }
                buf.push(byte[0]);
            }
            let header_end = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let mut parts = line.splitn(2, ':');
                    let key = parts.next()?.trim().to_ascii_lowercase();
                    let value = parts.next()?.trim();
                    (key == "content-length").then(|| value.parse::<usize>().ok())
                })
                .flatten()
                .unwrap_or(0);
            let mut body = vec![0u8; content_length];
            let mut read = 0;
            while read < content_length {
                let n = stream.read(&mut body[read..]).unwrap();
                if n == 0 {
                    break;
                }
                read += n;
            }
            if read < content_length {
                body.truncate(read);
            }
            tx.send(String::from_utf8_lossy(&body).to_string()).unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        (format!("http://127.0.0.1:{port}/token"), rx)
    }

    #[test]
    fn an_expired_authorization_refreshes_with_the_clear_refresh_token() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let vault = Vault {
            db: std::env::temp_dir().join(format!("fennec-oauth-vault-refresh-test-{nanos}.db")),
            secret: std::env::temp_dir()
                .join(format!("fennec-oauth-vault-refresh-test-{nanos}.key")),
        };
        let reply = r#"{"access_token":"new-access","token_type":"Bearer","expires_in":60}"#;
        let (url, body_rx) = token_server(reply);
        let clear = McpCredentials {
            client_id: "client".into(),
            client_secret: None,
            token_endpoint: url,
            resource: None,
            access_token: "old".into(),
            refresh_token: Some("r1".into()),
            expires_at_ms: Some(0),
        };
        let sealed = vault.seal_credentials(&clear).unwrap();
        vault.save_credentials("api", &sealed).unwrap();

        let server = McpServer {
            name: "api".into(),
            transport: McpTransport::Http,
            url: Some("http://127.0.0.1:1/mcp".into()),
            ..Default::default()
        };
        let authorized = vault.authorize(&server).unwrap();
        assert_eq!(authorized.auth_token.as_deref(), Some("new-access"));
        let body = body_rx.recv().unwrap();
        assert!(body.contains("refresh_token=r1"));

        let stored = vault.load_credentials("api").unwrap().unwrap();
        assert_ne!(stored.access_token, "new-access");
        let opened = vault.open_credentials(&stored).unwrap();
        assert_eq!(opened.access_token, "new-access");

        std::fs::remove_file(&vault.db).ok();
        std::fs::remove_file(&vault.secret).ok();
    }
}
