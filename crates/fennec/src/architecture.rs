//! The project's architecture rules on disk, and the check of an integration's
//! added lines against them.

use std::path::Path;

use crate::domain::architecture::{Rules, Violation, added_lines};
use crate::worktree::{Worktree, git};

/// The project's rules: `.agents/architecture` when present, none otherwise.
pub fn load(workdir: &Path) -> Rules {
    let content = std::fs::read_to_string(workdir.join(".agents").join("architecture"));
    Rules::parse(&content.unwrap_or_default())
}

/// The rule violations among the lines the agent's branch adds. `files` are
/// relative to the repository top; rules are relative to the project, so the
/// `prefix` is stripped and files outside the project are skipped. A file git
/// cannot diff (deleted, binary) adds no line.
pub fn violations(
    top: &Path,
    wt: &Worktree,
    files: &[String],
    prefix: &str,
    rules: &Rules,
) -> Vec<Violation> {
    if rules.is_empty() {
        return Vec::new();
    }
    files
        .iter()
        .filter_map(|file| Some((file, file.strip_prefix(prefix)?)))
        .flat_map(|(file, relative)| {
            let diff = git(
                top,
                &["diff", "--unified=0", &wt.base, &wt.branch, "--", file],
                None,
            )
            .map(|out| String::from_utf8_lossy(&out).into_owned())
            .unwrap_or_default();
            rules.check(relative, &added_lines(&diff))
        })
        .collect()
}
