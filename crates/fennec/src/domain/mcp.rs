//! MCP servers: what the human configured on the Settings page, how a
//! session's log reads back which of them are active, and the token cost of
//! their tools. No I/O: the live client lives in `crate::mcp`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use super::event::Event;
use super::preferences::Preferences;

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

/// The two fixed tool names fennec declares for lazy MCP access.
pub const META_TOOLS: [&str; 2] = ["mcp_tools", "mcp_call"];

/// The two fixed OpenAI tool definitions for lazy MCP access. They are
/// byte-stable because they are frozen in prompts.
pub fn meta_tool_definitions() -> Vec<Value> {
    let mcp_tools = json!({
        "type": "function",
        "function": {
            "name": "mcp_tools",
            "description": "Returns the JSON schemas of the named tools of an MCP server listed in the MCP catalog. Call it before mcp_call when the catalog does not list a tool's parameters.",
            "parameters": {
                "type": "object",
                "properties": {
                    "server": {"type": "string"},
                    "tools": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tool names from the catalog."
                    }
                },
                "required": ["server", "tools"]
            }
        }
    });
    let mcp_call = json!({
        "type": "function",
        "function": {
            "name": "mcp_call",
            "description": "Calls a tool of an MCP server listed in the MCP catalog. `arguments` must match the tool's schema.",
            "parameters": {
                "type": "object",
                "properties": {
                    "server": {"type": "string"},
                    "tool": {"type": "string"},
                    "arguments": {"type": "object"}
                },
                "required": ["server", "tool", "arguments"]
            }
        }
    });
    vec![mcp_tools, mcp_call]
}

/// The user-message catalog of the MCP servers the model may call.
/// Deterministic; only re-sent when its rendered text changes.
pub fn catalog(servers: &[McpServer]) -> String {
    let mut lines = vec!["<system-reminder>".to_string()];
    let mut server_lines: Vec<String> = Vec::new();
    let mut any = false;

    for server in servers {
        let tools: Vec<&McpTool> = server.offered_tools().collect();
        if tools.is_empty() {
            continue;
        }
        any = true;
        server_lines.push(format!("- {}:", server.name));
        let include_params = tools.len() <= 8;

        for tool in tools {
            let sentence = first_sentence(tool.description.as_deref());
            if include_params {
                let params = params(&tool.input_schema);
                if sentence.is_empty() {
                    server_lines.push(format!("  - {}({params})", tool.name));
                } else {
                    server_lines.push(format!("  - {}({params}): {sentence}", tool.name));
                }
            } else if sentence.is_empty() {
                server_lines.push(format!("  - {}", tool.name));
            } else {
                server_lines.push(format!("  - {}: {sentence}", tool.name));
            }
        }
    }

    if any {
        lines.push(
            "MCP servers in this session (this list replaces any earlier one; a server not listed is off and its calls are refused):"
                .to_string(),
        );
        lines.extend(server_lines);
        lines.push(
            "Call a tool with mcp_call(server, tool, arguments). For a tool listed without parameters, get its schema first with mcp_tools(server, [tools])."
                .to_string(),
        );
    } else {
        lines.push(
            "No MCP server is active in this session now: MCP calls are refused.".to_string(),
        );
    }
    lines.push("</system-reminder>".to_string());
    lines.join("\n")
}

/// The JSON schemas of the requested offered tools, in request order.
/// An empty request returns every offered tool.
pub fn tool_schemas(server: &McpServer, tools: &[String]) -> Result<Value, String> {
    let offered: Vec<&McpTool> = server.offered_tools().collect();
    let offered_names: Vec<String> = offered.iter().map(|tool| tool.name.clone()).collect();

    if tools.is_empty() {
        return Ok(Value::Array(
            offered.iter().map(|tool| tool_schema_value(tool)).collect(),
        ));
    }

    let mut schemas = Vec::with_capacity(tools.len());
    for name in tools {
        let tool = offered
            .iter()
            .find(|tool| tool.name == name.as_str())
            .ok_or_else(|| {
                format!(
                    "{} has no tool {name}. Its tools: {}",
                    server.name,
                    offered_names.join(", ")
                )
            })?;
        schemas.push(tool_schema_value(tool));
    }
    Ok(Value::Array(schemas))
}

/// A light check that `arguments` can be sent to `tool`.
pub fn check_arguments(tool: &McpTool, arguments: &Value) -> Result<(), String> {
    let schema = &tool.input_schema;
    let arguments = match arguments.as_object() {
        Some(arguments) => arguments,
        None => {
            return Err(format!(
                "arguments must be a JSON object.\nSchema of {}: {schema}",
                tool.name
            ));
        }
    };

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required {
            let Some(name) = name.as_str() else {
                continue;
            };
            if !arguments.contains_key(name) {
                return Err(format!(
                    "arguments is missing required field {name}.\nSchema of {}: {schema}",
                    tool.name
                ));
            }
        }
    }

    let properties = schema.get("properties").and_then(Value::as_object);
    for (key, value) in arguments {
        if let Some(property) = properties.and_then(|properties| properties.get(key))
            && let Some(Value::String(expected)) = property.get("type")
            && !matches_type(value, expected)
        {
            return Err(format!(
                "field {key} must be {expected}.\nSchema of {}: {schema}",
                tool.name
            ));
        }
    }

    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        for key in arguments.keys() {
            let declared = properties.is_some_and(|properties| properties.contains_key(key));
            if !declared {
                return Err(format!(
                    "field {key} is not allowed.\nSchema of {}: {schema}",
                    tool.name
                ));
            }
        }
    }

    Ok(())
}

/// Estimate: ceil(JSON bytes / 4).
pub fn estimate_tokens(tool: &McpTool) -> u64 {
    let definition = json!({
        "name": &tool.name,
        "description": tool.description.as_deref().unwrap_or_default(),
        "parameters": &tool.input_schema,
    });
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

fn tool_schema_value(tool: &McpTool) -> Value {
    json!({
        "name": &tool.name,
        "description": tool.description.as_deref().unwrap_or_default(),
        "inputSchema": &tool.input_schema,
    })
}

fn first_sentence(description: Option<&str>) -> String {
    let mut text = description.unwrap_or_default().trim();
    if let Some(pos) = text.find('\n') {
        text = &text[..pos];
    }
    if let Some((pos, _)) = text.char_indices().find(|(pos, c)| {
        *c == '.'
            && (pos + 1 == text.len()
                || text[pos + 1..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace))
    }) {
        text = &text[..pos];
    }
    text.trim().to_string()
}

fn params(schema: &Value) -> String {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return String::new();
    };
    let required = schema.get("required").and_then(Value::as_array);
    properties
        .keys()
        .map(|key| {
            let is_required = required
                .as_ref()
                .is_some_and(|required| required.iter().any(|name| name.as_str() == Some(key)));
            if is_required {
                key.clone()
            } else {
                format!("{key}?")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn matches_type(value: &Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    }
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
                annotations: None,
            })
            .collect();
        server.disabled_tools = disabled_tools.iter().map(|s| (*s).into()).collect();
        server
    }

    fn tool(name: &str, description: Option<&str>, schema: Value) -> McpTool {
        McpTool {
            name: name.into(),
            description: description.map(str::to_string),
            input_schema: schema,
            annotations: None,
        }
    }

    #[test]
    fn token_estimate_is_the_json_length_over_four() {
        let tool = tool("a", Some("b"), json!({"type": "object"}));
        let definition = json!({
            "name": "a",
            "description": "b",
            "parameters": {"type": "object"},
        });
        assert_eq!(
            estimate_tokens(&tool),
            (definition.to_string().len() as u64).div_ceil(4)
        );
        assert!(estimate_tokens(&tool) > 0);
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
    fn catalog_with_a_small_server_shows_params_with_optional_marks() {
        let servers = vec![McpServer {
            name: "fs".into(),
            cached_tools: vec![tool(
                "search",
                Some("Finds things. More detail."),
                json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "limit": {"type": "integer"}
                    },
                    "required": ["query"]
                }),
            )],
            ..Default::default()
        }];
        let catalog = catalog(&servers);
        assert!(catalog.contains("  - search(limit?, query): Finds things"));
    }

    #[test]
    fn a_server_with_nine_tools_shows_names_without_params() {
        let tools = (0..9)
            .map(|i| tool(&format!("t{i}"), None, json!({"type": "object"})))
            .collect();
        let servers = vec![McpServer {
            name: "many".into(),
            cached_tools: tools,
            ..Default::default()
        }];
        let catalog = catalog(&servers);
        assert!(catalog.contains("- many:"));
        assert!(catalog.contains("  - t0\n"));
        assert!(!catalog.contains("t0("));
    }

    #[test]
    fn disabled_tools_are_left_out_of_the_catalog() {
        let servers = vec![McpServer {
            name: "fs".into(),
            cached_tools: vec![
                tool("a", None, json!({"type": "object"})),
                tool("b", None, json!({"type": "object"})),
            ],
            disabled_tools: vec!["b".into()],
            ..Default::default()
        }];
        let catalog = catalog(&servers);
        assert!(catalog.contains("  - a"));
        assert!(!catalog.contains("  - b"));
    }

    #[test]
    fn no_active_server_gives_the_no_server_text() {
        let catalog = catalog(&[]);
        assert_eq!(
            catalog,
            "<system-reminder>\nNo MCP server is active in this session now: MCP calls are refused.\n</system-reminder>"
        );
    }

    #[test]
    fn catalog_is_deterministic_for_the_same_input() {
        let servers = vec![McpServer {
            name: "fs".into(),
            cached_tools: vec![tool("a", Some("Does a. More."), json!({"type": "object"}))],
            ..Default::default()
        }];
        assert_eq!(catalog(&servers), catalog(&servers));
    }

    #[test]
    fn tool_schemas_returns_requested_order_and_errors_on_unknown_name() {
        let server = McpServer {
            name: "fs".into(),
            cached_tools: vec![
                tool("a", None, json!({"type": "object"})),
                tool("b", None, json!({"type": "object"})),
            ],
            ..Default::default()
        };

        let schemas = tool_schemas(&server, &["b".into(), "a".into()]).unwrap();
        assert_eq!(
            schemas
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );

        let all = tool_schemas(&server, &[]).unwrap();
        assert_eq!(all.as_array().unwrap().len(), 2);

        let err = tool_schemas(&server, &["z".into()]).unwrap_err();
        assert!(err.contains("fs has no tool z"));
        assert!(err.contains("a, b"));
    }

    #[test]
    fn check_arguments_missing_required() {
        let tool = tool(
            "t",
            None,
            json!({
                "type": "object",
                "properties": {"a": {"type": "string"}},
                "required": ["a"]
            }),
        );
        let err = check_arguments(&tool, &json!({})).unwrap_err();
        assert!(err.contains("missing required field a"));
        assert!(err.contains("Schema of t"));
    }

    #[test]
    fn check_arguments_wrong_type() {
        let tool = tool(
            "t",
            None,
            json!({
                "type": "object",
                "properties": {"a": {"type": "string"}}
            }),
        );
        let err = check_arguments(&tool, &json!({"a": 1})).unwrap_err();
        assert!(err.contains("field a must be string"));
    }

    #[test]
    fn check_arguments_integer_vs_number() {
        let integer_tool = tool(
            "t",
            None,
            json!({
                "type": "object",
                "properties": {"n": {"type": "integer"}}
            }),
        );
        let err = check_arguments(&integer_tool, &json!({"n": 1.5})).unwrap_err();
        assert!(err.contains("field n must be integer"));
        assert!(check_arguments(&integer_tool, &json!({"n": 1})).is_ok());

        let number_tool = tool(
            "t",
            None,
            json!({
                "type": "object",
                "properties": {"n": {"type": "number"}}
            }),
        );
        assert!(check_arguments(&number_tool, &json!({"n": 1})).is_ok());
        assert!(check_arguments(&number_tool, &json!({"n": 1.5})).is_ok());
    }

    #[test]
    fn check_arguments_additional_properties_false() {
        let tool = tool(
            "t",
            None,
            json!({
                "type": "object",
                "properties": {"a": {"type": "string"}},
                "additionalProperties": false
            }),
        );
        let err = check_arguments(&tool, &json!({"a": "x", "b": 1})).unwrap_err();
        assert!(err.contains("field b is not allowed"));
    }

    #[test]
    fn check_arguments_non_object_arguments() {
        let tool = tool("t", None, json!({"type": "object"}));
        let err = check_arguments(&tool, &json!(1)).unwrap_err();
        assert!(err.contains("arguments must be a JSON object"));
    }

    #[test]
    fn check_arguments_valid_arguments_pass() {
        let tool = tool(
            "t",
            None,
            json!({
                "type": "object",
                "properties": {"a": {"type": "string"}},
                "required": ["a"]
            }),
        );
        assert!(check_arguments(&tool, &json!({"a": "x"})).is_ok());
    }

    #[test]
    fn meta_tool_definitions_names_equal_meta_tools() {
        let definitions = meta_tool_definitions();
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition["function"]["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            META_TOOLS.to_vec()
        );
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
