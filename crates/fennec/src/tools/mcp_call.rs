//! Lazy MCP tools: `mcp_tools` lists the schemas of named tools and `mcp_call` calls one.
//! The catalog notice tells the model which servers are active; a call to a server not active in the session is refused.
//! A live connection is opened per call.

use crate::domain::agent::CommandAccess;
use crate::domain::event::ToolResult;
use crate::domain::mcp::{McpServer, McpTransport, ToolAccess, check_arguments, tool_schemas};
use crate::sandbox::Sandbox;
use serde_json::{Value, json};
use std::path::Path;

/// Everything a lazy MCP call needs from the toolbox.
pub(super) struct McpContext<'a> {
    pub servers: &'a [McpServer],
    pub sandbox: &'a Sandbox,
    pub workdir: &'a Path,
    pub access: CommandAccess,
    pub vault: Option<&'a crate::mcp::oauth::Vault>,
}

/// Runs `mcp_tools` or `mcp_call` (`name` is one of them).
pub(super) fn run(name: &str, args: &Value, ctx: &McpContext<'_>) -> ToolResult {
    let server_name = match super::args::string(args, "server") {
        Ok(server) => server,
        Err(error) => return ToolResult::failure(error),
    };
    let server = match ctx.servers.iter().find(|server| server.name == server_name) {
        Some(server) => server,
        None => return ToolResult::failure(not_active(server_name, ctx.servers)),
    };

    match name {
        "mcp_tools" => run_tools(server, args),
        "mcp_call" => run_call(server, args, ctx),
        _ => ToolResult::failure(format!("Unknown tool: {name}")),
    }
}

/// The name the human's tool rules see for an `mcp_call`: `{server}_{tool}`, as when every MCP tool was a native
/// tool, so existing per-tool rules still apply. None when `name` is not `mcp_call` or an argument is missing.
pub(super) fn rule_name(name: &str, args: &Value) -> Option<String> {
    if name != "mcp_call" {
        return None;
    }
    let server = super::args::string(args, "server").ok()?;
    let tool = super::args::string(args, "tool").ok()?;
    Some(format!("{server}_{tool}"))
}

fn run_tools(server: &McpServer, args: &Value) -> ToolResult {
    let tools = tool_names(args);
    match tool_schemas(server, &tools) {
        Ok(value) => ToolResult::success(value.to_string(), false),
        Err(error) => ToolResult::failure(error),
    }
}

fn run_call(server: &McpServer, args: &Value, ctx: &McpContext<'_>) -> ToolResult {
    let tool_name = match super::args::string(args, "tool") {
        Ok(tool) => tool,
        Err(error) => return ToolResult::failure(error),
    };
    let tool = match server.offered_tools().find(|tool| tool.name == tool_name) {
        Some(tool) => tool,
        None => return ToolResult::failure(no_tool(server, tool_name)),
    };
    let arguments = arguments(args);
    if let Err(error) = check_arguments(tool, &arguments) {
        return ToolResult::failure(error);
    }

    // An HTTP MCP call is a direct request the sandbox never touches, so
    // a read-only mode only lets through the tools the server declares
    // read-only: a remote side-effect is not a read. The declaration is
    // the server's, which the human installed, never the model's.
    let http = server.transport == McpTransport::Http;
    if ctx.access == CommandAccess::ReadOnly
        && http
        && server.tool_access(tool_name) != ToolAccess::Read
    {
        return ToolResult::failure(
            "MCP over HTTP is refused in this read-only mode unless the server declares the tool read-only: it can trigger a remote side-effect. Use a builder session to call it.".to_string(),
        );
    }
    let read_only = ctx.sandbox.read_only(ctx.workdir);
    let sandbox = match ctx.access {
        CommandAccess::ReadWrite => ctx.sandbox,
        // The HTTP transport spawns nothing: the sandbox is not used.
        CommandAccess::ReadOnly if http => ctx.sandbox,
        CommandAccess::ReadOnly => match &read_only {
            Some(sandbox) => sandbox,
            None => {
                return ToolResult::failure(
                    "MCP is read-only in this mode and needs fennec's sandbox, which is turned off on the Settings page.".to_string(),
                )
            }
        },
    };
    call(
        server,
        sandbox,
        ctx.workdir,
        tool_name,
        arguments,
        ctx.vault,
    )
}

/// Open a connection to one MCP server, run the tool, close the connection.
fn call(
    server: &McpServer,
    sandbox: &Sandbox,
    workdir: &Path,
    tool: &str,
    arguments: Value,
    vault: Option<&crate::mcp::oauth::Vault>,
) -> ToolResult {
    let server = match vault {
        Some(vault) => match vault.authorize(server) {
            Ok(server) => server,
            Err(error) => return ToolResult::failure(error),
        },
        None => server.clone(),
    };
    match crate::mcp::Client::connect(&server, sandbox, workdir) {
        Ok(mut client) => match client.call_tool(tool, arguments) {
            Ok(text) => ToolResult::success(text, false),
            Err(error) => ToolResult::failure(error),
        },
        Err(error) => ToolResult::failure(error),
    }
}

fn not_active(server: &str, servers: &[McpServer]) -> String {
    let names = servers
        .iter()
        .map(|server| server.name.clone())
        .collect::<Vec<_>>();
    format!(
        "MCP server {server} is not active in this session. Active servers: {}.",
        join_names(names)
    )
}

fn no_tool(server: &McpServer, tool: &str) -> String {
    let names = server
        .offered_tools()
        .map(|tool| tool.name.clone())
        .collect::<Vec<_>>();
    format!(
        "{} has no tool {tool}. Its tools: {}.",
        server.name,
        join_names(names)
    )
}

fn join_names(names: Vec<String>) -> String {
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}

fn tool_names(args: &Value) -> Vec<String> {
    match args.get("tools") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn arguments(args: &Value) -> Value {
    match args.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(Value::String(text)) => serde_json::from_str::<Value>(text)
            .ok()
            .filter(|value| value.is_object())
            .unwrap_or_else(|| Value::String(text.clone())),
        Some(value) => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{McpContext, ToolResult, rule_name, run};
    use crate::domain::agent::CommandAccess;
    use crate::domain::mcp::{McpServer, McpTool, McpToolAnnotations, McpTransport};
    use crate::sandbox::Sandbox;
    use serde_json::{Value, json};
    use std::path::Path;

    fn error(result: &ToolResult) -> &str {
        result.error.as_deref().unwrap_or_default()
    }

    fn issue_tool(annotations: Option<McpToolAnnotations>) -> McpTool {
        McpTool {
            name: "create_issue".into(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": { "title": { "type": "string" } },
                "required": ["title"]
            }),
            annotations,
        }
    }

    fn api_server() -> McpServer {
        api_server_with_annotations(None)
    }

    fn api_server_with_annotations(annotations: Option<McpToolAnnotations>) -> McpServer {
        McpServer {
            name: "api".into(),
            transport: McpTransport::Http,
            url: Some("http://127.0.0.1:1/mcp".into()),
            timeout: 2,
            cached_tools: vec![issue_tool(annotations)],
            disabled_tools: vec![],
            ..Default::default()
        }
    }

    fn api_server_disabled() -> McpServer {
        McpServer {
            disabled_tools: vec!["create_issue".to_string()],
            ..api_server()
        }
    }

    fn ctx<'a>(
        servers: &'a [McpServer],
        sandbox: &'a Sandbox,
        access: CommandAccess,
    ) -> McpContext<'a> {
        McpContext {
            servers,
            sandbox,
            workdir: Path::new("/wt/1"),
            access,
            vault: None,
        }
    }

    fn valid_args() -> Value {
        json!({
            "server": "api",
            "tool": "create_issue",
            "arguments": { "title": "x" }
        })
    }

    #[test]
    fn refuses_server_not_active() {
        let sandbox = Sandbox::Off;
        let servers: Vec<McpServer> = vec![];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);

        let result = run("mcp_call", &valid_args(), &ctx);

        assert!(error(&result).contains("not active in this session"));
    }

    #[test]
    fn refuses_unknown_tool_and_lists_offered_tools() {
        let sandbox = Sandbox::Off;
        let servers = [api_server()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);
        let args = json!({
            "server": "api",
            "tool": "no_such_tool",
            "arguments": {}
        });

        let result = run("mcp_call", &args, &ctx);

        assert!(error(&result).contains("has no tool no_such_tool"));
        assert!(error(&result).contains("create_issue"));
    }

    #[test]
    fn refuses_disabled_tool_like_unknown_tool() {
        let sandbox = Sandbox::Off;
        let servers = [api_server_disabled()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);

        let result = run("mcp_call", &valid_args(), &ctx);

        assert!(error(&result).contains("has no tool create_issue"));
    }

    #[test]
    fn invalid_arguments_fail_with_schema() {
        let sandbox = Sandbox::Off;
        let servers = [api_server()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);
        let args = json!({
            "server": "api",
            "tool": "create_issue",
            "arguments": {}
        });

        let result = run("mcp_call", &args, &ctx);

        assert!(error(&result).contains("Schema of create_issue"));
    }

    #[test]
    fn double_encoded_arguments_pass_check() {
        let sandbox = Sandbox::Off;
        let servers = [api_server()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);
        let args = json!({
            "server": "api",
            "tool": "create_issue",
            "arguments": "{\"title\":\"x\"}"
        });

        let result = run("mcp_call", &args, &ctx);

        assert!(!error(&result).contains("Schema of create_issue"));
        assert!(!error(&result).contains("arguments"));
    }

    #[test]
    fn http_read_only_refuses_undeclared_tool() {
        let sandbox = Sandbox::Off;
        let servers = [api_server()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadOnly);

        let result = run("mcp_call", &valid_args(), &ctx);

        assert!(error(&result).contains("refused in this read-only mode"));
    }

    #[test]
    fn http_read_only_allows_declared_read_only_tool() {
        let sandbox = Sandbox::Off;
        let servers = [api_server_with_annotations(Some(McpToolAnnotations {
            read_only_hint: Some(true),
            destructive_hint: None,
        }))];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadOnly);

        let result = run("mcp_call", &valid_args(), &ctx);

        assert!(!error(&result).contains("refused in this read-only mode"));
    }

    #[test]
    fn http_read_only_refuses_non_read_only_hint() {
        let sandbox = Sandbox::Off;
        let servers = [api_server_with_annotations(Some(McpToolAnnotations {
            read_only_hint: Some(false),
            destructive_hint: Some(false),
        }))];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadOnly);

        let result = run("mcp_call", &valid_args(), &ctx);

        assert!(error(&result).contains("refused in this read-only mode"));
    }

    #[test]
    fn http_read_write_is_not_mode_refused() {
        let sandbox = Sandbox::Off;
        let servers = [api_server()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);

        let result = run("mcp_call", &valid_args(), &ctx);

        assert!(!error(&result).contains("refused in this read-only mode"));
    }

    #[test]
    fn mcp_tools_returns_schema() {
        let sandbox = Sandbox::Off;
        let servers = [api_server()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);
        let args = json!({
            "server": "api",
            "tools": ["create_issue"]
        });

        let result = run("mcp_tools", &args, &ctx);

        let value: Value = serde_json::from_str(result.output.as_deref().unwrap()).unwrap();
        assert!(value.is_array());
        assert_eq!(value[0]["name"], "create_issue");
    }

    #[test]
    fn mcp_tools_fails_for_unknown_tool() {
        let sandbox = Sandbox::Off;
        let servers = [api_server()];
        let ctx = ctx(&servers, &sandbox, CommandAccess::ReadWrite);
        let args = json!({
            "server": "api",
            "tools": ["nope"]
        });

        let result = run("mcp_tools", &args, &ctx);

        assert!(error(&result).contains("nope"));
    }

    #[test]
    fn rule_name_uses_legacy_native_tool_name() {
        assert_eq!(
            rule_name(
                "mcp_call",
                &json!({ "server": "api", "tool": "create_issue" })
            ),
            Some("api_create_issue".into())
        );
        assert_eq!(
            rule_name(
                "mcp_tools",
                &json!({ "server": "api", "tool": "create_issue" })
            ),
            None
        );
        assert_eq!(rule_name("mcp_call", &json!({ "server": "api" })), None);
    }
}
