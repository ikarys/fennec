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

/// Removes ANSI CSI escape sequences and lone ESC characters.
pub fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        // A CSI sequence runs to its final byte (0x40..=0x7E); a lone ESC is dropped.
        if chars.clone().next() == Some('[') {
            chars.next();
            for c in chars.by_ref() {
                if ('\x40'..='\x7e').contains(&c) {
                    break;
                }
            }
        }
    }
    out
}

/// Finds the URL announced in the last matching line of server output.
pub fn announced_url(lines: &[String]) -> Option<String> {
    const PREFIXES: [&str; 5] = [
        "http://localhost:",
        "http://127.0.0.1:",
        "http://0.0.0.0:",
        "http://[::1]:",
        "http://[::]:",
    ];

    for line in lines.iter().rev() {
        let stripped = strip_ansi(line);

        let mut best: Option<(usize, &str)> = None;
        for prefix in PREFIXES {
            if let Some(pos) = stripped.find(prefix)
                && best.is_none_or(|(best_pos, _)| pos < best_pos)
            {
                best = Some((pos, prefix));
            }
        }

        let Some((pos, prefix)) = best else {
            continue;
        };

        let rest = &stripped[pos + prefix.len()..];
        let port_end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if port_end == 0 {
            continue;
        }

        let Ok(port) = rest[..port_end].parse::<u16>() else {
            continue;
        };

        let path = rest[port_end..].split_whitespace().next().unwrap_or("");
        let path = path.trim_end_matches(['.', ',', ';', ')']);

        return Some(format!("http://127.0.0.1:{port}{path}"));
    }

    None
}

/// Extracts the port from a localhost/loopback URL.
pub fn url_port(url: &str) -> Option<u16> {
    let after_scheme = url.split_once("://")?.1;
    let host_port = after_scheme.split(['/', '?', '#']).next().unwrap_or("");

    if let Some(inner) = host_port.strip_prefix('[') {
        let end = inner.find(']')?;
        let port = inner[end + 1..].strip_prefix(':')?;
        return port.parse().ok();
    }

    let port = host_port.rsplit_once(':')?.1;
    port.parse().ok()
}

/// Project files used to guess a default dev command.
pub struct ProjectFiles<'a> {
    pub justfile: Option<&'a str>,
    pub package_json: Option<&'a str>,
    pub makefile: Option<&'a str>,
    pub cargo_toml: bool,
}

/// Guesses a default dev command from common project files.
pub fn guess_command(files: &ProjectFiles) -> Option<String> {
    if let Some(justfile) = files.justfile
        && just_has_dev_recipe(justfile)
    {
        return Some("just dev".into());
    }

    if let Some(package_json) = files.package_json
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(package_json)
        && value
            .get("scripts")
            .and_then(|scripts| scripts.get("dev"))
            .and_then(serde_json::Value::as_str)
            .is_some()
    {
        return Some("npm run dev".into());
    }

    if let Some(makefile) = files.makefile {
        if make_has_target(makefile, "dev") {
            return Some("make dev".into());
        }
        if make_has_target(makefile, "run") {
            return Some("make run".into());
        }
    }

    if files.cargo_toml {
        return Some("cargo run".into());
    }

    None
}

fn just_has_dev_recipe(justfile: &str) -> bool {
    for line in justfile.lines() {
        if line.starts_with([' ', '\t']) {
            continue;
        }

        let name_part = line.strip_prefix('@').unwrap_or(line);
        let Some(colon) = name_part.find(':') else {
            continue;
        };

        let before = name_part[..colon].trim_end();
        let after = &name_part[colon + 1..];
        if after.starts_with('=') {
            continue;
        }

        if before == "dev" {
            return true;
        }

        let mut parts = before.split_whitespace();
        if parts.next() == Some("dev") && parts.next().is_some() {
            return true;
        }
    }

    false
}

fn make_has_target(makefile: &str, target: &str) -> bool {
    for line in makefile.lines() {
        if line.starts_with([' ', '\t']) {
            continue;
        }

        let Some(rest) = line.strip_prefix(target) else {
            continue;
        };

        if rest.starts_with(':') && !rest.starts_with(":=") {
            return true;
        }
    }

    false
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

    #[test]
    fn strip_ansi_removes_ansi_and_lone_esc() {
        let line = "  \u{2794}  Local:   \x1b[1mhttp://localhost:5173/\x1b[22m";
        assert_eq!(
            strip_ansi(line),
            "  \u{2794}  Local:   http://localhost:5173/"
        );

        assert_eq!(strip_ansi("a\x1bb"), "ab");
    }

    #[test]
    fn announced_url_reads_vite_line_with_ansi_port() {
        let lines = vec!["  \u{2794}  Local:   http://localhost:\x1b[1m5173\x1b[22m/".to_string()];
        assert_eq!(
            announced_url(&lines),
            Some("http://127.0.0.1:5173/".to_string())
        );
    }

    #[test]
    fn announced_url_normalizes_0_0_0_0() {
        let lines = vec!["Listening on http://0.0.0.0:3000".to_string()];
        assert_eq!(
            announced_url(&lines),
            Some("http://127.0.0.1:3000".to_string())
        );
    }

    #[test]
    fn announced_url_last_match_wins() {
        let lines = vec![
            "http://localhost:1/".to_string(),
            "http://127.0.0.1:2/".to_string(),
        ];
        assert_eq!(
            announced_url(&lines),
            Some("http://127.0.0.1:2/".to_string())
        );
    }

    #[test]
    fn announced_url_none_when_no_url() {
        let lines = vec!["hello".to_string()];
        assert_eq!(announced_url(&lines), None);
    }

    #[test]
    fn url_port_reads_ports() {
        assert_eq!(url_port("http://127.0.0.1:5173/"), Some(5173));
        assert_eq!(url_port("http://[::1]:8080"), Some(8080));
        assert_eq!(url_port("http://localhost"), None);
    }

    #[test]
    fn guess_command_prefers_justfile_dev() {
        let files = ProjectFiles {
            justfile: Some("dev:\n  echo hi\n"),
            package_json: Some(r#"{"scripts":{"dev":"next dev"}}"#),
            makefile: Some("dev:\n\techo hi\n"),
            cargo_toml: true,
        };
        assert_eq!(guess_command(&files), Some("just dev".to_string()));
    }

    #[test]
    fn guess_command_uses_package_json_dev() {
        let files = ProjectFiles {
            justfile: None,
            package_json: Some(r#"{"scripts":{"dev":"next dev"}}"#),
            makefile: Some("dev:\n\techo hi\n"),
            cargo_toml: true,
        };
        assert_eq!(guess_command(&files), Some("npm run dev".to_string()));
    }

    #[test]
    fn guess_command_uses_makefile_dev_or_run() {
        let dev = ProjectFiles {
            justfile: None,
            package_json: None,
            makefile: Some("dev:\n\techo hi\n"),
            cargo_toml: true,
        };
        assert_eq!(guess_command(&dev), Some("make dev".to_string()));

        let run = ProjectFiles {
            justfile: None,
            package_json: None,
            makefile: Some("run:\n\techo hi\n"),
            cargo_toml: true,
        };
        assert_eq!(guess_command(&run), Some("make run".to_string()));
    }

    #[test]
    fn guess_command_uses_cargo_toml() {
        let files = ProjectFiles {
            justfile: None,
            package_json: None,
            makefile: None,
            cargo_toml: true,
        };
        assert_eq!(guess_command(&files), Some("cargo run".to_string()));
    }

    #[test]
    fn guess_command_ignores_justfile_dev_assignment() {
        let files = ProjectFiles {
            justfile: Some("dev:=x\n"),
            package_json: None,
            makefile: None,
            cargo_toml: true,
        };
        assert_eq!(guess_command(&files), Some("cargo run".to_string()));
    }

    #[test]
    fn guess_command_none_when_no_sources() {
        let files = ProjectFiles {
            justfile: None,
            package_json: None,
            makefile: None,
            cargo_toml: false,
        };
        assert_eq!(guess_command(&files), None);
    }
}
