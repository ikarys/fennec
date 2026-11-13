//! Pure OAuth 2.1 / MCP authorization helpers. No I/O: randomness and the
//! clock are passed in, so the flow can be tested without a network.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};

/// The parts of a `WWW-Authenticate: Bearer ...` challenge the flow needs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Challenge {
    pub resource_metadata: Option<String>,
    pub scope: Option<String>,
}

/// Parse the quoted parameters of a `WWW-Authenticate` header. Parameter names
/// are case-insensitive; values are kept as-is after removing quotes.
pub(crate) fn parse_www_authenticate(header: &str) -> Challenge {
    let mut out = Challenge::default();
    for part in split_params(header) {
        let part = part.trim();
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        // The first parameter follows the scheme: `Bearer resource_metadata=…`.
        let key = key
            .split_whitespace()
            .last()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let value = value
            .trim()
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or(value.trim())
            .to_string();
        match key.as_str() {
            "resource_metadata" => out.resource_metadata = Some(value),
            "scope" => out.scope = Some(value),
            _ => {}
        }
    }
    out
}

/// The protected-resource metadata URL for an MCP server URL.
pub(crate) fn well_known_resource_url(server_url: &str) -> Option<String> {
    let origin = origin_of(server_url)?;
    Some(format!("{origin}/.well-known/oauth-protected-resource"))
}

/// The two well-known authorization-server metadata URLs, in fallback order.
pub(crate) fn well_known_as_urls(issuer: &str) -> [String; 2] {
    let issuer = issuer.trim_end_matches('/');
    [
        format!("{issuer}/.well-known/oauth-authorization-server"),
        format!("{issuer}/.well-known/openid-configuration"),
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ResourceMetadata {
    pub resource: Option<String>,
    pub authorization_servers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AuthServerMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub registration_endpoint: Option<String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Registration {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registration")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

impl fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// PKCE S256 pair. The verifier is base64url(random bytes); the challenge is
/// base64url(SHA-256(verifier)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub(crate) fn from_random(bytes: &[u8; 32]) -> Pkce {
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = challenge_for(&verifier);
        Pkce {
            verifier,
            challenge,
        }
    }
}

/// The S256 challenge for a PKCE verifier.
pub(crate) fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()).as_ref())
}

/// RFC 3986 percent-encoding, keeping the unreserved characters.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Build an authorization URL. Values are percent-encoded; the first parameter
/// uses `?` unless the endpoint already has a query string.
pub(crate) fn authorize_url(endpoint: &str, params: &[(&str, &str)]) -> String {
    let mut out = endpoint.to_string();
    let mut separator = if endpoint.contains('?') { "&" } else { "?" };
    for (key, value) in params {
        out.push_str(separator);
        out.push_str(&percent_encode(key));
        out.push('=');
        out.push_str(&percent_encode(value));
        separator = "&";
    }
    out
}

/// OAuth credentials for one MCP server. The token fields hold whatever the
/// caller puts there: clear values from this crate; a caller may seal them
/// before storing.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCredentials {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub token_endpoint: String,
    pub resource: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_ms: Option<u64>,
}

impl fmt::Debug for McpCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpCredentials")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "<redacted>"),
            )
            .field("token_endpoint", &self.token_endpoint)
            .field("resource", &self.resource)
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

/// The expiry time for a token response, in milliseconds.
pub(crate) fn expires_at(now_ms: u64, expires_in: Option<u64>) -> Option<u64> {
    expires_in.map(|seconds| now_ms.saturating_add(seconds.saturating_mul(1000)))
}

impl McpCredentials {
    /// True when the token is expired or expires within 60 s. Unknown expiry
    /// never forces a refresh.
    pub fn needs_refresh(&self, now_ms: u64) -> bool {
        match self.expires_at_ms {
            Some(expires_at) => now_ms.saturating_add(60_000) >= expires_at,
            None => false,
        }
    }
}

fn origin_of(url: &str) -> Option<String> {
    let scheme_end = url.find("://")?;
    let rest = &url[scheme_end + 3..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() {
        return None;
    }
    Some(format!("{}://{}", &url[..scheme_end], authority))
}

/// Split on commas that are not inside double quotes.
fn split_params(header: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    for (i, byte) in header.bytes().enumerate() {
        if byte == b'"' {
            in_quotes = !in_quotes;
        }
        if byte == b',' && !in_quotes {
            out.push(&header[start..i]);
            start = i + 1;
        }
    }
    out.push(&header[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_notion_www_authenticate_challenge() {
        let header = r#"Bearer realm="OAuth", resource_metadata="https://mcp.notion.com/.well-known/oauth-protected-resource/mcp", scope="default""#;
        let challenge = parse_www_authenticate(header);
        assert_eq!(
            challenge.resource_metadata.as_deref(),
            Some("https://mcp.notion.com/.well-known/oauth-protected-resource/mcp")
        );
        assert_eq!(challenge.scope.as_deref(), Some("default"));
    }

    #[test]
    fn www_authenticate_names_are_case_insensitive() {
        let challenge = parse_www_authenticate(r#"Bearer SCOPE="a b", RESOURCE_METADATA="x""#);
        assert_eq!(challenge.scope.as_deref(), Some("a b"));
        assert_eq!(challenge.resource_metadata.as_deref(), Some("x"));
    }

    #[test]
    fn well_known_urls_use_the_origin() {
        assert_eq!(
            well_known_resource_url("https://mcp.notion.com/mcp").as_deref(),
            Some("https://mcp.notion.com/.well-known/oauth-protected-resource")
        );
        assert_eq!(
            well_known_resource_url("http://127.0.0.1:8443/mcp?x=1").as_deref(),
            Some("http://127.0.0.1:8443/.well-known/oauth-protected-resource")
        );
        assert_eq!(well_known_resource_url("not-a-url"), None);
    }

    #[test]
    fn well_known_as_urls_trims_a_trailing_slash() {
        let urls = well_known_as_urls("https://id.example/");
        assert_eq!(
            urls[0],
            "https://id.example/.well-known/oauth-authorization-server"
        );
        assert_eq!(
            urls[1],
            "https://id.example/.well-known/openid-configuration"
        );
    }

    #[test]
    fn pkce_matches_rfc_7636_appendix_b() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn pkce_from_random_uses_base64url_verifier() {
        let bytes = [7u8; 32];
        let pkce = Pkce::from_random(&bytes);
        assert_eq!(pkce.verifier, URL_SAFE_NO_PAD.encode(bytes));
        assert_eq!(pkce.challenge, challenge_for(&pkce.verifier));
    }

    #[test]
    fn percent_encode_keeps_unreserved_and_escapes_the_rest() {
        assert_eq!(percent_encode("aZ0-._~"), "aZ0-._~");
        assert_eq!(percent_encode("a b/c?d=e"), "a%20b%2Fc%3Fd%3De");
    }

    #[test]
    fn authorize_url_appends_query_with_the_right_separator() {
        let url = authorize_url(
            "https://id.example/authorize",
            &[("response_type", "code"), ("a b", "c d")],
        );
        assert_eq!(
            url,
            "https://id.example/authorize?response_type=code&a%20b=c%20d"
        );

        let url = authorize_url(
            "https://id.example/authorize?x=1",
            &[("response_type", "code")],
        );
        assert_eq!(url, "https://id.example/authorize?x=1&response_type=code");
    }

    #[test]
    fn expires_at_and_needs_refresh_use_the_60_second_window() {
        assert_eq!(expires_at(1_000, Some(30)), Some(31_000));
        assert_eq!(expires_at(1_000, None), None);

        let fresh = McpCredentials {
            client_id: "c".into(),
            client_secret: None,
            token_endpoint: "t".into(),
            resource: None,
            access_token: "a".into(),
            refresh_token: None,
            expires_at_ms: Some(100_000),
        };
        assert!(!fresh.needs_refresh(1_000));
        assert!(fresh.needs_refresh(40_001));
        assert!(fresh.needs_refresh(100_000));

        let unknown = McpCredentials {
            expires_at_ms: None,
            ..fresh
        };
        assert!(!unknown.needs_refresh(1_000_000));
    }

    #[test]
    fn mcp_credentials_debug_redacts_secret_fields() {
        let credentials = McpCredentials {
            client_id: "client".into(),
            client_secret: Some("client-secret".into()),
            token_endpoint: "https://id.example/token".into(),
            resource: Some("resource".into()),
            access_token: "access-token".into(),
            refresh_token: Some("refresh-token".into()),
            expires_at_ms: Some(123),
        };
        let debug = format!("{credentials:?}");
        assert!(debug.contains("client_id: \"client\""));
        assert!(debug.contains("client_secret: Some(\"<redacted>\")"));
        assert!(debug.contains("token_endpoint: \"https://id.example/token\""));
        assert!(debug.contains("resource: Some(\"resource\")"));
        assert!(debug.contains("access_token: \"<redacted>\""));
        assert!(debug.contains("refresh_token: Some(\"<redacted>\")"));
        assert!(debug.contains("expires_at_ms: Some(123)"));
        assert!(!debug.contains("client-secret"));
        assert!(!debug.contains("access-token"));
        assert!(!debug.contains("refresh-token"));
    }
}
