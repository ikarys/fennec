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

/// The remote a human opens in a browser: `origin` when set, else the first
/// remote; `None` when the repository has no remote.
pub fn remote_url(workdir: &Path) -> Option<String> {
    let name = preferred_remote(workdir)?;
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["remote", "get-url", &name])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(String::from)
}

/// `origin` when present, else the first configured remote; `None` when there
/// is none.
fn preferred_remote(workdir: &Path) -> Option<String> {
    let out = Command::new("git")
        .current_dir(workdir)
        .args(["remote"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&out.stdout);
    let remotes: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect();
    remotes
        .iter()
        .find(|r| **r == "origin")
        .or_else(|| remotes.first())
        .copied()
        .map(String::from)
}

/// A git remote as a web URL a browser opens; `None` for a local path.
/// `git@github.com:o/r.git` → `https://github.com/o/r`; `ssh://` and `https://`
/// are kept, a trailing `.git` is dropped.
pub fn web_url(remote: &str) -> Option<String> {
    // The SCP form `git@host:path` splits on `:`; every scheme splits on `/`.
    let rest = remote
        .strip_prefix("ssh://")
        .map(|r| r.strip_prefix("git@").unwrap_or(r))
        .or_else(|| remote.strip_prefix("https://"))
        .or_else(|| remote.strip_prefix("http://"))
        .or_else(|| remote.strip_prefix("git@"))?;
    let sep = if remote.starts_with("git@") { ':' } else { '/' };
    let (host, path) = rest.split_once(sep)?;
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!(
        "https://{}/{}",
        host,
        path.trim_end_matches(".git")
    ))
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

/// The sessions whose work reached the checked-out branch: the `Fennec-Session` trailers an
/// integration writes, read from HEAD's history in one `git log`. Survives the session's deletion.
pub fn integrated_sessions(workdir: &Path) -> HashSet<String> {
    let Ok(out) = Command::new("git")
        .current_dir(workdir)
        .args([
            "log",
            "HEAD",
            "--format=%(trailers:key=Fennec-Session,valueonly)",
        ])
        .output()
    else {
        return HashSet::new();
    };
    session_trailers(&String::from_utf8_lossy(&out.stdout))
}

fn session_trailers(log: &str) -> HashSet<String> {
    log.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchStats {
    pub commits: usize,
    pub files: usize,
}

/// What a session's worktree branch holds past its base ref; None when either ref is gone.
pub fn branch_stats(workdir: &Path, session_id: &str) -> Option<BranchStats> {
    let base = format!("refs/fennec/base/{session_id}");
    let head = format!("refs/heads/fennec/{session_id}");
    let range = format!("{base}..{head}");

    let count_out = Command::new("git")
        .current_dir(workdir)
        .args(["rev-list", "--count"])
        .arg(&range)
        .output()
        .ok()?;
    if !count_out.status.success() {
        return None;
    }
    let commits = String::from_utf8_lossy(&count_out.stdout)
        .trim()
        .parse::<usize>()
        .ok()?;

    let diff_out = Command::new("git")
        .current_dir(workdir)
        .args(["diff", "--name-only"])
        .arg(&base)
        .arg(&head)
        .output()
        .ok()?;
    if !diff_out.status.success() {
        return None;
    }
    let files = String::from_utf8_lossy(&diff_out.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();

    Some(BranchStats { commits, files })
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
    use super::{dirty_count, remote_url, session_trailers, unintegrated, web_url};
    use std::collections::HashSet;
    use std::process::Command;

    #[test]
    fn session_trailers_lists_each_integrated_session() {
        let ids = session_trailers("s1\n\n\n  s2 \n\n");
        assert_eq!(ids, HashSet::from(["s1".to_string(), "s2".to_string()]));
    }

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
    fn web_url_normalizes_remotes_to_web_links() {
        assert_eq!(
            web_url("git@github.com:owner/repo.git"),
            Some("https://github.com/owner/repo".to_string())
        );
        assert_eq!(
            web_url("https://github.com/owner/repo.git"),
            Some("https://github.com/owner/repo".to_string())
        );
        assert_eq!(
            web_url("https://gitlab.com/team/proj"),
            Some("https://gitlab.com/team/proj".to_string())
        );
        assert_eq!(
            web_url("ssh://git@gitea.io/owner/repo.git"),
            Some("https://gitea.io/owner/repo".to_string())
        );
        assert_eq!(
            web_url("git@gitlab.mycompany.io:team/app.git"),
            Some("https://gitlab.mycompany.io/team/app".to_string())
        );
        assert_eq!(web_url("/home/user/repo"), None);
        assert_eq!(web_url("file:///home/user/repo"), None);
    }

    #[test]
    fn remote_url_reads_origin_then_first_remote() {
        let dir = std::env::temp_dir().join(format!(
            "fennec-remote-url-{}-{}",
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
        assert_eq!(remote_url(&dir), None);

        git(&["remote", "add", "upstream", "git@example.com:team/proj.git"]);
        assert_eq!(
            remote_url(&dir),
            Some("git@example.com:team/proj.git".to_string())
        );

        git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/repo.git",
        ]);
        assert_eq!(
            remote_url(&dir),
            Some("https://github.com/owner/repo.git".to_string())
        );

        let _ = std::fs::remove_dir_all(&dir);
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
