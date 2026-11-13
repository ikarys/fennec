//! The lazy MCP tool layer: fixed meta tools, a deterministic catalog,
//! on-demand schemas, and light argument checks.

use serde_json::{Value, json};

use crate::server::{McpServer, McpTool};

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
}
