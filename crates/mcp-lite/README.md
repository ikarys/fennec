# mcp-lite

A small synchronous Model Context Protocol (MCP) client for Rust. It speaks JSON-RPC 2.0 over stdio and streamable HTTP without an async runtime, keeping binaries small. Its lazy tool layer keeps a model's prompt cache stable by exposing two frozen meta tools, a compact catalog, on-demand schemas, and light argument checks.

## Features

| Feature | Default | Dependencies | What it enables |
| --- | --- | --- | --- |
| (core, always on) | yes | `serde`, `serde_json` | Server config, catalog, schemas, argument checks |
| `client` | yes | `ureq` | Synchronous stdio and HTTP client |
| `oauth` | no | `client`, `ring`, `base64` | OAuth support for HTTP servers |

## Examples

Core only:

```rust
use mcp_lite::{catalog, check_arguments, meta_tool_definitions, McpServer, McpTool};
use serde_json::json;

let tool = McpTool {
    name: "echo".into(),
    description: Some("Echoes text".into()),
    input_schema: json!({
        "type": "object",
        "properties": {"text": {"type": "string"}},
        "required": ["text"]
    }),
    annotations: None,
};
let server = McpServer {
    name: "demo".into(),
    cached_tools: vec![tool],
    ..Default::default()
};

let _ = meta_tool_definitions();
let _ = catalog(&[server.clone()]);
check_arguments(&server.cached_tools[0], &json!({"text": "hi"})).unwrap();
```

Client:

```rust,no_run
use mcp_lite::{Client, ClientInfo, McpServer, McpTransport};

let server = McpServer {
    name: "demo".into(),
    transport: McpTransport::Stdio,
    command: Some("demo-mcp".into()),
    ..Default::default()
};
let info = ClientInfo { name: "my-app", version: "0.1.0" };
let mut client = Client::connect(&server, &info, mcp_lite::direct).unwrap();
let tools = client.list_tools().unwrap();
let text = client.call_tool("echo", serde_json::json!({"text": "hi"})).unwrap();
```

## Security

Stdio servers run whatever command is configured, so sandbox them with your own `spawn` function. Tool output is untrusted data, not instructions. With `oauth`, credentials are returned in clear and the caller must store them encrypted.

## License

AGPL-3.0-only
