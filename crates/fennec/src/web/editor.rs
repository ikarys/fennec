//! The link that opens a directory in the human's VS Code. Under WSL the
//! browser runs on Windows: a plain `vscode://file/` link would name a Windows
//! path, so the link goes through VS Code's WSL remote instead.

/// `vscode://` link to `path`; `wsl_distro` is `WSL_DISTRO_NAME` when set.
pub(super) fn vscode_link(path: &str, wsl_distro: Option<&str>) -> String {
    let encoded = encode_path(path);
    match wsl_distro.filter(|distro| !distro.is_empty()) {
        Some(distro) => format!("vscode://vscode-remote/wsl+{distro}{encoded}"),
        None => format!("vscode://file{encoded}"),
    }
}

/// Percent-encodes what would break the URL; `/` stays a separator.
fn encode_path(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_go_through_the_wsl_remote_when_under_wsl() {
        assert_eq!(
            vscode_link("/home/t/a b", Some("Ubuntu")),
            "vscode://vscode-remote/wsl+Ubuntu/home/t/a%20b"
        );
        assert_eq!(
            vscode_link("/home/t/x#1", None),
            "vscode://file/home/t/x%231"
        );
        assert_eq!(vscode_link("/p", Some("")), "vscode://file/p");
    }
}
