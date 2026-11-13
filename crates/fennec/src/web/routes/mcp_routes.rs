//! MCP routes: the session's MCP state for the composer, its per-session
//! toggle, and the Settings page's "Connect" (a live `tools/list` whose result
//! is cached in the preferences, so the frozen prompt stays byte-stable).

use crate::agent::ports::EventLog;
use crate::domain::event::Event;
use crate::domain::mcp;
use crate::domain::preferences::Preferences;
use crate::sandbox::Sandbox;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use serde_json::{Value, json};
use std::net::TcpStream;

/// The MCP servers as the composer sees them: which are active for the session
/// and the token cost of their tools.
fn mcp_view(prefs: &Preferences, events: &[Event]) -> Value {
    let active: Vec<String> = mcp::active_servers(events, prefs)
        .iter()
        .map(|server| server.name.clone())
        .collect();
    let servers: Vec<Value> = prefs
        .mcp
        .iter()
        .map(|server| {
            let is_active = active.contains(&server.name);
            let tools: Vec<Value> = server
                .cached_tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "enabled": !server.disabled_tools.contains(&tool.name),
                        "tokens": mcp::estimate_tokens(tool),
                    })
                })
                .collect();
            let tokens: u64 = server
                .cached_tools
                .iter()
                .filter(|tool| !server.disabled_tools.contains(&tool.name))
                .map(mcp::estimate_tokens)
                .sum();
            json!({
                "name": server.name,
                "visible": server.visible,
                "disabled": server.disabled,
                "active": is_active,
                "tokens": if is_active { tokens } else { 0 },
                // What switching it on would add: shown on an inactive server too.
                "enabledToolTokens": tokens,
                "toolCount": server.cached_tools.len(),
                "tools": tools,
            })
        })
        .collect();
    let total: u64 = servers
        .iter()
        .filter_map(|server| server["tokens"].as_u64())
        .sum();
    json!({ "servers": servers, "totalTokens": total })
}

/// The disabled array a request carries, as server names; `None` when absent or null.
pub(crate) fn disabled_servers(value: &Value) -> Option<Vec<String>> {
    let items = value.as_array()?;
    Some(
        items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
    )
}

/// `GET /api/sessions/{id}/mcp`: the session's MCP state and token totals.
pub(crate) fn session_mcp(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    app.require_session(id)?;
    let events = store.events(id)?;
    let prefs = store.preferences()?;
    send_json(stream, 200, &mcp_view(&prefs, &events))
}

/// `POST /api/sessions/{id}/mcp`: record the session's disabled MCP servers.
pub(crate) fn set_session_mcp(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let body = req.json()?;
    let disabled = disabled_servers(&body["disabled"]).unwrap_or_default();
    let store = app.store()?;
    app.require_session(id)?;
    crate::web::routes::session_routes::append_human_event(
        app,
        &store,
        id,
        Some(Event::McpDisabled { servers: disabled }),
    )?;
    let events = store.events(id)?;
    let prefs = store.preferences()?;
    send_json(stream, 200, &mcp_view(&prefs, &events))
}

/// `POST /api/mcp`: the MCP view a new session would get, so the draft's
/// composer can show it before the first message creates the session.
pub(crate) fn draft_mcp(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let body = req.json()?;
    let store = app.store()?;
    let prefs = store.preferences()?;
    let view = match disabled_servers(&body["disabled"]) {
        None => mcp_view(&prefs, &[]),
        Some(servers) => mcp_view(&prefs, &[Event::McpDisabled { servers }]),
    };
    send_json(stream, 200, &view)
}

/// `POST /api/settings/mcp/connect`: open one server, list its tools, and cache
/// the list in the preferences.
pub(crate) fn connect(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let body = req.json()?;
    let name = body["name"].as_str().unwrap_or("").to_string();
    if name.is_empty() {
        return error(stream, 400, "name is empty");
    }
    let store = app.store()?;
    let mut prefs = store.preferences()?;
    let Some(index) = prefs.mcp.iter().position(|server| server.name == name) else {
        return error(stream, 404, "no such mcp server");
    };
    let sandbox = Sandbox::for_store(&prefs.sandbox, &app.settings.db_path);
    let vault = crate::mcp::oauth::Vault::for_settings(&app.settings);
    match crate::mcp::connect_and_cache(&store, &mut prefs, index, &sandbox, &vault) {
        Ok(server) => send_json(stream, 200, &json!({ "server": server })),
        Err((status, message)) => error(stream, status, &message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::{McpServer, McpTool, McpTransport};

    fn server(name: &str, tools: &[&str]) -> McpServer {
        let mut server = McpServer {
            name: name.into(),
            transport: McpTransport::Stdio,
            command: Some("x".into()),
            ..Default::default()
        };
        server.cached_tools = tools
            .iter()
            .map(|tool| McpTool {
                annotations: None,
                name: (*tool).into(),
                description: None,
                input_schema: json!({ "type": "object" }),
            })
            .collect();
        server
    }

    #[test]
    fn the_view_reports_active_servers_and_token_totals() {
        let prefs = Preferences {
            mcp: vec![server("a", &["p", "q"]), server("b", &["r"])],
            ..Default::default()
        };
        let view = mcp_view(&prefs, &[]);
        let servers = view["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0]["name"], "a");
        assert_eq!(servers[0]["active"], true);
        assert!(servers[0]["tokens"].as_u64().unwrap() > 0);
        assert!(view["totalTokens"].as_u64().unwrap() > 0);
    }

    #[test]
    fn a_disabled_server_is_not_active_and_costs_no_tokens() {
        let mut prefs = Preferences {
            mcp: vec![server("a", &["p"])],
            ..Default::default()
        };
        prefs.mcp[0].disabled = true;
        let view = mcp_view(&prefs, &[]);
        let server = &view["servers"].as_array().unwrap()[0];
        assert_eq!(server["active"], false);
        assert_eq!(server["tokens"], 0);
        assert_eq!(view["totalTokens"], 0);
    }

    #[test]
    fn the_session_toggle_overrides_the_global_flag() {
        let prefs = Preferences {
            mcp: vec![server("a", &["p"]), server("b", &["q"])],
            ..Default::default()
        };
        let events = vec![Event::McpDisabled {
            servers: vec!["a".into()],
        }];
        let view = mcp_view(&prefs, &events);
        let servers = view["servers"].as_array().unwrap();
        assert_eq!(servers[0]["active"], false);
        assert_eq!(servers[1]["active"], true);
    }

    #[test]
    fn a_draft_disabled_server_is_inactive() {
        let prefs = Preferences {
            mcp: vec![server("a", &["p"]), server("b", &["q"])],
            ..Default::default()
        };
        let view = mcp_view(
            &prefs,
            &[Event::McpDisabled {
                servers: vec!["a".into()],
            }],
        );
        let servers = view["servers"].as_array().unwrap();
        assert_eq!(servers[0]["active"], false);
        assert_eq!(servers[1]["active"], true);
    }
}
