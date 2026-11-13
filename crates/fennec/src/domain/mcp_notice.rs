//! Tells the model which MCP servers it can call
//! (`mcp_tools` / `mcp_call`). The tool list is frozen with the prompt, so a server turned on or off mid-session is
//! announced with an appended user message rather than a new tool list: the prompt only grows.

use crate::domain::compaction::model_view;
use crate::domain::event::{Event, Role};
use crate::domain::mcp::{self, McpServer};

const NOTICE_PREFIX: &str = "<system-reminder>\nMCP servers in this session";
const EMPTY_PREFIX: &str = "<system-reminder>\nNo MCP server is active";

/// The last MCP catalog the model was told, if any.
fn stated(events: &[Event]) -> Option<String> {
    for event in model_view(events).iter().rev() {
        if let Event::MessageStart {
            content: Some(content),
            role: Role::User,
            is_system_generated: true,
            ..
        } = event
            && (content.starts_with(NOTICE_PREFIX) || content.starts_with(EMPTY_PREFIX))
        {
            return Some(content.clone());
        }
    }
    None
}

/// The MCP catalog to append when the model's stated catalog is not current.
pub fn needed(events: &[Event], servers: &[McpServer]) -> Option<String> {
    let text = mcp::catalog(servers);
    match stated(events) {
        None => servers
            .iter()
            .any(|server| server.offered_tools().next().is_some())
            .then_some(text),
        Some(stated) if stated == text => None,
        Some(_) => Some(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::McpTool;
    use serde_json::json;

    fn server(name: &str) -> McpServer {
        McpServer {
            name: name.into(),
            cached_tools: vec![McpTool {
                name: "tool".into(),
                description: None,
                input_schema: json!({"type": "object"}),
                annotations: None,
            }],
            ..Default::default()
        }
    }

    fn notice(content: String) -> Event {
        Event::MessageStart {
            message_id: "m".into(),
            role: Role::User,
            content: Some(content),
            is_system_generated: true,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn nothing_stated_and_no_server_needs_no_notice() {
        let events: Vec<Event> = Vec::new();
        assert_eq!(needed(&events, &[]), None);
    }

    #[test]
    fn nothing_stated_and_a_server_needs_the_catalog() {
        let events: Vec<Event> = Vec::new();
        let servers = vec![server("a")];
        let needed = needed(&events, &servers).expect("catalog");
        assert_eq!(needed, mcp::catalog(&servers));
    }

    #[test]
    fn same_catalog_stated_needs_no_notice() {
        let servers = vec![server("a")];
        let events = vec![notice(mcp::catalog(&servers))];
        assert_eq!(needed(&events, &servers), None);
    }

    #[test]
    fn a_server_turned_off_after_a_stated_catalog_needs_the_new_text() {
        let initial = vec![server("a"), server("b")];
        let current = vec![server("b")];
        let events = vec![notice(mcp::catalog(&initial))];
        let needed = needed(&events, &current).expect("new catalog");
        assert_eq!(needed, mcp::catalog(&current));
    }

    #[test]
    fn all_servers_off_after_a_stated_catalog_needs_the_no_server_text() {
        let initial = vec![server("a")];
        let events = vec![notice(mcp::catalog(&initial))];
        let needed = needed(&events, &[]).expect("no-server text");
        assert_eq!(needed, mcp::catalog(&[]));
    }
}
