//! MCP server configuration, tool access, and the token cost of tools. No
//! I/O: the live client lives in `crate::client`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
    /// Unique within the list; how the model names the server in `mcp_call`.
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
    /// The tool list from the last "Connect": the schemas `mcp_tools` returns and the catalog lists.
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

/// The per-tool hints an MCP server declares. Only the two hints fennec uses
/// are kept; the others are not part of the access grouping.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpToolAnnotations {
    pub read_only_hint: Option<bool>,
    pub destructive_hint: Option<bool>,
}

/// One tool of a server, as listed by `tools/list`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub annotations: Option<McpToolAnnotations>,
}

/// The access class a server declares for a tool. Provenance is declared, not
/// verified: the hints are indicative, not a guarantee of runtime behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolAccess {
    Read,
    Write,
    Destructive,
    Undeclared,
}

impl McpTool {
    /// The access class the server declares for this tool, using the MCP
    /// default: a tool that is not read-only is destructive unless the server
    /// explicitly says `destructiveHint` is false.
    pub fn access(&self) -> ToolAccess {
        match &self.annotations {
            None => ToolAccess::Undeclared,
            Some(annotations) if annotations.read_only_hint == Some(true) => ToolAccess::Read,
            Some(annotations) if annotations.destructive_hint == Some(false) => ToolAccess::Write,
            Some(_) => ToolAccess::Destructive,
        }
    }
}

impl McpServer {
    /// The access the server declared for one of its tools when it listed
    /// them; `Undeclared` for a tool it did not list.
    pub fn tool_access(&self, tool: &str) -> ToolAccess {
        self.cached_tools
            .iter()
            .find(|cached| cached.name == tool)
            .map_or(ToolAccess::Undeclared, McpTool::access)
    }

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
        out
    }

    /// The tools this server offers: its cached tools minus the human's
    /// per-tool choices, in cached order.
    pub fn offered_tools(&self) -> impl Iterator<Item = &McpTool> {
        self.cached_tools
            .iter()
            .filter(|tool| !self.disabled_tools.contains(&tool.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn access_is_undeclared_without_annotations() {
        let tool = McpTool {
            name: "x".into(),
            description: None,
            input_schema: json!({}),
            annotations: None,
        };
        assert_eq!(tool.access(), ToolAccess::Undeclared);
    }

    #[test]
    fn access_is_read_when_read_only_hint_is_true() {
        let tool = McpTool {
            name: "x".into(),
            description: None,
            input_schema: json!({}),
            annotations: Some(McpToolAnnotations {
                read_only_hint: Some(true),
                destructive_hint: None,
            }),
        };
        assert_eq!(tool.access(), ToolAccess::Read);
    }

    #[test]
    fn access_is_write_when_destructive_hint_is_false() {
        let tool = McpTool {
            name: "x".into(),
            description: None,
            input_schema: json!({}),
            annotations: Some(McpToolAnnotations {
                read_only_hint: None,
                destructive_hint: Some(false),
            }),
        };
        assert_eq!(tool.access(), ToolAccess::Write);
    }

    #[test]
    fn access_is_destructive_by_default_when_not_read_only_and_not_explicitly_non_destructive() {
        let tool = McpTool {
            name: "x".into(),
            description: None,
            input_schema: json!({}),
            annotations: Some(McpToolAnnotations {
                read_only_hint: None,
                destructive_hint: None,
            }),
        };
        assert_eq!(tool.access(), ToolAccess::Destructive);
    }

    #[test]
    fn a_tool_json_without_annotations_deserializes_with_none() {
        let tool: McpTool =
            serde_json::from_value(json!({ "name": "x", "inputSchema": {} })).unwrap();
        assert_eq!(tool.annotations, None);
    }
}
