//! The human's integration of a session's branch: preview what the agent
//! committed and cherry-pick it into the human's repository on confirmation.

use std::path::Path;

use serde::Serialize;

use crate::domain::protection::Protection;
use crate::worktree::{Worktree, git};

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
    pub human_branch: Option<String>,
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

    let files = git(top, &["diff", "--name-only", &wt.base, &wt.branch], None)?;
    let files = String::from_utf8_lossy(&files)
        .lines()
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();

    Ok(Preview {
        commits,
        files,
        human_branch: crate::git::current_branch(top),
    })
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

/// Cherry-picks the agent's commits onto the human's branch and moves the
/// base ref to the branch head, so a later integration only takes newer
/// commits.
///
/// No dirty-tree pre-check: git refuses on its own when the human's
/// uncommitted changes would be overwritten, and the common case (the same
/// uncommitted work copied into the worktree at the run's start, see
/// worktree.rs) must not be blocked.
pub fn apply(top: &Path, wt: &Worktree) -> Result<String, String> {
    let range = format!("{}..{}", wt.base, wt.branch);
    let command = format!("git cherry-pick {range}");

    if let Err(message) = git(top, &["cherry-pick", &range], None) {
        // A conflict leaves a cherry-pick in progress; abort it so the
        // human's tree is left as it was.
        if git(
            top,
            &["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"],
            None,
        )
        .is_ok()
        {
            let _ = git(top, &["cherry-pick", "--abort"], None);
        }
        return Err(format!("{message}\nresolve it by hand: {command}"));
    }

    git(top, &["update-ref", &wt.base, &wt.branch], None)?;
    let head = git(top, &["rev-parse", "HEAD"], None)?;
    Ok(String::from_utf8_lossy(&head).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;
    use crate::worktree::{commit, open, top};
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
    fn apply_brings_the_commits_onto_the_human_branch_and_moves_the_base() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("agent.txt"), "x\n").unwrap();
        assert_eq!(commit(&wt, "agent work"), Ok(true));

        let top = top(&repo).unwrap().0;
        let before_head = text(&repo, &["rev-parse", "HEAD"]);
        let head = apply(&top, &wt).unwrap();

        assert_ne!(head, before_head);
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

        assert!(apply(&top, &wt).is_err());

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
