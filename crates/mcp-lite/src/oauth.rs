//! OAuth 2.1 authorization for HTTP MCP servers.
//!
//! The crate never stores or encrypts anything: credentials come back in clear
//! and the caller must store them encrypted.

mod flow;
mod protocol;

pub use flow::{Authorization, Pending, begin, finish, refresh};
pub use protocol::McpCredentials;
