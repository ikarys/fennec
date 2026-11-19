//! The diff of what a session changed, read from git for the workflow nodes
//! that judge it (the agent's `Changes` port): the verify and review prompts
//! carry it, so those nodes do not spend calls running `git diff` themselves.

use crate::agent::ports::Changes;
use crate::git;
use std::path::{Path, PathBuf};

/// Diffs `dir`'s working tree against `start`: the worktree's base ref (the
/// commit the agent's work starts from) or `HEAD` in a plain checkout.
pub struct GitChanges {
    pub dir: PathBuf,
    pub start: String,
}

impl Changes for GitChanges {
    fn diff(&self, files: &[String]) -> String {
        match diff(&self.dir, &self.start, files) {
            Ok(diff) => diff,
            // A prompt without its diff is a worse node, not a failed run:
            // the agent reads the files instead.
            Err(e) => {
                eprintln!("fennec: diff of the changed files: {e}");
                String::new()
            }
        }
    }
}

/// The tracked files against `start`, then every file git does not track yet
/// as an addition (`git diff <commit>` alone shows nothing for those). The
/// paths come from the model's tool calls (argument injection): they go after
/// `--`, so none is read as an option.
fn diff(dir: &Path, start: &str, files: &[String]) -> Result<String, String> {
    if files.is_empty() {
        return Ok(String::new());
    }
    let paths: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut args = vec![
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-renames",
        "--unified=3",
        start,
        "--",
    ];
    args.extend(&paths);
    let mut out = String::from_utf8_lossy(&git::run(dir, &args, None)?).into_owned();

    let mut args = vec!["ls-files", "--others", "--"];
    args.extend(&paths);
    let untracked = git::run(dir, &args, None)?;
    for path in String::from_utf8_lossy(&untracked)
        .lines()
        .filter(|line| !line.is_empty())
    {
        // Exit code 1 means "they differ": the answer wanted here.
        let added = git::run_output(
            dir,
            &[
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--no-index",
                "--",
                "/dev/null",
                path,
            ],
        )?;
        out.push_str(&String::from_utf8_lossy(&added.stdout));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;

    /// A temp repository with `a.txt` committed, then changed, and `b.txt` written but untracked.
    fn setup() -> PathBuf {
        let repo = std::env::temp_dir().join(format!("fennec-changes-{}", new_id()));
        std::fs::create_dir_all(&repo).unwrap();
        git::run(&repo, &["init", "-q"], None).unwrap();
        git::run(&repo, &["config", "user.name", "human"], None).unwrap();
        git::run(&repo, &["config", "user.email", "human@example.com"], None).unwrap();
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        git::run(&repo, &["add", "-A"], None).unwrap();
        git::run(&repo, &["commit", "-q", "-m", "init"], None).unwrap();
        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        std::fs::write(repo.join("b.txt"), "new\n").unwrap();
        repo
    }

    #[test]
    fn the_diff_shows_tracked_changes_and_untracked_files_as_additions() {
        let repo = setup();
        let changes = GitChanges {
            dir: repo.clone(),
            start: "HEAD".into(),
        };
        let diff = changes.diff(&["a.txt".into(), "b.txt".into(), "gone.txt".into()]);
        assert!(diff.contains("-one\n+two"), "{diff}");
        assert!(diff.contains("+++ b/b.txt"), "{diff}");
        assert!(diff.contains("+new"), "{diff}");
        assert!(!diff.contains("gone.txt"), "{diff}");
        std::fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn a_path_that_looks_like_an_option_is_a_path() {
        let repo = setup();
        let changes = GitChanges {
            dir: repo.clone(),
            start: "HEAD".into(),
        };
        assert_eq!(changes.diff(&["--output=/tmp/pwned".into()]), "");
        assert!(!Path::new("/tmp/pwned").exists());
        std::fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn no_files_means_no_git_call_and_no_diff() {
        let changes = GitChanges {
            dir: PathBuf::from("/nonexistent"),
            start: "HEAD".into(),
        };
        assert_eq!(changes.diff(&[]), "");
    }
}
