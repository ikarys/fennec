//! The human's integration of a session's branch: preview what the agent
//! committed and integrate it as one commit into the human's repository on
//! confirmation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::domain::diff::{ChangedLines, changed_lines};
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
    /// The human's uncommitted files the agent built on; their uncommitted
    /// work goes into the integration commit.
    pub carried_changes: Vec<String>,
    /// Uncommitted files of the human's tree the agent's work would
    /// overwrite (content equal to neither the base's nor the branch's):
    /// the integration refuses on them.
    pub local_changes: Vec<String>,
    /// Files the merge onto the human's HEAD would conflict on (empty: it
    /// merges cleanly).
    pub conflicts: Vec<String>,
}

/// Why an integration failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ApplyError {
    /// The human's branch changed the same lines: the merge was refused.
    Conflict { files: Vec<String>, command: String },
    /// Anything else git refused (e.g. uncommitted changes in the way).
    Git { message: String },
}

/// The agent's commits and files since the merge start, plus the human's branch.
pub fn preview(top: &Path, wt: &Worktree) -> Result<Preview, String> {
    let start = merge_start(top, wt);
    let range = format!("{}..{}", start, wt.branch);
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

    let files = touched_files(top, wt, &start)?;
    let numstat = git(
        top,
        &["diff", "--numstat", "--no-renames", &start, &wt.branch],
        None,
    )?;
    let counts = parse_numstat(&String::from_utf8_lossy(&numstat));

    let (carried_changes, local_changes) =
        uncommitted_split(top, start.as_str(), wt.branch.as_str(), &files);

    // The preview reports a conflict before the click; a git failure here is
    // not a conflict, the click will report it.
    let conflicts = conflicts(top, wt);

    Ok(Preview {
        carried_changes,
        local_changes,
        conflicts,
        commits,
        files,
        counts,
        human_branch: crate::git::current_branch(top),
        origin_branch: origin_branch(top, wt),
    })
}

/// The files the merge of the branch onto the human's HEAD conflicts on;
/// empty when it merges cleanly or git fails.
pub fn conflicts(top: &Path, wt: &Worktree) -> Vec<String> {
    let start = merge_start(top, wt);
    let Ok(files) = touched_files(top, wt, &start) else {
        return Vec::new();
    };
    let (carried, _) = uncommitted_split(top, start.as_str(), wt.branch.as_str(), &files);
    let Ok(ours) = ours_with_carried(top, &carried) else {
        return Vec::new();
    };
    match merge_tree(top, &ours, wt.branch.as_str(), &start) {
        Ok((_, conflicted)) => conflicted,
        Err(_) => Vec::new(),
    }
}

/// Brings the branch up to date with the human's HEAD as one commit, leaving
/// conflict markers in the files for the agent to resolve; returns the
/// conflicted files.
pub fn update_branch(top: &Path, wt: &Worktree) -> Result<Vec<String>, String> {
    let ours = git(top, &["rev-parse", "HEAD"], None)
        .map(|out| String::from_utf8_lossy(&out).trim().to_string())?;
    let (tree, conflicted) = merge_tree(top, &ours, wt.branch.as_str(), &merge_start(top, wt))?;
    let message = format!("fennec: bring {} up to date with HEAD", wt.branch);
    let commit = git(
        top,
        &["commit-tree", &tree, "-p", &ours, "-m", &message],
        None,
    )
    .map(|out| String::from_utf8_lossy(&out).trim().to_string())?;
    git(&wt.path, &["reset", "-q", "--hard", &commit], None)?;
    git(top, &["update-ref", &wt.base, &ours], None)?;
    Ok(conflicted)
}

/// The agent's touched files: `git diff --name-only --no-renames start branch`.
fn touched_files(top: &Path, wt: &Worktree, start: &str) -> Result<Vec<String>, String> {
    let out = git(
        top,
        &["diff", "--name-only", "--no-renames", start, &wt.branch],
        None,
    )?;
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect())
}

/// Splits the human's uncommitted files among `files` (the agent's touched
/// files) into the two kinds the integration treats differently:
///
/// - `carried`: an uncommitted file whose working-tree content equals its
///   content at `base` or at `branch`. The agent built on that work, so it
///   goes into the integration commit.
/// - `local_changes`: an uncommitted file whose content equals neither. The
///   agent's work would overwrite the human's; the integration refuses on it.
///
/// Files the agent did not touch are not in `files`, so they are never split:
/// the human's unrelated uncommitted work is left alone.
fn uncommitted_split(
    top: &Path,
    base: &str,
    branch: &str,
    files: &[String],
) -> (Vec<String>, Vec<String>) {
    let modified = git_names(top, &["diff", "HEAD", "--name-only"]);
    let untracked = git_names(top, &["ls-files", "--others", "--exclude-standard"]);
    let mut carried = Vec::new();
    let mut local = Vec::new();
    for file in files {
        let uncommitted = modified.contains(file.as_str()) || untracked.contains(file.as_str());
        if !uncommitted {
            continue;
        }
        if content_equals(top, base, file) || content_equals(top, branch, file) {
            carried.push(file.clone());
        } else {
            local.push(file.clone());
        }
    }
    (carried, local)
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
///
/// Both sides are hashed: the working tree's file (`git hash-object`) against
/// the ref's (`git rev-parse <ref>:<file>`). A file missing from the working
/// tree equals the ref only when the ref has no file there either. Hashes,
/// not `git diff <ref> -- <file>`, which does not see untracked files.
fn content_equals(top: &Path, ref_name: &str, file: &str) -> bool {
    let working = git(top, &["hash-object", "--", file], None);
    let at_ref = git(
        top,
        &["rev-parse", "-q", "--verify", &format!("{ref_name}:{file}")],
        None,
    );
    match (working, at_ref) {
        (Ok(w), Ok(r)) => String::from_utf8_lossy(&w).trim() == String::from_utf8_lossy(&r).trim(),
        // Neither side has the file: they agree.
        (Err(_), Err(_)) => true,
        // One side has it, the other does not.
        _ => false,
    }
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

/// The lines the agent's branch adds and removes, per file under the project
/// (`files` relative to the repository top, the result relative to the
/// project: the `prefix` is stripped, files outside it are skipped). A file
/// git cannot diff (binary) changes no line.
pub fn changed_files(
    top: &Path,
    wt: &Worktree,
    files: &[String],
    prefix: &str,
) -> Vec<(String, ChangedLines)> {
    let start = merge_start(top, wt);
    files
        .iter()
        .filter_map(|file| Some((file, file.strip_prefix(prefix)?)))
        .map(|(file, relative)| {
            let diff = git(
                top,
                &["diff", "--unified=0", &start, &wt.branch, "--", file],
                None,
            )
            .map(|out| String::from_utf8_lossy(&out).into_owned())
            .unwrap_or_default();
            (relative.to_string(), changed_lines(&diff))
        })
        .collect()
}

/// Git's record of a cherry-pick under way; it exists while one is.
fn sequencer_dir(top: &Path) -> PathBuf {
    let out = git(top, &["rev-parse", "--git-path", "sequencer"], None).unwrap_or_default();
    top.join(String::from_utf8_lossy(&out).trim())
}

/// Like `worktree::git`, but with `GIT_INDEX_FILE` set to `index`, so the
/// command works on that index instead of the human's (which the integration
/// must not touch). `worktree::git` cannot set environment variables.
fn git_with_index(dir: &Path, index: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let command = format!("git -C {} {}", dir.display(), args.join(" "));
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_INDEX_FILE", index)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("{command}: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(format!("{command}: {}", stderr.trim()))
    }
}

/// A temporary commit of `HEAD` plus the carried files' working content,
/// built without touching the human's index or working tree: a private index
/// file (removed before and after) is used through `GIT_INDEX_FILE`, so
/// `read-tree`/`update-index`/`write-tree` never see the human's index. The
/// commit's tree is what matters; the index file is dropped once the commit
/// is in the object store.
fn temp_commit_with_carried(top: &Path, carried: &[String]) -> Result<String, String> {
    let index_out = git(
        top,
        &["rev-parse", "--git-path", "fennec-integration.index"],
        None,
    )?;
    let index = top.join(String::from_utf8_lossy(&index_out).trim());
    // Remove a leftover from an earlier failed run.
    let _ = std::fs::remove_file(&index);

    // HEAD's tree, with the carried files taken from the working tree.
    git_with_index(top, &index, &["read-tree", "HEAD"])?;
    let mut update = vec![
        "update-index".to_string(),
        "--add".to_string(),
        "--remove".to_string(),
        "--".to_string(),
    ];
    update.extend(carried.iter().cloned());
    let update_refs: Vec<&str> = update.iter().map(String::as_str).collect();
    git_with_index(top, &index, &update_refs)?;
    let tree_out = git_with_index(top, &index, &["write-tree"])?;
    let tree = String::from_utf8_lossy(&tree_out).trim().to_string();
    let commit_out = git_with_index(
        top,
        &index,
        &[
            "commit-tree",
            &tree,
            "-p",
            "HEAD",
            "-m",
            "fennec: the human's uncommitted work the agent built on",
        ],
    )?;
    let _ = std::fs::remove_file(&index);
    Ok(String::from_utf8_lossy(&commit_out).trim().to_string())
}

/// `ours` for the merge: `HEAD` when nothing is carried, else a temporary
/// commit of `HEAD` plus the carried files' working content.
fn ours_with_carried(top: &Path, carried: &[String]) -> Result<String, String> {
    if carried.is_empty() {
        Ok("HEAD".to_string())
    } else {
        temp_commit_with_carried(top, carried)
    }
}

/// Where the merge starts: the base ref, or the branch's fork point with the
/// human's HEAD when that is newer. A branch merged by hand leaves the base ref
/// behind, and its already merged lines would conflict; after an integration,
/// which squashes, the fork point is the older of the two and the base ref is
/// right.
pub(crate) fn merge_start(top: &Path, wt: &Worktree) -> String {
    let fork_point = git(top, &["merge-base", "HEAD", &wt.branch], None)
        .map(|out| String::from_utf8_lossy(&out).trim().to_string());
    match fork_point {
        Ok(fork_point)
            if git(
                top,
                &["merge-base", "--is-ancestor", &wt.base, &fork_point],
                None,
            )
            .is_ok() =>
        {
            fork_point
        }
        _ => wt.base.clone(),
    }
}

/// `git merge-tree --write-tree --name-only --no-messages --merge-base=<base>
/// <ours> <branch>`: the merged tree and the conflicted files, computed off
/// the trees so a dirty working tree does not make git refuse (needs git ≥
/// 2.40). `Err` is a git failure.
fn merge_tree(
    top: &Path,
    ours: &str,
    branch: &str,
    base: &str,
) -> Result<(String, Vec<String>), String> {
    let merge_base = format!("--merge-base={base}");
    let command = format!(
        "git -C {} merge-tree --write-tree --name-only --no-messages {merge_base} {ours} {branch}",
        top.display()
    );
    let out = Command::new("git")
        .arg("-C")
        .arg(top)
        .arg("merge-tree")
        .arg("--write-tree")
        .arg("--name-only")
        .arg("--no-messages")
        .arg(&merge_base)
        .arg(ours)
        .arg(branch)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("{command}: {e}"))?;

    match out.status.code() {
        // No conflicts: the first line is the merged tree.
        Some(0) => {
            let tree = String::from_utf8_lossy(&out.stdout)
                .lines()
                .find(|line| !line.is_empty())
                .map(str::to_string)
                .ok_or_else(|| format!("{command}: no tree in the output"))?;
            Ok((tree, Vec::new()))
        }
        // Conflicts: the first line is a tree, the following non-empty lines
        // are the conflicted paths.
        Some(1) => {
            let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|line| !line.is_empty())
                .map(String::from)
                .collect();
            let tree = lines
                .first()
                .cloned()
                .ok_or_else(|| format!("{command}: no tree in the output"))?;
            Ok((tree, lines.into_iter().skip(1).collect()))
        }
        _ => Err(format!(
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// Runs `git <args> -- <files…>` in `top` (the files after `--` so a path
/// that looks like an option is not misread).
fn git_files(top: &Path, args: &[&str], files: &[String]) -> Result<Vec<u8>, String> {
    let mut all: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    all.push("--".to_string());
    all.extend_from_slice(files);
    let refs: Vec<&str> = all.iter().map(String::as_str).collect();
    git(top, &refs, None)
}

/// Commits the agent's work as one commit on the human's branch, then moves
/// the base ref to the branch head, so a later integration only takes newer
/// commits.
///
/// Staged changes in the human's repository are refused: a commit made from
/// the index would sweep the human's staged work in too.
///
/// The result is computed with `git merge-tree` off the trees, so a dirty
/// working tree no longer makes git refuse the way a cherry-pick would. The
/// human's uncommitted work the agent built on (`carried`) rides along into
/// the commit; only an uncommitted edit the agent's work would overwrite
/// (content equal to neither the base's nor the branch's) is a blocker.
pub fn apply(top: &Path, wt: &Worktree, message: &str) -> Result<String, ApplyError> {
    // A commit made from the index would also take whatever the human staged.
    if git(top, &["diff", "--cached", "--quiet"], None).is_err() {
        return Err(ApplyError::Git {
            message: "you have staged changes: commit or unstage them first, they would go into the integration commit"
                .into(),
        });
    }

    // A pick left by something else (the human's, or an older run) would mix
    // with ours: say whose it is.
    if sequencer_dir(top).exists() {
        return Err(ApplyError::Git {
            message: "a cherry-pick is already in progress in your repository: finish it (git cherry-pick --continue) or drop it (git cherry-pick --quit) first"
                .into(),
        });
    }

    let start = merge_start(top, wt);
    let files = touched_files(top, wt, &start).map_err(|message| ApplyError::Git { message })?;
    let (carried, local) = uncommitted_split(top, start.as_str(), wt.branch.as_str(), &files);

    // An uncommitted edit the agent's work would overwrite: refuse, naming it.
    if !local.is_empty() {
        return Err(ApplyError::Git {
            message: format!(
                "uncommitted changes the agent's work would overwrite: {}; commit or stash them first",
                local.join(", ")
            ),
        });
    }

    let range = format!("{}..{}", start, wt.branch);
    let command = format!("git cherry-pick --no-commit {range}");

    // The branch is already merged when the range is empty; refuse before
    // making a commit.
    let count = git(top, &["rev-list", "--count", &range], None)
        .map(|out| String::from_utf8_lossy(&out).trim().to_string())
        .map_err(|message| ApplyError::Git { message })?;
    if count == "0" {
        return Err(ApplyError::Git {
            message: "nothing to integrate: the branch is already merged".into(),
        });
    }

    // `ours`: HEAD, or a temporary commit of HEAD plus the carried files'
    // working content, so the merge carries the human's uncommitted work the
    // agent built on.
    let ours = ours_with_carried(top, &carried).map_err(|message| ApplyError::Git { message })?;

    // The merge, computed off the trees so a dirty working tree does not make
    // git refuse; a conflict names the files.
    let (tree, conflicted) = match merge_tree(top, &ours, wt.branch.as_str(), &start) {
        Ok((tree, conflicted)) => (tree, conflicted),
        Err(message) => {
            return Err(ApplyError::Git {
                message: format!("{message}\nresolve it by hand: {command}"),
            });
        }
    };
    if !conflicted.is_empty() {
        return Err(ApplyError::Conflict {
            files: conflicted,
            command,
        });
    }

    // The integration commit, on the human's HEAD.
    let commit = git(
        top,
        &["commit-tree", &tree, "-p", "HEAD", "-F", "-"],
        Some(message.as_bytes()),
    )
    .map_err(|message| ApplyError::Git {
        message: format!("{message}\nthe agent's changes were not committed"),
    })?;
    let commit = String::from_utf8_lossy(&commit).trim().to_string();

    // Bring the human's branch and tree to the commit. First put each carried
    // file back to HEAD's content (safe: that content is in `ours`), clearing
    // the local modification so `reset --keep` can update it; then move the
    // branch and update the working tree, keeping the human's other uncommitted
    // work.
    for file in &carried {
        if git(
            top,
            &["rev-parse", "-q", "--verify", &format!("HEAD:{file}")],
            None,
        )
        .is_ok()
        {
            let _ = git(top, &["checkout", "HEAD", "--", file], None);
        } else {
            let _ = std::fs::remove_file(top.join(file));
        }
    }
    if let Err(message) = git(top, &["reset", "-q", "--keep", &commit], None) {
        // Restore the carried files from `ours` and unstage them, so the
        // human's tree is back to what it was.
        if !carried.is_empty() {
            let _ = git_files(top, &["checkout", &ours], &carried);
            let _ = git_files(top, &["reset", "-q"], &carried);
        }
        return Err(ApplyError::Git { message });
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
        assert!(preview.conflicts.is_empty());
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
        assert!(preview.carried_changes.is_empty());
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
        assert_eq!(preview.carried_changes, vec!["a.txt".to_string()]);
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
        assert!(preview.carried_changes.is_empty());
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
    fn preview_reports_a_conflict_before_the_click() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        std::fs::write(repo.join("a.txt"), "human\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "human change"], None).unwrap();

        let top = top(&repo).unwrap().0;
        assert_eq!(
            preview(&top, &wt).unwrap().conflicts,
            vec!["a.txt".to_string()]
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn conflicts_and_update_branch_leave_markers_for_the_agent() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        std::fs::write(repo.join("a.txt"), "human\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "human change"], None).unwrap();

        let top = top(&repo).unwrap().0;
        let human_head = text(&repo, &["rev-parse", "HEAD"]);
        assert_eq!(conflicts(&top, &wt), vec!["a.txt".to_string()]);

        let conflicted = update_branch(&top, &wt).unwrap();
        assert_eq!(conflicted, vec!["a.txt".to_string()]);
        assert!(
            std::fs::read_to_string(wt.workdir.join("a.txt"))
                .unwrap()
                .contains("<<<<<<<")
        );
        assert_eq!(text(&repo, &["rev-parse", &wt.base]), human_head);
        assert_eq!(
            text(
                &repo,
                &[
                    "rev-list",
                    "--count",
                    &format!("{}..{}", wt.base, wt.branch)
                ]
            ),
            "1"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn work_after_an_integration_starts_from_the_moved_base() {
        // The squash leaves the fork point (`merge-base HEAD branch`) at the run's start:
        // the gate must start from the base the integration moved, not from there.
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("agent.txt"), "x\n").unwrap();
        assert_eq!(commit(&wt, "agent work"), Ok(true));
        let top = top(&repo).unwrap().0;
        apply(&top, &wt, "integrate\n\nFennec-Session: s1").unwrap();

        std::fs::write(wt.workdir.join("later.txt"), "y\n").unwrap();
        assert_eq!(commit(&wt, "later work"), Ok(true));
        let next = preview(&top, &wt).unwrap();

        assert_eq!(
            next.commits
                .iter()
                .map(|c| c.subject.as_str())
                .collect::<Vec<_>>(),
            vec!["later work"]
        );
        assert_eq!(next.files, vec!["later.txt".to_string()]);
        assert_eq!(merge_start(&top, &wt), wt.base);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preview_after_a_merge_by_hand_lists_only_the_newer_work() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        git(&repo, &["merge", "--no-ff", "--no-edit", "fennec/s1"], None).unwrap();
        std::fs::write(wt.workdir.join("b.txt"), "b\n").unwrap();
        assert_eq!(commit(&wt, "agent work after the merge"), Ok(true));

        let top = top(&repo).unwrap().0;
        let preview = preview(&top, &wt).unwrap();

        assert_eq!(
            preview.commits,
            vec![Commit {
                sha: text(&repo, &["rev-parse", "fennec/s1"]),
                subject: "agent work after the merge".into(),
            }]
        );
        assert_eq!(preview.files, vec!["b.txt".to_string()]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_branch_fully_merged_by_hand_has_nothing_to_integrate() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        git(&repo, &["merge", "--no-ff", "--no-edit", "fennec/s1"], None).unwrap();

        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);
        let preview = preview(&top, &wt).unwrap();
        assert!(preview.commits.is_empty());
        assert!(preview.files.is_empty());

        let Err(ApplyError::Git { message }) = apply(&top, &wt, "integrate") else {
            panic!("expected nothing to integrate");
        };
        assert!(message.contains("nothing to integrate"));
        assert_eq!(text(&repo, &["rev-parse", "HEAD"]), before_head);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_branch_merged_by_hand_has_no_false_conflict() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        git(&repo, &["merge", "--no-ff", "--no-edit", "fennec/s1"], None).unwrap();
        std::fs::write(repo.join("a.txt"), "human\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "human change"], None).unwrap();

        std::fs::write(wt.workdir.join("b.txt"), "b\n").unwrap();
        assert_eq!(commit(&wt, "agent work after the merge"), Ok(true));

        let top = top(&repo).unwrap().0;
        assert!(conflicts(&top, &wt).is_empty());

        apply(&top, &wt, "integrate").unwrap();
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "human\n"
        );
        assert_eq!(std::fs::read_to_string(repo.join("b.txt")).unwrap(), "b\n");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn apply_carries_the_uncommitted_work_the_agent_built_on() {
        let (base, repo) = setup();
        // The human's uncommitted edit of a.txt, before the run started.
        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        // The agent built on that work and committed it.
        std::fs::write(wt.workdir.join("a.txt"), "two\nagent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);
        let head = apply(&top, &wt, "integrate").unwrap();

        assert_ne!(head, before_head);
        assert_eq!(
            text(
                &repo,
                &["rev-list", "--count", &format!("{before_head}..HEAD")]
            ),
            "1"
        );
        // The human's uncommitted work is now committed, with the agent's
        // change on top, and the tree is clean.
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "two\nagent\n"
        );
        assert_eq!(text(&repo, &["show", "HEAD:a.txt"]), "two\nagent");
        assert_eq!(text(&repo, &["status", "--porcelain"]), "");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn apply_carries_an_untracked_file_the_agent_edited() {
        let (base, repo) = setup();
        // The human's untracked file, before the run started.
        std::fs::write(repo.join("new.txt"), "n\n").unwrap();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        // The agent edited it and committed it.
        std::fs::write(wt.workdir.join("new.txt"), "n\nagent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        let top = top(&repo).unwrap().0;
        let head = apply(&top, &wt, "integrate").unwrap();

        assert_eq!(text(&repo, &["rev-list", "--count", "HEAD"]), "2");
        assert_eq!(head, text(&repo, &["rev-parse", "HEAD"]));
        assert_eq!(
            std::fs::read_to_string(repo.join("new.txt")).unwrap(),
            "n\nagent\n"
        );
        assert_eq!(text(&repo, &["show", "HEAD:new.txt"]), "n\nagent");
        assert_eq!(text(&repo, &["status", "--porcelain"]), "");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn apply_refuses_an_uncommitted_edit_made_after_the_run_started() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "agent\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        // The human's own edit, made after the run started: content equal to
        // neither the base's nor the branch's.
        std::fs::write(repo.join("a.txt"), "human\n").unwrap();

        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);
        let Err(ApplyError::Git { message }) = apply(&top, &wt, "integrate") else {
            panic!("expected git to refuse");
        };
        assert!(message.contains("a.txt"));

        assert_eq!(text(&repo, &["rev-parse", "HEAD"]), before_head);
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap(),
            "human\n"
        );
        assert!(git(&repo, &["diff", "--cached", "--quiet"], None).is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn apply_keeps_unrelated_uncommitted_work() {
        let (base, repo) = setup();
        // c.txt is tracked and committed, so the human's later edit is a real
        // uncommitted change the agent does not touch.
        std::fs::write(repo.join("c.txt"), "c\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "c"], None).unwrap();

        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("b.txt"), "b\n").unwrap();
        assert_eq!(commit(&wt, "agent change"), Ok(true));

        // The human's uncommitted edit of c.txt, after the run started.
        std::fs::write(repo.join("c.txt"), "c\nhuman\n").unwrap();

        let top = top(&repo).unwrap().0;
        let head = apply(&top, &wt, "integrate").unwrap();

        assert_eq!(text(&repo, &["rev-list", "--count", "HEAD"]), "3");
        assert_eq!(head, text(&repo, &["rev-parse", "HEAD"]));
        // c.txt keeps the human's edit, still uncommitted; b.txt is committed.
        assert_eq!(
            std::fs::read_to_string(repo.join("c.txt")).unwrap(),
            "c\nhuman\n"
        );
        assert_eq!(text(&repo, &["show", "HEAD:b.txt"]), "b");
        assert_eq!(text(&repo, &["status", "--porcelain"]), "M c.txt");
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
