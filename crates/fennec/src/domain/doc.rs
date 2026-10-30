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
    pub content: String,
}

/// Which paths of a project's tree count as documentation: the root
/// `AGENTS.md` and `README.md`, and the markdown of `docs/` — its ADRs
/// (`docs/adr/`) are read and rendered on their own.
pub fn is_doc_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let path = normalized.trim_start_matches("./");
    if path == "AGENTS.md" || path == "README.md" {
        return true;
    }
    path.starts_with("docs/") && path.ends_with(".md") && !path.starts_with("docs/adr/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_doc_path_selects_docs_and_skips_adrs() {
        assert!(is_doc_path("AGENTS.md"));
        assert!(is_doc_path("README.md"));
        assert!(is_doc_path("docs/api.md"));
        assert!(is_doc_path("docs/sub/notes.md"));

        assert!(!is_doc_path("docs/adr/0001-x.md"));
        assert!(!is_doc_path("docs/readme.txt"));
        assert!(!is_doc_path("src/main.rs"));
        assert!(!is_doc_path("README"));
    }
}
