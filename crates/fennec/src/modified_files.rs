//! What a session's worktree changed, counted by git: the thread's modified files agree
//! with the integration gate.

use std::collections::BTreeMap;
use std::path::Path;

use crate::domain::modified_files::{self, ModifiedFiles};
use crate::git;
use crate::worktree::Worktree;

/// The most untracked files read for their line counts; the rest count as files with 0 lines.
const MAX_UNTRACKED_READ: usize = 200;
/// The largest untracked file read for its line count; larger files count 0 lines.
const MAX_COUNTED_BYTES: u64 = 256 * 1024;

/// What the agent changed in its worktree, counted by git so the panel agrees with the
/// integration gate.
pub fn in_worktree(top: &Path, wt: &Worktree) -> Result<ModifiedFiles, String> {
    let start = crate::integration::merge_start(top, wt);
    let mut files: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    let tracked = git::run(
        &wt.workdir,
        &["diff", "--numstat", "--no-renames", "--relative", &start],
        None,
    )?;
    for (path, (added, removed)) in
        crate::integration::parse_numstat(&String::from_utf8_lossy(&tracked))
    {
        files.insert(path, (added as usize, removed as usize));
    }

    let untracked = git::run(
        &wt.workdir,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        None,
    )?;
    // ponytail: only the first `MAX_UNTRACKED_READ` untracked files are read; the others count
    // as files with 0 lines (an untracked dependency folder must not cost a read per file at
    // each thread refresh).
    let paths = untracked
        .split(|&b| b == b'\0')
        .filter(|path| !path.is_empty());
    for (index, path_bytes) in paths.enumerate() {
        let path = String::from_utf8_lossy(path_bytes).into_owned();
        let lines = if index < MAX_UNTRACKED_READ {
            untracked_lines(&wt.workdir.join(&path))
        } else {
            0
        };
        files.insert(path, (lines, 0));
    }

    Ok(modified_files::summarize(files))
}

/// The lines of an untracked file, 0 for a non-regular file, a file over `MAX_COUNTED_BYTES`,
/// a read failure, or a binary file (git numstat counts binary as 0).
fn untracked_lines(path: &Path) -> usize {
    // A symlink (to /dev/zero, say) or a huge file is not read: no DoS through a refresh.
    let regular_and_small = std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= MAX_COUNTED_BYTES);
    let Some(bytes) = regular_and_small
        .then(|| std::fs::read(path).ok())
        .flatten()
    else {
        return 0;
    };
    if bytes.contains(&0) {
        return 0;
    }
    let mut lines = bytes.iter().filter(|&&b| b == b'\n').count();
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        lines += 1;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::in_worktree;
    use crate::domain::id::new_id;
    use crate::domain::modified_files::ModifiedFile;
    use crate::git::run as git;
    use crate::worktree::{commit, open, top};
    use std::path::PathBuf;

    /// A temp directory with a repository inside (`repo/`), one commit.
    fn setup() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("fennec-modified-files-{}", new_id()));
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
    fn counts_committed_uncommitted_and_untracked_files() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        commit(&wt, "agent work").unwrap();
        std::fs::write(wt.workdir.join("a.txt"), "one\ntwo\nthree\nfour\n").unwrap();
        std::fs::write(wt.workdir.join("b.txt"), "x\ny\n").unwrap();

        let top = top(&repo).unwrap().0;
        let files = in_worktree(&top, &wt).unwrap();

        assert_eq!(files.count, 2);
        assert_eq!(files.added, 5);
        assert_eq!(files.removed, 0);
        assert_eq!(
            files.files,
            vec![
                ModifiedFile {
                    path: "a.txt".into(),
                    added: 3,
                    removed: 0,
                },
                ModifiedFile {
                    path: "b.txt".into(),
                    added: 2,
                    removed: 0,
                },
            ]
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn an_untracked_symlink_counts_zero_lines() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::os::unix::fs::symlink("/dev/zero", wt.workdir.join("zero")).unwrap();

        let top = top(&repo).unwrap().0;
        let files = in_worktree(&top, &wt).unwrap();

        let zero = files.files.iter().find(|file| file.path == "zero").unwrap();
        assert_eq!(zero.added, 0);
        assert_eq!(zero.removed, 0);
        std::fs::remove_dir_all(base).unwrap();
    }
}
