//! A project's documentation file, read from disk (verified): the markdown the
//! human commits for the project. Its ADRs (`docs/adr/`) are read and rendered
//! separately, so they are not part of a doc file's set.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocFile {
    /// The path as stored in the tree, forward slashes (`docs/api.md`).
    pub path: String,
    /// The file name, for display (`api.md`).
    pub name: String,
    /// The folder the tab groups it under (`docs/guide`), empty at the root.
    pub dir: String,
    pub content: String,
}

/// The folder part of a doc path: `docs/guide/cli.md` → `docs/guide`, `README.md` → empty.
pub fn dir_of(path: &str) -> String {
    path.rsplit_once('/').map(|(dir, _)| dir.to_string()).unwrap_or_default()
}

/// Which paths of a project's tree count as documentation: the root
/// `README.md` and the markdown of `docs/` — its ADRs (`docs/adr/`) are read
/// and rendered on their own. `AGENTS.md` instructs agents, it is not doc.
pub fn is_doc_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let path = normalized.trim_start_matches("./");
    if path == "README.md" {
        return true;
    }
    path.starts_with("docs/") && path.ends_with(".md") && !path.starts_with("docs/adr/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_doc_path_selects_docs_and_skips_adrs() {
        assert!(is_doc_path("README.md"));
        assert!(is_doc_path("docs/api.md"));
        assert!(is_doc_path("docs/sub/notes.md"));

        assert!(!is_doc_path("AGENTS.md"));
        assert!(!is_doc_path("docs/adr/0001-x.md"));
        assert!(!is_doc_path("docs/readme.txt"));
        assert!(!is_doc_path("src/main.rs"));
        assert!(!is_doc_path("README"));
    }

    #[test]
    fn dir_of_keeps_the_folder_only() {
        assert_eq!(dir_of("docs/guide/src/cli.md"), "docs/guide/src");
        assert_eq!(dir_of("README.md"), "");
    }
}
