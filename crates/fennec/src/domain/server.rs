//! The project's servers, as the human lists them in `.agents/servers.json`.

use serde::{Deserialize, Serialize};

/// One server the human can start and stop from a session's panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerDef {
    pub name: String,
    pub command: String,
}

/// What a server list may be: non-empty names, unique, non-empty commands.
pub fn validate(defs: &[ServerDef]) -> Result<(), String> {
    let mut seen: Vec<String> = Vec::new();
    for def in defs {
        let name = def.name.trim();
        if name.is_empty() {
            return Err("a server name must not be empty".into());
        }
        if def.command.trim().is_empty() {
            return Err(format!("server {name}'s command must not be empty"));
        }
        if seen.iter().any(|seen| seen == name) {
            return Err(format!("duplicate server {name}"));
        }
        seen.push(name.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(name: &str, command: &str) -> ServerDef {
        ServerDef {
            name: name.into(),
            command: command.into(),
        }
    }

    #[test]
    fn a_valid_list_passes() {
        assert!(validate(&[def("web", "npm run dev"), def("api", "cargo run")]).is_ok());
    }

    #[test]
    fn an_empty_name_is_rejected() {
        assert!(validate(&[def("  ", "npm run dev")]).is_err());
    }

    #[test]
    fn an_empty_command_is_rejected() {
        assert!(validate(&[def("web", "   ")]).is_err());
    }

    #[test]
    fn a_duplicate_name_is_rejected() {
        assert!(validate(&[def("web", "a"), def("web", "b")]).is_err());
    }
}
