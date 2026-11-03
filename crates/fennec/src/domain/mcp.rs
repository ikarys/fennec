//! MCP servers: what the human configured on the Settings page, how a
//! session's log reads back which of them are active, and the token cost of
//! their tools. No I/O: the live client lives in `crate::mcp`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use super::event::Event;
use super::preferences::Preferences;

/// The tool names fennec owns (host tools + the loop's own tools): an MCP tool
/// must not take one, or it would shadow a built-in in the model's view.
pub const BUILTIN_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "edit_file",
    "run_command",
    "load_skill",
    "web_search",
    "web_fetch",
    "ask_user",
    "request_waiver",
    "propose_criteria",
    "record_decision",
    "propose_task",
    "suggest_agent",
];

/// The wire transport of an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    /// A child process speaking newline-delimited JSON-RPC on stdio.
    #[default]
    Stdio,
    /// A streamable-HTTP endpoint speaking JSON-RPC.
    Http,
}

/// One MCP server as the human configured it on the Settings page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServer {
    /// Unique within the list; part of every tool name (`{name}_{tool}`).
    pub name: String,
    pub transport: McpTransport,
    /// stdio: the program to spawn.
    pub command: Option<String>,
    /// stdio: its arguments.
    pub args: Vec<String>,
    /// stdio: extra environment for the child.
    pub env: BTreeMap<String, String>,
    /// http: the endpoint.
    pub url: Option<String>,
    /// http: request headers.
    pub headers: BTreeMap<String, String>,
    /// Optional auth token: sent as a Bearer `Authorization` header (http) or
    /// as the `MCP_AUTH_TOKEN` env var (stdio).
    pub auth_token: Option<String>,
    /// Globally off: the server's tools are not sent to the model.
    pub disabled: bool,
    /// Hidden from the composer dropdown; independent of `disabled` (a server
    /// can be active but hidden, or shown but off).
    pub visible: bool,
    /// Tool names the human turned off for this server.
    pub disabled_tools: Vec<String>,
    /// The tool list from the last "Connect"; the definitions sent to the
    /// model come from here, so the frozen prompt stays byte-stable.
    pub cached_tools: Vec<McpTool>,
    /// Per-call timeout in seconds; 0 keeps the client's default (60).
    pub timeout: u64,
}

impl Default for McpServer {
    fn default() -> Self {
        Self {
            name: String::new(),
            transport: McpTransport::default(),
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            url: None,
            headers: BTreeMap::new(),
            auth_token: None,
            disabled: false,
            visible: true,
            disabled_tools: Vec::new(),
            cached_tools: Vec::new(),
            timeout: 0,
        }
    }
}

/// One tool of a server, as listed by `tools/list`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

impl McpServer {
    /// The problems with this server's config, empty when it is usable.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("the name is empty".into());
        } else if self.name.chars().any(char::is_whitespace) {
            out.push("the name must not contain spaces".into());
        }
        match self.transport {
            McpTransport::Stdio => {
                if self
                    .command
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
                {
                    out.push("a stdio server needs a command".into());
                }
            }
            McpTransport::Http => {
                let url = self.url.as_deref().unwrap_or_default();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    out.push("an http server needs an http(s) url".into());
                }
            }
        }
        // An MCP tool named `{server}_{tool}` must not take a built-in name,
        // or it would shadow the host tool in the model's view.
        for tool in &self.cached_tools {
            let name = format!("{}_{}", self.name, tool.name);
            if BUILTIN_TOOLS.contains(&name.as_str()) {
                out.push(format!("tool {name} collides with a built-in tool"));
            }
        }
        out
    }

    /// The tool names this server offers, minus the human's per-tool choices.
    pub fn tool_names(&self) -> Vec<String> {
        self.cached_tools
            .iter()
            .filter(|tool| !self.disabled_tools.contains(&tool.name))
            .map(|tool| format!("{}_{}", self.name, tool.name))
            .collect()
    }
}

/// The tool definition sent to the model for one tool, named `{server}_{tool}`
pub fn tool_definition(server: &McpServer, tool: &McpTool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": format!("{}_{}", server.name, tool.name),
            "description": tool.description.clone().unwrap_or_default(),
            "parameters": tool.input_schema.clone(),
        }
    })
}

/// Estimate: ceil(JSON bytes / 4).
pub fn estimate_tokens(definition: &Value) -> u64 {
    (definition.to_string().len() as u64).div_ceil(4)
}

/// The servers the session runs with: the global list with the per-session
/// `mcp.disabled` applied (the last event wins, like the model choice). A
/// `visible` server is still active; visibility only hides it from the
/// composer.
pub fn active_servers(events: &[Event], prefs: &Preferences) -> Vec<McpServer> {
    let disabled = events.iter().rev().find_map(|event| match event {
        Event::McpDisabled { servers } => Some(servers.clone()),
        _ => None,
    });
    prefs
        .mcp
        .iter()
        .filter(|server| match &disabled {
            Some(list) => !list.contains(&server.name),
            None => !server.disabled,
        })
        .cloned()
        .collect()
}

/// The tool names the session sends to the model: `{server}_{tool}` for every
/// active server's cached tools, minus the per-tool `disabledTools`, in a
/// stable order (server order, then tool order) so the same config yields
/// byte-identical definitions.
pub fn active_tool_names(servers: &[McpServer]) -> Vec<String> {
    servers.iter().flat_map(McpServer::tool_names).collect()
}

/// The `{server}_{tool}` names offered by more than one server, i.e. two
/// definitions that would share one name in the model's view.
pub fn duplicate_tool_names(servers: &[McpServer]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut duplicates: Vec<String> = Vec::new();
    for server in servers {
        for tool in &server.cached_tools {
            let name = format!("{}_{}", server.name, tool.name);
            if seen.contains(&name) && !duplicates.contains(&name) {
                duplicates.push(name.clone());
            }
            seen.push(name);
        }
    }
    duplicates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str, tools: &[&str], disabled_tools: &[&str]) -> McpServer {
        let mut server = McpServer {
            name: name.into(),
            ..Default::default()
        };
        server.cached_tools = tools
            .iter()
            .map(|name| McpTool {
                name: (*name).into(),
                description: None,
                input_schema: json!({"type": "object"}),
            })
            .collect();
        server.disabled_tools = disabled_tools.iter().map(|s| (*s).into()).collect();
        server
    }

    #[test]
    fn token_estimate_is_the_json_length_over_four() {
        let definition = json!({"type": "function", "function": {"name": "a", "description": "b", "parameters": {}}});
        assert_eq!(
            estimate_tokens(&definition),
            (definition.to_string().len() as u64).div_ceil(4)
        );
        assert!(estimate_tokens(&definition) > 0);
    }

    #[test]
    fn a_stdio_server_needs_a_command() {
        let problems = McpServer {
            name: "fs".into(),
            transport: McpTransport::Stdio,
            ..Default::default()
        }
        .problems();
        assert!(problems.iter().any(|p| p.contains("command")));
    }

    #[test]
    fn a_well_formed_server_has_no_problems() {
        let problems = McpServer {
            name: "fs".into(),
            transport: McpTransport::Stdio,
            command: Some("fs-mcp".into()),
            ..Default::default()
        }
        .problems();
        assert!(problems.is_empty());
    }

    #[test]
    fn an_http_server_needs_a_url() {
        let problems = McpServer {
            name: "api".into(),
            transport: McpTransport::Http,
            ..Default::default()
        }
        .problems();
        assert!(problems.iter().any(|p| p.contains("url")));
    }

    #[test]
    fn an_mcp_tool_cannot_take_a_builtin_name() {
        // server "read" + tool "file" == "read_file", a built-in.
        let problems = server("read", &["file"], &[]).problems();
        assert!(
            problems
                .iter()
                .any(|p| p.contains("collides with a built-in tool"))
        );
    }

    #[test]
    fn a_non_colliding_mcp_tool_has_no_problems() {
        let problems = server("fs", &["read", "write"], &[]).problems();
        assert!(!problems.iter().any(|p| p.contains("collides")));
    }

    #[test]
    fn tool_names_are_prefixed_and_respect_the_per_tool_choice() {
        let server = server("fs", &["read", "write"], &["write"]);
        assert_eq!(server.tool_names(), vec!["fs_read"]);
    }

    #[test]
    fn active_servers_use_the_global_disabled_flag_by_default() {
        let mut prefs = Preferences {
            mcp: vec![server("on", &["a"], &[]), server("off", &["b"], &[])],
            ..Default::default()
        };
        prefs.mcp[1].disabled = true;
        let events: Vec<Event> = vec![];
        assert_eq!(
            active_servers(&events, &prefs)
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>(),
            vec!["on"]
        );
    }

    #[test]
    fn the_last_mcp_disabled_event_wins_over_the_global_flag() {
        let prefs = Preferences {
            mcp: vec![server("a", &["x"], &[]), server("b", &["y"], &[])],
            ..Default::default()
        };
        let events = vec![
            Event::McpDisabled {
                servers: vec!["a".into()],
            },
            Event::McpDisabled {
                servers: vec!["b".into()],
            },
        ];
        assert_eq!(
            active_servers(&events, &prefs)
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
    }

    #[test]
    fn active_tool_names_follow_the_stable_order() {
        let servers = vec![server("a", &["p", "q"], &[]), server("b", &["r"], &[])];
        assert_eq!(active_tool_names(&servers), vec!["a_p", "a_q", "b_r"]);
    }

    #[test]
    fn a_cross_server_collision_shares_one_name() {
        // `a_b` + tool `tool` and `a` + tool `b_tool` both yield `a_b_tool`.
        let servers = vec![server("a_b", &["tool"], &[]), server("a", &["b_tool"], &[])];
        assert_eq!(duplicate_tool_names(&servers), vec!["a_b_tool"]);
    }

    #[test]
    fn distinct_tools_have_no_duplicate_names() {
        let servers = vec![server("a", &["p"], &[]), server("b", &["q"], &[])];
        assert!(duplicate_tool_names(&servers).is_empty());
    }
}
