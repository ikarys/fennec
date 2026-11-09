//! The link that opens a directory in the human's IDE. Under WSL the browser
//! runs on Windows: a plain `<scheme>://file/` link would name a Windows path,
//! so the link goes through the IDE's WSL remote instead.

use crate::domain::editor::Editor;

/// `editor.scheme()`:// link to `path`; `wsl_distro` is `WSL_DISTRO_NAME` when set.
pub(crate) fn link(editor: Editor, path: &str, wsl_distro: Option<&str>) -> String {
    let encoded = encode_path(path);
    match wsl_distro.filter(|distro| !distro.is_empty()) {
        Some(distro) => format!("{}://vscode-remote/wsl+{distro}{encoded}", editor.scheme()),
        None => format!("{}://file{encoded}", editor.scheme()),
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
            link(Editor::Vscode, "/home/t/a b", Some("Ubuntu")),
            "vscode://vscode-remote/wsl+Ubuntu/home/t/a%20b"
        );
        assert_eq!(
            link(Editor::Vscode, "/home/t/x#1", None),
            "vscode://file/home/t/x%231"
        );
        assert_eq!(link(Editor::Vscode, "/p", Some("")), "vscode://file/p");
        assert_eq!(
            link(Editor::Cursor, "/p", Some("Ubuntu")),
            "cursor://vscode-remote/wsl+Ubuntu/p"
        );
    }
}
