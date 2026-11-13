//! A small synchronous Model Context Protocol (MCP) client.
//!
//! It speaks JSON-RPC 2.0 over stdio and streamable HTTP without an async
//! runtime. It also provides a lazy tool layer that keeps a model's prompt a
//! constant size: two frozen meta tools, a catalog of servers and tools,
//! schemas fetched on demand, and light argument checks.
//!
//! Features:
//! - core (always on): server configuration, tool access, catalog, schemas,
//!   argument checks.
//! - `client` (default): the synchronous stdio and HTTP client.
//! - `oauth`: optional OAuth support for HTTP servers.

mod lazy;
mod server;

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
mod error;

#[cfg(feature = "oauth")]
pub mod oauth;

pub use crate::lazy::{
    META_TOOLS, catalog, check_arguments, estimate_tokens, meta_tool_definitions, tool_schemas,
};
pub use crate::server::{McpServer, McpTool, McpToolAnnotations, McpTransport, ToolAccess};

#[cfg(feature = "client")]
pub use crate::client::{Client, ClientInfo, direct};
#[cfg(feature = "client")]
pub use crate::error::Error;

/// Compiles the README examples as doctests so they cannot drift from the API.
#[cfg(all(doctest, feature = "client"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
