//! What fennec reads from a project's git repository.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

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

/// The number of uncommitted entries in the human's tree, for a quick glance.
pub fn dirty_count(workdir: &Path) -> usize {
    let Ok(output) = Command::new("git")
        .current_dir(workdir)
        .args(["status", "--porcelain", "-z"])
        .output()
    else {
        return 0;
    };
    if !output.status.success() {
        return 0;
    }

    let mut count = 0;
    let mut skip_next = false;
    for field in output.stdout.split(|byte| *byte == 0) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if field.is_empty() {
            continue;
        }
        count += 1;
        if field[0] == b'R' || field[0] == b'C' {
            skip_next = true;
        }
    }
    count
}

/// The sessions whose worktree branch holds commits past their base ref: work
/// still waiting for the human. One `git for-each-ref` per project; sessions
/// whose branch or base is gone (discarded, never opened) are not listed.
pub fn sessions_with_unintegrated_work(workdir: &Path) -> HashSet<String> {
    let Ok(out) = Command::new("git")
        .current_dir(workdir)
        .args([
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/heads/fennec",
            "refs/fennec/base",
        ])
        .output()
    else {
        return HashSet::new();
    };
    unintegrated(&String::from_utf8_lossy(&out.stdout))
}

/// Parses `for-each-ref` lines: a session is pending when its branch and base
/// refs both exist and point to different commits.
fn unintegrated(refs: &str) -> HashSet<String> {
    let mut branch = HashMap::new();
    let mut base = HashMap::new();
    for line in refs.lines() {
        let Some((name, sha)) = line.split_once(' ') else {
            continue;
        };
        if let Some(id) = name.strip_prefix("refs/heads/fennec/") {
            branch.insert(id, sha);
        } else if let Some(id) = name.strip_prefix("refs/fennec/base/") {
            base.insert(id, sha);
        }
    }
    branch
        .into_iter()
        .filter(|(id, sha)| base.get(id).is_some_and(|b| b != sha))
        .map(|(id, _)| id.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{dirty_count, unintegrated};
    use std::process::Command;

    #[test]
    fn pending_only_when_branch_moved_past_base() {
        let refs = "refs/heads/fennec/a 2\nrefs/fennec/base/a 1\n\
                    refs/heads/fennec/b 1\nrefs/fennec/base/b 1\n\
                    refs/heads/fennec/c 3\n";
        let pending = unintegrated(refs);
        assert_eq!(pending.len(), 1);
        assert!(pending.contains("a"));
    }

    #[test]
    fn dirty_count_counts_modified_and_untracked_entries() {
        let dir = std::env::temp_dir().join(format!(
            "fennec-dirty-count-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&dir)
                .args(args)
                .output()
                .expect("git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
            output
        };

        git(&["init"]);
        git(&["config", "user.email", "test@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-m", "a"]);

        assert_eq!(dirty_count(&dir), 0);

        std::fs::write(dir.join("a.txt"), "b\n").unwrap();
        std::fs::write(dir.join("untracked.txt"), "u\n").unwrap();
        assert_eq!(dirty_count(&dir), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
