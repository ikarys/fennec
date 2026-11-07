//! OAuth 2.1 authorization for HTTP MCP servers: discovery, dynamic client
//! registration, PKCE, the local redirect callback, and token refresh.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use llm_providers_connector::MasterKey;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::json;

use crate::domain::clock::now_ms;
use crate::domain::mcp::{McpServer, McpTransport};
use crate::domain::mcp_oauth::{
    self as domain, AuthServerMetadata, McpCredentials, Pkce, Registration, ResourceMetadata,
    TokenResponse,
};
use crate::store::SqliteStore;
use crate::tools::Sandbox;

/// The sealed-credential store and master key location for MCP OAuth.
#[derive(Debug, Clone)]
pub struct Vault {
    pub db: PathBuf,
    pub secret: PathBuf,
}

/// A pending authorization, keyed by the PKCE `state`.
// ponytail: in memory, lost on restart; the human clicks again.
struct Pending {
    server_name: String,
    verifier: String,
    redirect: String,
    client_id: String,
    client_secret: Option<String>,
    token_endpoint: String,
    resource: Option<String>,
}

static PENDING: LazyLock<Mutex<HashMap<String, Pending>>> =
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
            let Some(refresh_token) = credentials.refresh_token.clone() else {
                return Err(format!(
                    "mcp {}: the authorization expired; authorize it again on the Settings page",
                    server.name
                ));
            };
            credentials = self.refresh(&server.name, &credentials, &refresh_token)?;
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

    fn seal(&self, value: &str) -> Result<String, String> {
        Ok(self.master_key()?.seal(value))
    }

    fn open_secret(&self, value: &str) -> Result<String, String> {
        self.master_key()?.open(value)
    }

    fn refresh(
        &self,
        server: &str,
        credentials: &McpCredentials,
        refresh_token: &str,
    ) -> Result<McpCredentials, String> {
        let client_secret = match &credentials.client_secret {
            Some(sealed) => Some(self.open_secret(sealed)?),
            None => None,
        };
        let mut form: Vec<(&str, String)> = vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.to_string()),
            ("client_id", credentials.client_id.clone()),
        ];
        if let Some(resource) = &credentials.resource {
            form.push(("resource", resource.clone()));
        }
        if let Some(secret) = client_secret {
            form.push(("client_secret", secret));
        }
        let form_refs: Vec<(&str, &str)> = form
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        let response = token_request(&credentials.token_endpoint, &form_refs)?;
        let mut out = credentials.clone();
        out.access_token = self.seal(&response.access_token)?;
        out.refresh_token = match response.refresh_token {
            Some(new_refresh) => Some(self.seal(&new_refresh)?),
            None => credentials.refresh_token.clone(),
        };
        out.expires_at_ms = domain::expires_at(now_ms(), response.expires_in);
        self.save_credentials(server, &out)?;
        Ok(out)
    }
}

/// Start the OAuth flow for one saved HTTP MCP server and return the URL the
/// human opens in a browser.
pub fn begin(server: &McpServer, redirect: &str) -> Result<String, String> {
    if server.transport != McpTransport::Http {
        return Err("only an http server uses OAuth".into());
    }
    let url = server
        .url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .ok_or("mcp http: no url")?;

    let challenge = discover_challenge(server, url)?;
    let resource_metadata_url = challenge
        .resource_metadata
        .ok_or("mcp oauth: no resource metadata")?;
    let resource_metadata = get_json::<ResourceMetadata>(&resource_metadata_url)?;
    let authorization_server = resource_metadata
        .authorization_servers
        .first()
        .cloned()
        .ok_or("mcp oauth: no authorization server")?;
    let as_metadata = get_auth_server_metadata(&authorization_server)?;
    let registration_endpoint = as_metadata
        .registration_endpoint
        .ok_or("this server does not support dynamic client registration")?;

    let registration = register(&registration_endpoint, redirect)?;
    let pkce = random_pkce()?;
    let state = random_state()?;

    let pending = Pending {
        server_name: server.name.clone(),
        verifier: pkce.verifier.clone(),
        redirect: redirect.to_string(),
        client_id: registration.client_id.clone(),
        client_secret: registration.client_secret.clone(),
        token_endpoint: as_metadata.token_endpoint.clone(),
        resource: resource_metadata.resource.clone(),
    };
    crate::locks::lock(&PENDING).insert(state.clone(), pending);

    let mut params: Vec<(&str, &str)> = vec![
        ("response_type", "code"),
        ("client_id", registration.client_id.as_str()),
        ("redirect_uri", redirect),
        ("code_challenge", pkce.challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", state.as_str()),
    ];
    if let Some(resource) = &resource_metadata.resource {
        params.push(("resource", resource));
    }
    if let Some(scope) = &challenge.scope {
        params.push(("scope", scope));
    }
    Ok(domain::authorize_url(
        &as_metadata.authorization_endpoint,
        &params,
    ))
}

/// Complete the OAuth flow after the browser redirects back: exchange the code,
/// store sealed credentials, and run the shared connect/cache logic.
pub fn finish(state: &str, code: &str, vault: &Vault) -> Result<McpServer, String> {
    let pending = take_pending(state)
        .ok_or("unknown or expired authorization; start again from the Settings page")?;
    let response = exchange_code(&pending, code)?;

    let client_secret = match &pending.client_secret {
        Some(secret) => Some(vault.seal(secret)?),
        None => None,
    };
    let credentials = McpCredentials {
        client_id: pending.client_id.clone(),
        client_secret,
        token_endpoint: pending.token_endpoint.clone(),
        resource: pending.resource.clone(),
        access_token: vault.seal(&response.access_token)?,
        refresh_token: response
            .refresh_token
            .as_deref()
            .map(|token| vault.seal(token))
            .transpose()?,
        expires_at_ms: domain::expires_at(now_ms(), response.expires_in),
    };
    vault.save_credentials(&pending.server_name, &credentials)?;

    let store = vault.store()?;
    let mut prefs = store.preferences().map_err(|e| e.to_string())?;
    let index = prefs
        .mcp
        .iter()
        .position(|server| server.name == pending.server_name)
        .ok_or("no such mcp server")?;
    let sandbox = Sandbox::for_store(&prefs.sandbox, &vault.db);
    crate::mcp::connect_and_cache(&store, &mut prefs, index, &sandbox, vault)
        .map_err(|(_, message)| message)
}

fn take_pending(state: &str) -> Option<Pending> {
    crate::locks::lock(&PENDING).remove(state)
}

fn discover_challenge(server: &McpServer, url: &str) -> Result<domain::Challenge, String> {
    let agent = agent();
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "fennec", "version": env!("CARGO_PKG_VERSION") },
        },
    });
    let response = agent
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .send_json(&body)
        .map_err(|e| format!("mcp oauth: discovery request failed: {e}"))?;
    let status = response.status();
    if status != 401 {
        return Err(format!(
            "mcp {}: the server did not return a 401 challenge",
            server.name
        ));
    }
    let header = response
        .headers()
        .get("www-authenticate")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let challenge = header
        .as_deref()
        .map(domain::parse_www_authenticate)
        .unwrap_or_default();
    let resource_metadata = challenge
        .resource_metadata
        .or_else(|| domain::well_known_resource_url(url))
        .ok_or("mcp oauth: no resource metadata")?;
    Ok(domain::Challenge {
        resource_metadata: Some(resource_metadata),
        scope: challenge.scope,
    })
}

fn get_auth_server_metadata(issuer: &str) -> Result<AuthServerMetadata, String> {
    for url in domain::well_known_as_urls(issuer) {
        if let Ok(metadata) = get_json::<AuthServerMetadata>(&url) {
            return Ok(metadata);
        }
    }
    Err("mcp oauth: no authorization server metadata".into())
}

fn register(endpoint: &str, redirect: &str) -> Result<Registration, String> {
    let body = json!({
        "client_name": "fennec",
        "redirect_uris": [redirect],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    let response = agent()
        .post(endpoint)
        .header("content-type", "application/json")
        .send_json(&body)
        .map_err(|_| "mcp oauth: registration request failed")?;
    let status = response.status();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|_| "mcp oauth: registration response body failed")?;
    if !status.is_success() {
        return Err(format!(
            "mcp oauth: registration endpoint returned {status}"
        ));
    }
    serde_json::from_str(&text).map_err(|_| "mcp oauth: invalid registration response".into())
}

fn exchange_code(pending: &Pending, code: &str) -> Result<TokenResponse, String> {
    let mut form: Vec<(&str, String)> = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("redirect_uri", pending.redirect.clone()),
        ("client_id", pending.client_id.clone()),
        ("code_verifier", pending.verifier.clone()),
    ];
    if let Some(resource) = &pending.resource {
        form.push(("resource", resource.clone()));
    }
    if let Some(secret) = &pending.client_secret {
        form.push(("client_secret", secret.clone()));
    }
    let form_refs: Vec<(&str, &str)> = form
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    token_request(&pending.token_endpoint, &form_refs)
}

fn token_request(endpoint: &str, form: &[(&str, &str)]) -> Result<TokenResponse, String> {
    let response = agent()
        .post(endpoint)
        .send_form(form.iter().copied())
        .map_err(|_| "mcp oauth: token request failed")?;
    let status = response.status();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|_| "mcp oauth: token response body failed")?;
    if !status.is_success() {
        return Err(format!("mcp oauth: token endpoint returned {status}"));
    }
    serde_json::from_str(&text).map_err(|_| "mcp oauth: invalid token response".into())
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let response = agent()
        .get(url)
        .header("accept", "application/json")
        .call()
        .map_err(|e| format!("mcp oauth: metadata request failed: {e}"))?;
    let status = response.status();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|_| "mcp oauth: metadata response body failed")?;
    if !status.is_success() {
        return Err(format!("mcp oauth: metadata endpoint returned {status}"));
    }
    serde_json::from_str(&text).map_err(|_| "mcp oauth: invalid metadata response".into())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn random_pkce() -> Result<Pkce, String> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "mcp oauth: random bytes failed")?;
    Ok(Pkce::from_random(&bytes))
}

fn random_state() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "mcp oauth: random bytes failed")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
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
}
