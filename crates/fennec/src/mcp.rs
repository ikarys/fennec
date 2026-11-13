//! The synchronous MCP client: JSON-RPC 2.0 over stdio (a child process in the
//! sandbox world) and streamable HTTP (ureq). Only `initialize`, `tools/list`
//! and `tools/call`. No async runtime: fennec is std::thread, so a hanging
//! server is bounded by a per-call timeout that kills the child.

pub mod oauth;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::domain::mcp::{McpServer, McpTool, McpTransport};
use crate::sandbox::Sandbox;
use crate::store::SqliteStore;

const PROTOCOL_VERSION: &str = "2025-06-18";

/// A connected MCP server, ready for `tools/list` and `tools/call`.
pub struct Client {
    transport: Transport,
    timeout: Duration,
    /// The `Mcp-Session-Id` a streamable-HTTP server assigned at `initialize`.
    session_id: Option<String>,
}

enum Transport {
    Stdio {
        child: Box<Child>,
        stdin: Box<ChildStdin>,
        rx: mpsc::Receiver<String>,
    },
    Http {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl Client {
    /// Open the transport and complete the `initialize` handshake.
    pub fn connect(
        server: &McpServer,
        sandbox: &Sandbox,
        workdir: &Path,
    ) -> Result<Client, String> {
        let timeout = Duration::from_secs(if server.timeout == 0 {
            60
        } else {
            server.timeout
        });
        let transport = match server.transport {
            McpTransport::Stdio => {
                let program = server
                    .command
                    .as_deref()
                    .filter(|command| !command.trim().is_empty())
                    .ok_or_else(|| format!("mcp {}: no command", server.name))?;
                let mut cmd = sandbox.spawn(program, &server.args, workdir, workdir);
                cmd.stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null());
                for (key, value) in &server.env {
                    cmd.env(key, value);
                }
                if let Some(token) = server
                    .auth_token
                    .as_deref()
                    .filter(|token| !token.trim().is_empty())
                {
                    cmd.env("MCP_AUTH_TOKEN", token);
                }
                let mut child = cmd
                    .spawn()
                    .map_err(|error| format!("mcp {}: spawn failed: {error}", server.name))?;
                let stdin = child.stdin.take().ok_or("mcp: no stdin")?;
                let stdout = child.stdout.take().ok_or("mcp: no stdout")?;
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    for line in BufReader::new(stdout).lines() {
                        match line {
                            Ok(line) => {
                                if tx.send(line).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
                Transport::Stdio {
                    child: Box::new(child),
                    stdin: Box::new(stdin),
                    rx,
                }
            }
            McpTransport::Http => Transport::Http {
                url: server.url.clone().unwrap_or_default(),
                headers: http_headers(server),
            },
        };
        let mut client = Client {
            transport,
            timeout,
            session_id: None,
        };
        client.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "fennec", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        client.send_initialized()?;
        Ok(client)
    }

    /// `tools/list`: the server's tools, in its own order.
    pub fn list_tools(&mut self) -> Result<Vec<McpTool>, String> {
        let result = self.request("tools/list", json!({}))?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        tools.iter().map(value_to_tool).collect()
    }

    /// `tools/call`: run one tool and return its text result.
    pub fn call_tool(&mut self, name: &str, args: Value) -> Result<String, String> {
        let result = self.request("tools/call", json!({ "name": name, "arguments": args }))?;
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            return Err(tool_text(&result).unwrap_or_else(|| "mcp tool error".into()));
        }
        Ok(tool_text(&result).unwrap_or_default())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        match &mut self.transport {
            Transport::Stdio { child, stdin, rx } => {
                stdio_request(child, stdin, rx, method, params, self.timeout)
            }
            Transport::Http { url, headers } => {
                let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
                let body =
                    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
                let reply = http_post(url, headers, &mut self.session_id, &body, self.timeout)?;
                reply_response(&reply)
            }
        }
    }

    /// The MCP `notifications/initialized` notification after `initialize`.
    fn send_initialized(&mut self) -> Result<(), String> {
        match &mut self.transport {
            Transport::Stdio { stdin, .. } => {
                let message = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
                writeln!(stdin, "{message}")
                    .and_then(|()| stdin.flush())
                    .map_err(|error| format!("mcp write failed: {error}"))
            }
            Transport::Http { url, headers } => {
                let body = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
                http_post(url, headers, &mut self.session_id, &body, self.timeout).map(|_| ())
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Transport::Stdio { child, .. } = &mut self.transport {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The shared "Connect" logic: open the server, list tools, cache them in the
/// preferences, and run the collision checks.
pub fn connect_and_cache(
    store: &SqliteStore,
    prefs: &mut crate::domain::preferences::Preferences,
    index: usize,
    sandbox: &Sandbox,
    vault: &oauth::Vault,
) -> Result<McpServer, (u16, String)> {
    let server = prefs.mcp[index].clone();
    if !server.problems().is_empty() {
        return Err((400, server.problems().join("; ")));
    }
    // An OAuth-authorized server gets its (refreshed) bearer here.
    let authorized = vault.authorize(&server).map_err(|e| (401, e))?;
    let mut client = Client::connect(&authorized, sandbox, std::path::Path::new("."))
        .map_err(|e| (502, format!("connect failed: {e}")))?;
    let tools = client
        .list_tools()
        .map_err(|e| (502, format!("tools/list failed: {e}")))?;
    prefs.mcp[index].cached_tools = tools;
    store
        .save_preferences(prefs)
        .map_err(|e| (500, e.to_string()))?;
    Ok(prefs.mcp[index].clone())
}

fn stdio_request(
    child: &mut Child,
    stdin: &mut ChildStdin,
    rx: &mpsc::Receiver<String>,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    writeln!(stdin, "{message}")
        .and_then(|()| stdin.flush())
        .map_err(|error| format!("mcp write failed: {error}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return kill(child, "mcp: timed out");
        }
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                if let Some(outcome) = parse_response(&line) {
                    return outcome;
                }
                // A notification or log line: keep reading for the response.
            }
            Err(_) => return kill(child, "mcp: timed out"),
        }
    }
}

fn kill(child: &mut Child, message: &str) -> Result<Value, String> {
    let _ = child.kill();
    let _ = child.wait();
    Err(message.to_string())
}

/// A JSON-RPC response line: `None` for notifications, else the `result` or
/// the `error`.
fn parse_response(line: &str) -> Option<Result<Value, String>> {
    let value: Value = serde_json::from_str(line).ok()?;
    value.get("id")?;
    if let Some(error) = value.get("error") {
        return Some(Err(format!("mcp error: {error}")));
    }
    Some(Ok(value.get("result").cloned().unwrap_or(Value::Null)))
}

/// The HTTP headers to send: the server's headers, plus the auth token as a
/// Bearer `Authorization` header when set (a manual `authorization` header,
/// in any case, is overridden).
fn http_headers(server: &McpServer) -> BTreeMap<String, String> {
    let mut headers = server.headers.clone();
    if let Some(token) = server
        .auth_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
    {
        headers.retain(|key, _| !key.eq_ignore_ascii_case("authorization"));
        headers.insert("Authorization".into(), format!("Bearer {token}"));
    }
    headers
}

/// A server's HTTP reply: its status, content type and body.
struct HttpReply {
    status: u16,
    content_type: String,
    text: String,
}

/// POST one JSON-RPC message. The session id the server assigns (at
/// `initialize`) is kept and sent back on every later message, with the
/// negotiated protocol version, as streamable HTTP requires.
fn http_post(
    url: &str,
    headers: &BTreeMap<String, String>,
    session_id: &mut Option<String>,
    body: &Value,
    timeout: Duration,
) -> Result<HttpReply, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(session) = session_id.as_deref() {
        request = request
            .header("Mcp-Session-Id", session)
            .header("MCP-Protocol-Version", PROTOCOL_VERSION);
    }
    for (key, value) in headers {
        request = request.header(key, value);
    }
    let response = request
        .send_json(body)
        .map_err(|error| format!("mcp http: {error}"))?;
    let status = response.status().as_u16();
    if status == 401 {
        return Err(
            "mcp http: 401 unauthorized: authorize this server on the Settings page".into(),
        );
    }
    if let Some(session) = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
    {
        *session_id = Some(session.to_string());
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|error| format!("mcp http body: {error}"))?;
    if !(200..300).contains(&status) && !text.contains("\"jsonrpc\"") {
        return Err(format!("mcp http: status {status}"));
    }
    Ok(HttpReply {
        status,
        content_type,
        text,
    })
}

/// The JSON-RPC response in a reply: a JSON body, or the last `data:` line of
/// an event stream.
fn reply_response(reply: &HttpReply) -> Result<Value, String> {
    let parsed = if reply.content_type.contains("text/event-stream") {
        reply
            .text
            .lines()
            .rev()
            .filter_map(|line| line.trim().strip_prefix("data:"))
            .find_map(|data| parse_response(data.trim()))
    } else {
        parse_response(&reply.text)
    };
    parsed.unwrap_or_else(|| Err(format!("mcp http: no response (status {})", reply.status)))
}

fn value_to_tool(value: &Value) -> Result<McpTool, String> {
    Ok(McpTool {
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        input_schema: value
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object" })),
        annotations: value
            .get("annotations")
            .and_then(|a| serde_json::from_value(a.clone()).ok()),
    })
}

/// The text of a `tools/call` result, from its `content` parts.
fn tool_text(result: &Value) -> Option<String> {
    let content = result.get("content")?.as_array()?;
    Some(
        content
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal MCP server in a shell script: answers initialize, reads the
    /// `initialized` notification, then answers tools/list and tools/call in
    /// order, matching the ids the client sends.
    const STDIO_SCRIPT: &str = r#"
read line
echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"t","version":"0"}}}'
read line
read line
echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"echoes","inputSchema":{"type":"object","properties":{"msg":{"type":"string"}}}}]}}'
read line
echo '{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"hello mcp"}]}}'
"#;

    fn stdio_server() -> McpServer {
        McpServer {
            name: "t".into(),
            transport: McpTransport::Stdio,
            command: Some("sh".into()),
            args: vec!["-c".into(), STDIO_SCRIPT.into()],
            ..Default::default()
        }
    }

    #[test]
    fn a_stdio_client_lists_and_calls_tools() {
        let mut client = Client::connect(&stdio_server(), &Sandbox::Off, Path::new(".")).unwrap();
        let tools = client.list_tools().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[0].input_schema["type"], "object");
        let text = client.call_tool("echo", json!({ "msg": "hi" })).unwrap();
        assert_eq!(text, "hello mcp");
    }

    #[test]
    fn http_headers_include_the_bearer_token() {
        let server = McpServer {
            transport: McpTransport::Http,
            auth_token: Some("secret".into()),
            ..Default::default()
        };
        let headers = http_headers(&server);
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer secret")
        );
    }

    #[test]
    fn a_token_overrides_a_manual_authorization_header_in_any_case() {
        let mut server = McpServer {
            transport: McpTransport::Http,
            auth_token: Some("secret".into()),
            ..Default::default()
        };
        server
            .headers
            .insert("authorization".into(), "Bearer manual".into());
        let headers = http_headers(&server);
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer secret")
        );
        assert_eq!(
            headers
                .iter()
                .filter(|(key, _)| key.eq_ignore_ascii_case("authorization"))
                .count(),
            1
        );
    }

    #[test]
    fn an_empty_token_sends_no_authorization_header() {
        let mut server = McpServer {
            transport: McpTransport::Http,
            auth_token: Some("".into()),
            ..Default::default()
        };
        server
            .headers
            .insert("authorization".into(), "Bearer manual".into());
        let headers = http_headers(&server);
        assert_eq!(headers.get("Authorization"), None);
        assert_eq!(
            headers.get("authorization").map(String::as_str),
            Some("Bearer manual")
        );
    }

    #[test]
    fn http_headers_omit_authorization_without_a_token() {
        let server = McpServer {
            transport: McpTransport::Http,
            ..Default::default()
        };
        assert!(!http_headers(&server).contains_key("Authorization"));
    }

    #[test]
    fn a_stdio_server_receives_the_auth_token_env_var() {
        // A stdio server that returns its `MCP_AUTH_TOKEN` env var as the tool
        // result: the client must have passed the configured token to it.
        let script = r#"
read line
echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"t","version":"0"}}}'
read line
read line
echo "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"$MCP_AUTH_TOKEN\"}]}}"
"#;
        let server = McpServer {
            name: "t".into(),
            transport: McpTransport::Stdio,
            command: Some("sh".into()),
            args: vec!["-c".into(), script.into()],
            auth_token: Some("secret".into()),
            ..Default::default()
        };
        let mut client = Client::connect(&server, &Sandbox::Off, Path::new(".")).unwrap();
        let text = client.call_tool("env", json!({})).unwrap();
        assert_eq!(text, "secret");
    }

    #[test]
    fn a_hanging_stdio_server_times_out() {
        let server = McpServer {
            name: "hang".into(),
            transport: McpTransport::Stdio,
            command: Some("sh".into()),
            args: vec!["-c".into(), "sleep 5".into()],
            timeout: 1,
            ..Default::default()
        };
        // `sleep` never answers initialize, so connect must time out, not hang.
        let start = Instant::now();
        let result = Client::connect(&server, &Sandbox::Off, Path::new("."));
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn tool_text_reads_the_content_parts() {
        let result = json!({ "content": [ { "type": "text", "text": "a" }, { "type": "text", "text": "b" } ] });
        assert_eq!(tool_text(&result).as_deref(), Some("ab"));
        assert_eq!(tool_text(&json!({})), None);
    }
}
