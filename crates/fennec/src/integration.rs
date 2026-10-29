//! The human's integration of a session's branch: preview what the agent
//! committed and integrate it as one commit into the human's repository on
//! confirmation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::domain::protection::Protection;
use crate::worktree::{Worktree, git, origin_branch};

/// One agent commit to integrate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
}

/// What the integration would do, before the human confirms it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub commits: Vec<Commit>,
    /// Paths relative to the repository top.
    pub files: Vec<String>,
    /// Lines added and removed per file of `files` (binary files count 0).
    pub counts: HashMap<String, (u64, u64)>,
    pub human_branch: Option<String>,
    /// The branch the run started from; `None` for older worktrees or a
    /// detached HEAD.
    pub origin_branch: Option<String>,
    /// Uncommitted files of the human's tree the cherry-pick would
    /// overwrite; git would refuse the pick on them.
    pub local_changes: Vec<String>,
}

/// Why an integration failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ApplyError {
    /// The human's branch changed the same lines: the cherry-pick was aborted.
    Conflict { files: Vec<String>, command: String },
    /// Anything else git refused (e.g. uncommitted changes in the way).
    Git { message: String },
}

/// The agent's commits and files since the base ref, plus the human's branch.
pub fn preview(top: &Path, wt: &Worktree) -> Result<Preview, String> {
    let range = format!("{}..{}", wt.base, wt.branch);
    let log = git(
        top,
        &["log", "--reverse", "--format=%H%x09%s", &range],
        None,
    )?;
    let commits = String::from_utf8_lossy(&log)
        .lines()
        .filter_map(|line| {
            line.split_once('\t').map(|(sha, subject)| Commit {
                sha: sha.to_string(),
                subject: subject.to_string(),
            })
        })
        .collect();

    let files = git(
        top,
        &["diff", "--name-only", "--no-renames", &wt.base, &wt.branch],
        None,
    )?;
    let files: Vec<String> = String::from_utf8_lossy(&files)
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();

    let numstat = git(
        top,
        &["diff", "--numstat", "--no-renames", &wt.base, &wt.branch],
        None,
    )?;
    let counts = parse_numstat(&String::from_utf8_lossy(&numstat));

    Ok(Preview {
        local_changes: local_changes(top, wt.base.as_str(), wt.branch.as_str(), &files),
        commits,
        files,
        counts,
        human_branch: crate::git::current_branch(top),
        origin_branch: origin_branch(top, wt),
    })
}

/// The human's uncommitted files among `files` that the cherry-pick would
/// overwrite: edits, staged or not (git refuses both), and untracked files.
///
/// An edit only counts when the tree's content differs from both
/// the base's and the branch's: git overwrites a local edit only when the
/// pick writes something else. That keeps the common case open, where the
/// worktree started from the human's same dirty work and committed it.
fn local_changes(top: &Path, base: &str, branch: &str, files: &[String]) -> Vec<String> {
    let modified = git_names(top, &["diff", "HEAD", "--name-only"]);
    let untracked = git_names(top, &["ls-files", "--others", "--exclude-standard"]);
    files
        .iter()
        .filter(|file| {
            if modified.contains(file.as_str()) {
                !tree_equals(top, base, file) && !tree_equals(top, branch, file)
            } else {
                untracked.contains(file.as_str())
            }
        })
        .cloned()
        .collect()
}

/// The output lines of a git command, one set entry per non-empty line.
fn git_names(top: &Path, args: &[&str]) -> std::collections::HashSet<String> {
    git(top, args, None)
        .map(|out| String::from(String::from_utf8_lossy(&out)))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect()
}

/// True when the working tree's `file` equals its content at `ref`.
fn tree_equals(top: &Path, ref_name: &str, file: &str) -> bool {
    git(top, &["diff", "--quiet", ref_name, "--", file], None).is_ok()
}

/// `git diff --numstat` lines (`added<TAB>removed<TAB>path`); binary files
/// show `-` and count 0.
fn parse_numstat(out: &str) -> HashMap<String, (u64, u64)> {
    out.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let (added, removed, path) = (parts.next()?, parts.next()?, parts.next()?);
            Some((
                path.to_string(),
                (added.parse().unwrap_or(0), removed.parse().unwrap_or(0)),
            ))
        })
        .collect()
}

/// The protected files among `files` (relative to the repository top).
///
/// The protection patterns are relative to the project, so a file under the
/// project's prefix is checked with the prefix stripped; a file outside the
/// project's prefix is checked as is.
pub fn protected_touched(files: &[String], prefix: &str, protection: &Protection) -> Vec<String> {
    files
        .iter()
        .filter(|file| {
            let relative = file.strip_prefix(prefix).unwrap_or(file.as_str());
            protection.is_protected(relative)
        })
        .cloned()
        .collect()
}

/// Git's record of a cherry-pick under way; it exists while one is.
fn sequencer_dir(top: &Path) -> PathBuf {
    let out = git(top, &["rev-parse", "--git-path", "sequencer"], None).unwrap_or_default();
    top.join(String::from_utf8_lossy(&out).trim())
}

/// Puts the human's repository back as it was before the pick: the index was
/// clean (checked first), so what is staged is the pick's; `reset --merge`
/// keeps the human's unstaged edits. A single commit leaves no sequencer,
/// hence the fallback.
fn undo_pick(top: &Path) {
    if git(top, &["cherry-pick", "--abort"], None).is_err() {
        let _ = git(top, &["reset", "-q", "--merge"], None);
        let _ = git(top, &["cherry-pick", "--quit"], None);
    }
}

/// Stages the agent's changes and commits them as one commit on the human's
/// branch, then moves the base ref to the branch head, so a later
/// integration only takes newer commits.
///
/// Staged changes in the human's repository are refused: `cherry-pick
/// --no-commit` puts the agent's changes in the index, and the following
/// commit would sweep the human's staged work in too.
///
/// No dirty-tree pre-check for unstaged changes: git refuses on its own when
/// the human's uncommitted changes would be overwritten, and the common case
/// (the same uncommitted work copied into the worktree at the run's start,
/// see worktree.rs) must not be blocked.
pub fn apply(top: &Path, wt: &Worktree, message: &str) -> Result<String, ApplyError> {
    // `cherry-pick --no-commit` stages the agent's changes; a following
    // commit would also take whatever the human staged.
    if git(top, &["diff", "--cached", "--quiet"], None).is_err() {
        return Err(ApplyError::Git {
            message: "you have staged changes: commit or unstage them first, they would go into the integration commit"
                .into(),
        });
    }

    // A pick left by something else (the human's, or an older run) makes git
    // refuse ours with a message that blames fennec: say whose it is.
    if sequencer_dir(top).exists() {
        return Err(ApplyError::Git {
            message: "a cherry-pick is already in progress in your repository: finish it (git cherry-pick --continue) or drop it (git cherry-pick --quit) first"
                .into(),
        });
    }

    let range = format!("{}..{}", wt.base, wt.branch);
    let command = format!("git cherry-pick --no-commit {range}");

    if let Err(message) = git(top, &["cherry-pick", "--no-commit", &range], None) {
        // With --no-commit a single commit leaves no CHERRY_PICK_HEAD: the
        // unmerged files are what tells a conflict.
        let files: Vec<String> = git(top, &["diff", "--name-only", "--diff-filter=U"], None)
            .map(|out| {
                String::from_utf8_lossy(&out)
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        if !files.is_empty() {
            undo_pick(top);
            return Err(ApplyError::Conflict { files, command });
        }
        // Git refused (e.g. an unstaged edit it would overwrite); a range may
        // have staged earlier commits, so undo the pick.
        undo_pick(top);
        return Err(ApplyError::Git {
            message: format!("{message}\nresolve it by hand: {command}"),
        });
    }

    if let Err(e) = git(top, &["commit", "-q", "-m", message], None) {
        return Err(ApplyError::Git {
            message: format!(
                "{e}\nthe agent's changes are staged in your repository: commit them by hand"
            ),
        });
    }

    git(top, &["update-ref", &wt.base, &wt.branch], None)
        .map_err(|message| ApplyError::Git { message })?;
    let head =
        git(top, &["rev-parse", "HEAD"], None).map_err(|message| ApplyError::Git { message })?;
    Ok(String::from_utf8_lossy(&head).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_numstat;

    #[test]
    fn numstat_counts_lines_and_zeroes_binary_files() {
        let counts = parse_numstat("3\t1\ta.rs\n-\t-\timg.png\n");
        assert_eq!(counts["a.rs"], (3, 1));
        assert_eq!(counts["img.png"], (0, 0));
    }

    use super::*;
    use crate::domain::id::new_id;
    use crate::worktree::{commit, open, remove, top};
    use std::path::PathBuf;

    fn text(dir: &Path, args: &[&str]) -> String {
        String::from_utf8_lossy(&git(dir, args, None).unwrap())
            .trim()
            .to_string()
    }

    /// A temp directory with a repository inside (`repo/`), one commit.
    fn setup() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("fennec-integration-{}", new_id()));
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"], None).unwrap();
        git(&repo, &["config", "user.name", "human"], None).unwrap();
        git(&repo, &["config", "user.email", "human@example.com"], None).unwrap();
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "init"], None).unwrap();
        (base, repo)
    }

    #[test]
    fn preview_lists_the_agents_commits_only() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("agent.txt"), "x\n").unwrap();
        assert_eq!(commit(&wt, "agent work"), Ok(true));
        std::fs::write(wt.workdir.join("agent2.txt"), "y\n").unwrap();
        assert_eq!(commit(&wt, "agent work 2"), Ok(true));

        let top = top(&repo).unwrap().0;
        let preview = preview(&top, &wt).unwrap();

        assert_eq!(
            preview.commits,
            vec![
                Commit {
                    sha: text(&repo, &["rev-parse", "fennec/s1~1"]),
                    subject: "agent work".into(),
                },
                Commit {
                    sha: text(&repo, &["rev-parse", "fennec/s1"]),
                    subject: "agent work 2".into(),
                },
            ]
        );
        assert_eq!(
            preview.files,
            vec!["agent.txt".to_string(), "agent2.txt".to_string()]
        );
        assert!(preview.human_branch.is_some());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_records_the_origin_branch_and_remove_deletes_it() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        let top = top(&repo).unwrap().0;
        let human_branch = text(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
        let preview = preview(&top, &wt).unwrap();

        assert_eq!(
            preview.origin_branch.as_deref(),
            Some(human_branch.as_str())
        );

        assert_eq!(remove(&top, &wt), Ok(()));
        assert!(!text(&repo, &["for-each-ref", "refs/fennec/"]).contains("origin"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_counts_a_renamed_file_under_its_new_path() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::rename(wt.workdir.join("a.txt"), wt.workdir.join("c.txt")).unwrap();
        assert_eq!(commit(&wt, "rename"), Ok(true));

        let top = top(&repo).unwrap().0;
        let p = preview(&top, &wt).unwrap();

        assert!(p.files.contains(&"a.txt".to_string()));
        assert!(p.files.contains(&"c.txt".to_string()));
        assert_eq!(p.counts["c.txt"], (1, 0));
        assert_eq!(p.counts["a.txt"], (0, 1));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_flags_uncommitted_files_the_pick_would_overwrite() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        std::fs::write(repo.join("a.txt"), "human\n").unwrap();
        let top = top(&repo).unwrap().0;
        let preview = preview(&top, &wt).unwrap();
        assert_eq!(preview.local_changes, vec!["a.txt".to_string()]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_does_not_flag_the_dirty_work_the_worktree_copied() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "two\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        let top = top(&repo).unwrap().0;
        let preview = preview(&top, &wt).unwrap();
        assert!(preview.local_changes.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_ignores_uncommitted_files_the_pick_does_not_touch() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("b.txt"), "b\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        std::fs::write(repo.join("c.txt"), "c\n").unwrap();
        std::fs::write(repo.join("a.txt"), "human\n").unwrap();
        let top = top(&repo).unwrap().0;
        let preview = preview(&top, &wt).unwrap();
        assert!(preview.local_changes.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_flags_an_untracked_file_the_pick_would_create() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("new.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        std::fs::write(repo.join("new.txt"), "human\n").unwrap();
        let top = top(&repo).unwrap().0;
        let preview = preview(&top, &wt).unwrap();
        assert_eq!(preview.local_changes, vec!["new.txt".to_string()]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn apply_brings_the_commits_onto_the_human_branch_and_moves_the_base() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("agent.txt"), "x\n").unwrap();
        assert_eq!(commit(&wt, "agent work"), Ok(true));

        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);
        let message = "integrate the agent's work\n\nFennec-Session: s1";
        let head = apply(&top, &wt, message).unwrap();

        assert_ne!(head, before_head);
        assert_eq!(
            text(
                &repo,
                &["rev-list", "--count", &format!("{before_head}..HEAD")]
            ),
            "1"
        );
        let log = text(&repo, &["log", "-1", "--format=%B"]);
        assert!(log.contains("integrate the agent's work"));
        assert!(log.contains("Fennec-Session: s1"));
        assert_eq!(
            std::fs::read_to_string(repo.join("agent.txt")).unwrap(),
            "x\n"
        );
        assert_eq!(
            text(&repo, &["rev-parse", &wt.base]),
            text(&repo, &["rev-parse", &wt.branch])
        );

        let second = preview(&top, &wt).unwrap();
        assert!(second.commits.is_empty());
        assert!(second.files.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_conflicting_apply_leaves_the_human_branch_and_tree_unchanged() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        std::fs::write(repo.join("a.txt"), "human\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "human change"], None).unwrap();

        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);
        let before_status = text(&repo, &["status", "--porcelain"]);

        let Err(ApplyError::Conflict { files, .. }) = apply(&top, &wt, "integrate") else {
            panic!("expected a conflict");
        };
        assert_eq!(files, vec!["a.txt".to_string()]);

        assert_eq!(text(&repo, &["rev-parse", "HEAD"]), before_head);
        assert_eq!(text(&repo, &["status", "--porcelain"]), before_status);
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "human\n"
        );
        assert!(
            git(
                &repo,
                &["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"],
                None
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_refused_apply_leaves_no_cherry_pick_in_progress() {
        let (base, repo) = setup();
        // The human's unstaged edit of a file the agent also changes: git
        // refuses to overwrite it, which is not a conflict.
        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        let top = top(&repo).unwrap().0;
        let Err(ApplyError::Git { .. }) = apply(&top, &wt, "integrate") else {
            panic!("expected git to refuse");
        };

        assert!(!sequencer_dir(&top).exists());
        assert!(matches!(
            apply(&top, &wt, "integrate"),
            Err(ApplyError::Git { message }) if !message.contains("already in progress")
        ));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_range_refused_midway_leaves_the_human_tree_unchanged() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("b.txt"), "b\n").unwrap();
        assert_eq!(commit(&wt, "first"), Ok(true));
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "second"), Ok(true));

        std::fs::write(repo.join("a.txt"), "human\n").unwrap();

        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);
        let Err(ApplyError::Git { .. }) = apply(&top, &wt, "integrate") else {
            panic!("expected git to refuse");
        };

        assert_eq!(text(&repo, &["rev-parse", "HEAD"]), before_head);
        assert!(!repo.join("b.txt").exists());
        assert!(git(&repo, &["diff", "--cached", "--quiet"], None).is_ok());
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "human\n"
        );
        assert!(!sequencer_dir(&top).exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_staged_change_in_the_human_repository_blocks_the_integration() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();

        std::fs::write(repo.join("staged.txt"), "x\n").unwrap();
        git(&repo, &["add", "staged.txt"], None).unwrap();
        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);

        let Err(ApplyError::Git { message }) = apply(&top, &wt, "integrate") else {
            panic!("expected a staged-changes error");
        };
        assert!(message.contains("staged changes"));
        assert_eq!(text(&repo, &["rev-parse", "HEAD"]), before_head);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn protected_touched_strips_the_project_prefix() {
        let protection = Protection::new(Some("secrets/\n"));
        let files = vec![
            "sub/secrets/key".to_string(),
            "sub/other".to_string(),
            "secrets/key".to_string(),
        ];

        assert_eq!(
            protected_touched(&files, "sub/", &protection),
            vec!["sub/secrets/key".to_string(), "secrets/key".to_string()]
        );
    }

    #[test]
    fn protected_touched_with_an_empty_prefix_checks_as_is() {
        let protection = Protection::new(Some("secrets/\n"));
        let files = vec!["secrets/key".to_string(), "src/main.rs".to_string()];

        assert_eq!(
            protected_touched(&files, "", &protection),
            vec!["secrets/key".to_string()]
        );
    }
}
