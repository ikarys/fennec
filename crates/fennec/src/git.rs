//! What fennec reads from a project's git repository.

use std::path::Path;

/// The checked-out branch, read from `.git/HEAD` (worktrees: the `gitdir:`
/// file); `None` when detached or not a repository.
pub fn current_branch(workdir: &Path) -> Option<String> {
    let dot_git = workdir.join(".git");
    let git_dir = if dot_git.is_file() {
        let text = std::fs::read_to_string(&dot_git).ok()?;
        workdir.join(text.strip_prefix("gitdir:")?.trim())
    } else {
        dot_git
    };
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(String::from)
}
