//! fennec's side of MCP: connecting a configured server through the sandbox,
//! and caching its tools in the preferences; the protocol lives in `mcp-lite`.

pub mod oauth;

use std::path::Path;

use crate::domain::mcp::McpServer;
use crate::sandbox::Sandbox;
use crate::store::SqliteStore;

/// The client identity sent in the MCP `initialize` request.
pub(crate) const INFO: mcp_lite::ClientInfo = mcp_lite::ClientInfo {
    name: "fennec",
    version: env!("CARGO_PKG_VERSION"),
};

/// Open a configured MCP server through the sandbox.
pub fn connect(
    server: &McpServer,
    sandbox: &Sandbox,
    workdir: &Path,
) -> Result<mcp_lite::Client, String> {
    mcp_lite::Client::connect(server, &INFO, |program, args| {
        sandbox.spawn(program, args, workdir, workdir)
    })
    .map_err(message)
}

/// Render an `mcp_lite` error as fennec's user-facing message.
pub fn message(error: mcp_lite::Error) -> String {
    match error {
        mcp_lite::Error::Unauthorized => {
            "mcp http: 401 unauthorized: authorize this server on the Settings page".to_string()
        }
        other => other.to_string(),
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
    let mut client = connect(&authorized, sandbox, Path::new("."))
        .map_err(|e| (502, format!("connect failed: {e}")))?;
    let tools = client
        .list_tools()
        .map_err(message)
        .map_err(|e| (502, format!("tools/list failed: {e}")))?;
    prefs.mcp[index].cached_tools = tools;
    store
        .save_preferences(prefs)
        .map_err(|e| (500, e.to_string()))?;
    Ok(prefs.mcp[index].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_maps_mcp_errors() {
        assert_eq!(
            message(mcp_lite::Error::Unauthorized),
            "mcp http: 401 unauthorized: authorize this server on the Settings page"
        );
        assert_eq!(message(mcp_lite::Error::Failed("boom".into())), "boom");
    }
}
