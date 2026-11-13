//! The network flow for OAuth 2.1 authorization of HTTP MCP servers.
//!
//! It performs discovery, dynamic client registration, PKCE, and token
//! exchange. It never stores credentials; it returns them in clear.

use std::fmt;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::json;

use crate::client::ClientInfo;
use crate::error::Error;
use crate::server::{McpServer, McpTransport};

use super::protocol::{
    AuthServerMetadata, Challenge, McpCredentials, Pkce, Registration, ResourceMetadata,
    TokenResponse, authorize_url, expires_at, parse_www_authenticate, well_known_as_urls,
    well_known_resource_url,
};

/// The state needed to finish an authorization-code flow after the browser
/// redirect.
#[derive(Clone)]
pub struct Pending {
    verifier: String,
    redirect: String,
    client_id: String,
    client_secret: Option<String>,
    token_endpoint: String,
    resource: Option<String>,
}

impl fmt::Debug for Pending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pending")
            .field("verifier", &"<redacted>")
            .field("redirect", &self.redirect)
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "<redacted>"),
            )
            .field("token_endpoint", &self.token_endpoint)
            .field("resource", &self.resource)
            .finish()
    }
}

/// The result of starting an OAuth flow.
#[derive(Debug)]
pub struct Authorization {
    pub url: String,
    pub state: String,
    pub pending: Pending,
}

/// Start the OAuth flow for one HTTP MCP server and return the URL the human
/// opens in a browser.
pub fn begin(
    server: &McpServer,
    redirect: &str,
    info: &ClientInfo,
) -> Result<Authorization, Error> {
    if server.transport != McpTransport::Http {
        return Err("only an http server uses OAuth".into());
    }
    let url = server
        .url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .ok_or("mcp http: no url")?;

    let challenge = discover_challenge(server, url, info)?;
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

    let registration = register(&registration_endpoint, redirect, info)?;
    let pkce = random_pkce()?;
    let state = random_state()?;

    let pending = Pending {
        verifier: pkce.verifier.clone(),
        redirect: redirect.to_string(),
        client_id: registration.client_id.clone(),
        client_secret: registration.client_secret.clone(),
        token_endpoint: as_metadata.token_endpoint.clone(),
        resource: resource_metadata.resource.clone(),
    };

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

    let url = authorize_url(&as_metadata.authorization_endpoint, &params);
    Ok(Authorization {
        url,
        state,
        pending,
    })
}

/// Complete the OAuth flow after the browser redirects back: exchange the code
/// and return clear credentials.
pub fn finish(pending: Pending, code: &str) -> Result<McpCredentials, Error> {
    let response = exchange_code(&pending, code)?;
    Ok(McpCredentials {
        client_id: pending.client_id,
        client_secret: pending.client_secret,
        token_endpoint: pending.token_endpoint,
        resource: pending.resource,
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        expires_at_ms: expires_at(now_ms(), response.expires_in),
    })
}

/// Refresh clear credentials with a refresh-token grant.
pub fn refresh(credentials: &McpCredentials) -> Result<McpCredentials, Error> {
    let refresh_token = credentials
        .refresh_token
        .as_deref()
        .ok_or("the authorization expired and has no refresh token")?;

    let mut form: Vec<(&str, String)> = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
        ("client_id", credentials.client_id.clone()),
    ];
    if let Some(resource) = &credentials.resource {
        form.push(("resource", resource.clone()));
    }
    if let Some(secret) = &credentials.client_secret {
        form.push(("client_secret", secret.clone()));
    }
    let form_refs: Vec<(&str, &str)> = form
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    let response = token_request(&credentials.token_endpoint, &form_refs)?;

    let mut out = credentials.clone();
    out.access_token = response.access_token;
    out.refresh_token = match response.refresh_token {
        Some(new_refresh) => Some(new_refresh),
        None => credentials.refresh_token.clone(),
    };
    out.expires_at_ms = expires_at(now_ms(), response.expires_in);
    Ok(out)
}

fn discover_challenge(
    server: &McpServer,
    url: &str,
    info: &ClientInfo,
) -> Result<Challenge, Error> {
    let agent = agent();
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": info.name, "version": info.version },
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
        )
        .into());
    }
    let header = response
        .headers()
        .get("www-authenticate")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let challenge = header
        .as_deref()
        .map(parse_www_authenticate)
        .unwrap_or_default();
    let resource_metadata = challenge
        .resource_metadata
        .or_else(|| well_known_resource_url(url))
        .ok_or("mcp oauth: no resource metadata")?;
    Ok(Challenge {
        resource_metadata: Some(resource_metadata),
        scope: challenge.scope,
    })
}

fn get_auth_server_metadata(issuer: &str) -> Result<AuthServerMetadata, Error> {
    for url in well_known_as_urls(issuer) {
        if let Ok(metadata) = get_json::<AuthServerMetadata>(&url) {
            return Ok(metadata);
        }
    }
    Err("mcp oauth: no authorization server metadata".into())
}

fn register(endpoint: &str, redirect: &str, info: &ClientInfo) -> Result<Registration, Error> {
    let body = json!({
        "client_name": info.name,
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
        return Err(format!("mcp oauth: registration endpoint returned {status}").into());
    }
    serde_json::from_str(&text).map_err(|_| "mcp oauth: invalid registration response".into())
}

fn exchange_code(pending: &Pending, code: &str) -> Result<TokenResponse, Error> {
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

fn token_request(endpoint: &str, form: &[(&str, &str)]) -> Result<TokenResponse, Error> {
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
        return Err(format!("mcp oauth: token endpoint returned {status}").into());
    }
    serde_json::from_str(&text).map_err(|_| "mcp oauth: invalid token response".into())
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, Error> {
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
        return Err(format!("mcp oauth: metadata endpoint returned {status}").into());
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

fn random_pkce() -> Result<Pkce, Error> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "mcp oauth: random bytes failed")?;
    Ok(Pkce::from_random(&bytes))
}

fn random_state() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "mcp oauth: random bytes failed")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_without_a_refresh_token_errors() {
        let credentials = McpCredentials {
            client_id: "client".into(),
            client_secret: None,
            token_endpoint: "https://id.example/token".into(),
            resource: None,
            access_token: "access".into(),
            refresh_token: None,
            expires_at_ms: Some(0),
        };
        let error = refresh(&credentials).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the authorization expired and has no refresh token"
        );
    }

    #[test]
    fn begin_on_a_stdio_server_errors() {
        let server = McpServer {
            name: "stdio".into(),
            transport: McpTransport::Stdio,
            command: Some("x".into()),
            ..Default::default()
        };
        let info = ClientInfo {
            name: "test",
            version: "0.0.0",
        };
        let error = begin(&server, "http://127.0.0.1/callback", &info).unwrap_err();
        assert_eq!(error.to_string(), "only an http server uses OAuth");
    }

    #[test]
    fn pending_debug_hides_the_verifier() {
        let pending = Pending {
            verifier: "verifier-secret".into(),
            redirect: "http://127.0.0.1/callback".into(),
            client_id: "client".into(),
            client_secret: Some("client-secret".into()),
            token_endpoint: "https://id.example/token".into(),
            resource: None,
        };
        let debug = format!("{pending:?}");
        assert!(debug.contains("verifier: \"<redacted>\""));
        assert!(debug.contains("client_secret: Some(\"<redacted>\")"));
        assert!(!debug.contains("verifier-secret"));
        assert!(!debug.contains("client-secret"));
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
    fn refresh_sends_the_refresh_grant_and_keeps_the_old_refresh_token() {
        let reply = r#"{"access_token":"new-access","token_type":"Bearer","expires_in":60}"#;
        let (url, body_rx) = token_server(reply);
        let credentials = McpCredentials {
            client_id: "client".into(),
            client_secret: None,
            token_endpoint: url,
            resource: None,
            access_token: "old".into(),
            refresh_token: Some("r1".into()),
            expires_at_ms: Some(0),
        };
        let refreshed = refresh(&credentials).unwrap();
        let body = body_rx.recv().unwrap();
        assert!(body.contains("grant_type=refresh_token"));
        assert!(body.contains("refresh_token=r1"));
        assert!(body.contains("client_id=client"));
        assert_eq!(refreshed.access_token, "new-access");
        assert_eq!(refreshed.refresh_token.as_deref(), Some("r1"));
        assert!(refreshed.expires_at_ms.is_some());
    }
}
