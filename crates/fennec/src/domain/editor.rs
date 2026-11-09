//! The IDE the human opens projects in from the UI.

use serde::{Deserialize, Serialize};

/// The IDE the "open in editor" links target; stored as display.editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Editor {
    #[default]
    Vscode,
    VscodeInsiders,
    Cursor,
    Windsurf,
    Vscodium,
}

impl Editor {
    /// The URL scheme this IDE registers for `://file/` and WSL remote links.
    pub fn scheme(self) -> &'static str {
        match self {
            Editor::Vscode => "vscode",
            Editor::VscodeInsiders => "vscode-insiders",
            Editor::Cursor => "cursor",
            Editor::Windsurf => "windsurf",
            Editor::Vscodium => "vscodium",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Editor;

    #[test]
    fn stored_as_kebab_case() {
        assert_eq!(
            serde_json::to_string(&Editor::VscodeInsiders).unwrap(),
            "\"vscode-insiders\""
        );
        assert_eq!(
            serde_json::from_str::<Editor>("\"cursor\"").unwrap(),
            Editor::Cursor
        );
        assert!(serde_json::from_str::<Editor>("\"zed\"").is_err());
    }

    #[test]
    fn default_is_vscode() {
        assert_eq!(Editor::default(), Editor::Vscode);
    }

    #[test]
    fn scheme_is_the_ide_url_scheme() {
        assert_eq!(Editor::Vscode.scheme(), "vscode");
        assert_eq!(Editor::VscodeInsiders.scheme(), "vscode-insiders");
        assert_eq!(Editor::Cursor.scheme(), "cursor");
        assert_eq!(Editor::Windsurf.scheme(), "windsurf");
        assert_eq!(Editor::Vscodium.scheme(), "vscodium");
    }
}
