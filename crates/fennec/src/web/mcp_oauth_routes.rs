//! Thin HTTP routes for MCP OAuth: start the flow and render the browser
//! callback page. The flow logic lives in `crate::mcp::oauth`.

use super::App;
use super::http::{Request, error, send, send_json};
use crate::mcp::oauth::Vault;
use crate::web::http::HttpError;
use serde_json::json;
use std::net::TcpStream;

/// `POST /api/settings/mcp/authorize`: return the URL the human opens.
pub(super) fn authorize(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let body = req.json()?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    if name.is_empty() {
        return error(stream, 400, "name is empty");
    }
    let store = app.store()?;
    let prefs = store.preferences()?;
    let Some(server) = prefs.mcp.iter().find(|server| server.name == name) else {
        return error(stream, 404, "no such mcp server");
    };
    if server.transport != crate::domain::mcp::McpTransport::Http {
        return error(stream, 400, "only an http server uses OAuth");
    }
    let redirect = format!("http://127.0.0.1:{}/api/mcp/oauth/callback", app.port);
    match crate::mcp::oauth::begin(server, &redirect) {
        Ok(url) => send_json(stream, 200, &json!({ "url": url })),
        Err(e) => error(stream, 502, &e),
    }
}

/// `GET /api/mcp/oauth/callback`: the browser lands here after authorization.
pub(super) fn callback(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let state = req.query_param("state").unwrap_or_default();
    if state.is_empty() {
        return html_page(stream, 400, "fennec: OAuth failed: missing state");
    }
    // ponytail: a refused consent leaves its pending entry in memory until restart.
    if let Some(error) = req.query_param("error") {
        return html_page(
            stream,
            400,
            &format!("fennec: OAuth failed: {}", html_escape(&error)),
        );
    }
    let code = req.query_param("code").unwrap_or_default();
    if code.is_empty() {
        return html_page(stream, 400, "fennec: OAuth failed: missing code");
    }
    match crate::mcp::oauth::finish(&state, &code, &Vault::for_settings(&app.settings)) {
        Ok(server) => html_page(
            stream,
            200,
            &format!(
                "fennec: {} is connected ({} tools). You can close this tab.",
                html_escape(&server.name),
                server.cached_tools.len()
            ),
        ),
        Err(message) => html_page(
            stream,
            502,
            &format!("fennec: OAuth failed: {}", html_escape(&message)),
        ),
    }
}

fn html_page(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), HttpError> {
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"></head><body><p>{body}</p></body></html>"
    );
    send(stream, status, "text/html; charset=utf-8", html.as_bytes())
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
