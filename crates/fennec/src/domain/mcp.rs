//! MCP servers: what the human configured on the Settings page and how a
//! session's log reads back which of them are active. The types, the lazy
//! tool layer and the client live in the `mcp-lite` crate.

pub use mcp_lite::{
    META_TOOLS, McpServer, McpTransport, ToolAccess, catalog, check_arguments, estimate_tokens,
    meta_tool_definitions, tool_schemas,
};

use super::event::Event;
use super::preferences::Preferences;

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

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_lite::McpTool;
    use serde_json::json;

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
                annotations: None,
            })
            .collect();
        server.disabled_tools = disabled_tools.iter().map(|s| (*s).into()).collect();
        server
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
}
