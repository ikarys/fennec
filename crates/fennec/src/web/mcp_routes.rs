//! MCP routes: the session's MCP state for the composer, its per-session
//! toggle, and the Settings page's "Connect" (a live `tools/list` whose result
//! is cached in the preferences, so the frozen prompt stays byte-stable).

use super::App;
use super::http::{Request, error, send_json};
use crate::agent::ports::EventLog;
use crate::domain::event::Event;
use crate::domain::mcp;
use crate::domain::preferences::Preferences;
use crate::tools::Sandbox;
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
                        "tokens": mcp::estimate_tokens(&mcp::tool_definition(server, tool)),
                    })
                })
                .collect();
            let tokens: u64 = server
                .cached_tools
                .iter()
                .filter(|tool| !server.disabled_tools.contains(&tool.name))
                .map(|tool| mcp::estimate_tokens(&mcp::tool_definition(server, tool)))
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

/// `GET /api/sessions/{id}/mcp`: the session's MCP state and token totals.
pub(super) fn session_mcp(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    if app.session_and_project(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let events = store.events(id)?;
    let prefs = store.preferences()?;
    send_json(stream, 200, &mcp_view(&prefs, &events))
}

/// `POST /api/sessions/{id}/mcp`: record the session's disabled MCP servers.
pub(super) fn set_session_mcp(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let disabled = body["disabled"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let store = app.store()?;
    if app.session_and_project(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    super::session_routes::append_human_event(
        app,
        &store,
        id,
        Some(Event::McpDisabled { servers: disabled }),
    )?;
    let events = store.events(id)?;
    let prefs = store.preferences()?;
    send_json(stream, 200, &mcp_view(&prefs, &events))
}

/// `POST /api/settings/mcp/connect`: open one server, list its tools, and cache
/// the list in the preferences.
pub(super) fn connect(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
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

    use crate::config::{LlmSettings, Settings};
    use crate::domain::preferences::{ModelParams, SandboxPrefs};
    use crate::notifier::Notifier;
    use crate::web::http::Request;
    use crate::web::turns::Turns;
    use std::collections::HashMap;
    use std::io::Read;
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn app() -> App {
        // A real file, not `:memory:`: `store()` reopens the db on every call,
        // so an in-memory db would lose the saved preferences.
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_path =
            std::env::temp_dir().join(format!("fennec-mcp-test-{}-{}.db", std::process::id(), n));
        let settings = Settings {
            llm: LlmSettings {
                base_url: "http://localhost".into(),
                api_key: None,
                defaults: ModelParams {
                    provider: "local".into(),
                    model: "test".into(),
                    max_tokens: 100,
                    context_window: 1000,
                    compact_at: 0.85,
                    reasoning_effort: None,
                },
                listed: Vec::new(),
            },
            db_path,
        };
        let notifier = Arc::new(Notifier::default());
        App {
            turns: Turns::new(settings.clone(), notifier.clone()),
            settings,
            port: 0,
            notifier: notifier.clone(),
            skill_folds: Mutex::new(HashMap::new()),
            idea_reports: Mutex::new(HashMap::new()),
            idea_queues: Mutex::default(),
            worktree_scan: Mutex::default(),
            server_runs: Arc::new(Mutex::new(HashMap::new())),
            plugins: Arc::new(crate::plugins::PluginManager::new(
                crate::tools::Sandbox::Off,
                std::env::temp_dir(),
                std::env::temp_dir(),
                notifier,
            )),
            agents_home: std::env::temp_dir(),
            update_check: Default::default(),
        }
    }

    /// Reads a full HTTP response (head then body) off one end of a pipe.
    fn read_response(stream: &mut TcpStream) -> (u16, String) {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf).to_string();
        let head_end = text.find("\r\n\r\n").unwrap();
        let head = &text[..head_end];
        let body = text[head_end + 4..].to_string();
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, body)
    }

    /// A connected localhost pipe: the handler writes to `writer`, the test
    /// reads the response off `reader` (closing `writer` gives it an EOF).
    fn pipe() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let reader = TcpStream::connect(addr).unwrap();
        let (writer, _) = listener.accept().unwrap();
        (writer, reader)
    }

    #[test]
    fn connect_refuses_a_tool_that_collides_with_a_builtin() {
        let app = app();
        let store = app.store().unwrap();
        // A stdio server named `read` that lists a tool `file`: `read_file`
        // is a built-in, so the connect must refuse to cache it.
        let script = r#"
read line
echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2.0","capabilities":{},"serverInfo":{"name":"read","version":"0"}}}'
read line
read line
echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"file","description":"f","inputSchema":{"type":"object"}}]}}'
"#;
        let mut prefs = Preferences {
            mcp: vec![McpServer {
                name: "read".into(),
                transport: McpTransport::Stdio,
                command: Some("sh".into()),
                args: vec!["-c".into(), script.into()],
                ..Default::default()
            }],
            ..Default::default()
        };
        prefs.sandbox = SandboxPrefs {
            enabled: false,
            network: false,
        };
        store.save_preferences(&prefs).unwrap();

        let req = Request {
            method: "POST".into(),
            path: "/api/settings/mcp/connect".into(),
            query: String::new(),
            headers: Vec::new(),
            body: serde_json::to_vec(&json!({ "name": "read" })).unwrap(),
        };
        let (mut writer, mut reader) = pipe();
        connect(&app, &req, &mut writer).unwrap();
        drop(writer);
        let (status, body) = read_response(&mut reader);
        assert_eq!(status, 400);
        assert!(body.contains("collides with a built-in tool"));
        // The collision was not persisted: the tool list stayed empty.
        assert!(store.preferences().unwrap().mcp[0].cached_tools.is_empty());
    }

    #[test]
    fn connect_refuses_a_tool_that_collides_with_another_server() {
        let app = app();
        let store = app.store().unwrap();
        // `a_b` already offers `a_b_tool`; connecting `a` which lists `b_tool`
        // would yield a second `a_b_tool`.
        let existing = server("a_b", &["tool"]);
        let script = r#"
read line
echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2.0","capabilities":{},"serverInfo":{"name":"a","version":"0"}}}'
read line
read line
echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"b_tool","description":"t","inputSchema":{"type":"object"}}]}}'
"#;
        let mut prefs = Preferences {
            mcp: vec![
                existing,
                McpServer {
                    name: "a".into(),
                    transport: McpTransport::Stdio,
                    command: Some("sh".into()),
                    args: vec!["-c".into(), script.into()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        prefs.sandbox = SandboxPrefs {
            enabled: false,
            network: false,
        };
        store.save_preferences(&prefs).unwrap();

        let req = Request {
            method: "POST".into(),
            path: "/api/settings/mcp/connect".into(),
            query: String::new(),
            headers: Vec::new(),
            body: serde_json::to_vec(&json!({ "name": "a" })).unwrap(),
        };
        let (mut writer, mut reader) = pipe();
        connect(&app, &req, &mut writer).unwrap();
        drop(writer);
        let (status, body) = read_response(&mut reader);
        assert_eq!(status, 400);
        assert!(body.contains("offered by more than one mcp server"));
    }
}
