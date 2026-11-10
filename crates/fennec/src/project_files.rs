//! The project file explorer's reads and the human's commit: the tree as git lists
//! it, a file from the working tree or from a session's branch, and one file saved
//! as a commit on the human's current branch. The checks live in `domain/project_file.rs`.

use crate::domain::project_file::{
    CommitMessage, MAX_FILE_BYTES, MAX_LISTED, RelPath, SessionBranch,
};
use crate::git;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum Version {
    Tree,
    Session(SessionBranch),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedFile {
    pub path: String,
    pub tracked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub files: Vec<ListedFile>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub path: String,
    pub content: Option<String>,
    pub binary: bool,
    pub too_large: bool,
    pub blob: Option<String>,
    pub editable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Committed {
    pub commit: String,
    pub blob: String,
}

#[derive(Debug)]
pub enum FileError {
    Invalid(String),
    NotFound,
    Stale,
    Git(String),
}

impl FileError {
    pub fn status(&self) -> u16 {
        match self {
            FileError::Invalid(_) => 400,
            FileError::NotFound => 404,
            FileError::Stale => 409,
            FileError::Git(_) => 500,
        }
    }

    pub fn message(&self) -> String {
        match self {
            FileError::Invalid(message) => message.clone(),
            FileError::NotFound => "not found".into(),
            FileError::Stale => "the file changed since it was read: reload it".into(),
            FileError::Git(message) => message.clone(),
        }
    }
}

#[derive(Debug)]
struct TreeParts {
    full: PathBuf,
    content: Option<String>,
    binary: bool,
    too_large: bool,
    blob: Option<String>,
    tracked: bool,
}

/// A session's branch is gone once the session is integrated or discarded: its files
/// are then read from the working tree, where an integration put them.
pub fn resolve(workdir: &Path, version: Version) -> Version {
    match version {
        Version::Session(branch) => {
            let reference = format!("refs/heads/{}", branch.as_str());
            match git::stdout(workdir, &["rev-parse", "--verify", "--quiet", &reference]) {
                Some(_) => Version::Session(branch),
                None => Version::Tree,
            }
        }
        Version::Tree => Version::Tree,
    }
}

pub fn list(workdir: &Path, version: &Version) -> Result<Listing, String> {
    let mut files: Vec<ListedFile> = match version {
        Version::Tree => {
            let all = git::run(
                workdir,
                &[
                    "ls-files",
                    "-z",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                ],
                None,
            )?;
            let cached = git::run(workdir, &["ls-files", "-z", "--cached"], None)?;
            let cached_paths: HashSet<String> = split_nul(&cached).into_iter().collect();
            let mut seen = HashSet::new();
            split_nul(&all)
                .into_iter()
                .filter(|path| RelPath::parse(path).is_ok() && seen.insert(path.clone()))
                .map(|path| ListedFile {
                    tracked: cached_paths.contains(&path),
                    path,
                })
                .collect()
        }
        Version::Session(branch) => {
            let output = git::run(
                workdir,
                &["ls-tree", "-r", "-z", "--name-only", branch.as_str()],
                None,
            )?;
            let mut seen = HashSet::new();
            split_nul(&output)
                .into_iter()
                .filter(|path| RelPath::parse(path).is_ok() && seen.insert(path.clone()))
                .map(|path| ListedFile {
                    path,
                    tracked: true,
                })
                .collect()
        }
    };

    files.sort_by(|a, b| a.path.cmp(&b.path));
    let truncated = files.len() > MAX_LISTED;
    files.truncate(MAX_LISTED);

    Ok(Listing { files, truncated })
}

pub fn read(workdir: &Path, version: &Version, path: &RelPath) -> Result<FileView, FileError> {
    match version {
        Version::Tree => {
            let parts = tree_parts(workdir, path)?;
            Ok(FileView {
                path: path.as_str().to_string(),
                content: parts.content,
                binary: parts.binary,
                too_large: parts.too_large,
                blob: parts.blob,
                editable: !parts.too_large && !parts.binary && parts.tracked,
            })
        }
        Version::Session(branch) => session_view(workdir, branch, path),
    }
}

pub fn commit(
    workdir: &Path,
    path: &RelPath,
    content: &str,
    base: &str,
    message: &CommitMessage,
) -> Result<Committed, FileError> {
    if content.len() as u64 > MAX_FILE_BYTES {
        return Err(FileError::Invalid(
            "the file is too large to edit here".into(),
        ));
    }

    let parts = tree_parts(workdir, path)?;
    if !parts.tracked {
        return Err(FileError::Invalid(
            "only a tracked file can be edited".into(),
        ));
    }
    if parts.too_large {
        return Err(FileError::Invalid(
            "the file is too large to edit here".into(),
        ));
    }

    let Some(current) = parts.blob.as_deref() else {
        return Err(FileError::Stale);
    };
    if current != base {
        return Err(FileError::Stale);
    }

    std::fs::write(&parts.full, content).map_err(|e| FileError::Git(e.to_string()))?;

    git::run(
        workdir,
        &[
            "--literal-pathspecs",
            "commit",
            "--quiet",
            "--only",
            "-m",
            message.as_str(),
            "--",
            path.as_str(),
        ],
        None,
    )
    .map_err(|e| FileError::Git(format!("the file was saved but not committed: {e}")))?;

    let commit = git::stdout(workdir, &["rev-parse", "HEAD"])
        .ok_or_else(|| FileError::Git("the file was saved but HEAD is unknown".into()))?;
    let commit = commit.trim().to_string();
    let blob = hash_bytes(workdir, content.as_bytes())?;

    Ok(Committed { commit, blob })
}

fn tree_parts(workdir: &Path, path: &RelPath) -> Result<TreeParts, FileError> {
    let full = workdir.join(path.as_str());
    let metadata = std::fs::symlink_metadata(&full).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            FileError::NotFound
        } else {
            FileError::Git(e.to_string())
        }
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FileError::Invalid("not a regular file".into()));
    }

    let canonical_file = std::fs::canonicalize(&full).map_err(|e| FileError::Git(e.to_string()))?;
    let canonical_workdir =
        std::fs::canonicalize(workdir).map_err(|e| FileError::Git(e.to_string()))?;
    if !canonical_file.starts_with(&canonical_workdir) {
        return Err(FileError::Invalid("outside the project".into()));
    }

    if metadata.len() > MAX_FILE_BYTES {
        return Ok(TreeParts {
            full,
            content: None,
            binary: false,
            too_large: true,
            blob: None,
            tracked: false,
        });
    }

    let bytes = std::fs::read(&full).map_err(|e| FileError::Git(e.to_string()))?;
    let binary = std::str::from_utf8(&bytes).is_err() || bytes.contains(&0);
    let blob = hash_bytes(workdir, &bytes)?;
    let content = if binary {
        None
    } else {
        Some(String::from_utf8(bytes).map_err(|e| FileError::Git(e.to_string()))?)
    };
    let tracked = git::run(
        workdir,
        &[
            "--literal-pathspecs",
            "ls-files",
            "--error-unmatch",
            "--",
            path.as_str(),
        ],
        None,
    )
    .is_ok();

    Ok(TreeParts {
        full,
        content,
        binary,
        too_large: false,
        blob: Some(blob),
        tracked,
    })
}

fn session_view(
    workdir: &Path,
    branch: &SessionBranch,
    path: &RelPath,
) -> Result<FileView, FileError> {
    let ref_name = format!("{}:./{}", branch.as_str(), path.as_str());
    let blob = git::stdout(
        workdir,
        &[
            "--literal-pathspecs",
            "rev-parse",
            "--verify",
            "--quiet",
            &ref_name,
        ],
    )
    .filter(|value| !value.trim().is_empty())
    .map(|value| value.trim().to_string())
    .ok_or(FileError::NotFound)?;

    let kind = git::stdout(workdir, &["cat-file", "-t", &blob])
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    if kind != "blob" {
        return Err(FileError::NotFound);
    }

    let size = git::stdout(workdir, &["cat-file", "-s", &blob])
        .ok_or_else(|| FileError::Git("git cat-file -s failed".into()))?;
    let size: u64 = size
        .trim()
        .parse()
        .map_err(|e| FileError::Git(format!("git cat-file -s did not return a size: {e}")))?;

    if size > MAX_FILE_BYTES {
        return Ok(FileView {
            path: path.as_str().to_string(),
            content: None,
            binary: false,
            too_large: true,
            blob: Some(blob),
            editable: false,
        });
    }

    let bytes = git::run(workdir, &["cat-file", "blob", &blob], None)
        .map_err(|e| FileError::Git(e.to_string()))?;
    let binary = std::str::from_utf8(&bytes).is_err() || bytes.contains(&0);
    let content = if binary {
        None
    } else {
        Some(String::from_utf8(bytes).map_err(|e| FileError::Git(e.to_string()))?)
    };

    Ok(FileView {
        path: path.as_str().to_string(),
        content,
        binary,
        too_large: false,
        blob: Some(blob),
        editable: false,
    })
}

fn hash_bytes(workdir: &Path, bytes: &[u8]) -> Result<String, FileError> {
    let output = git::run(workdir, &["hash-object", "--stdin"], Some(bytes))
        .map_err(|e| FileError::Git(e.to_string()))?;
    let id = String::from_utf8_lossy(&output).trim().to_string();
    if id.is_empty() {
        Err(FileError::Git("git hash-object produced no blob id".into()))
    } else {
        Ok(id)
    }
}

fn split_nul(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|&byte| byte == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fennec-project-files-{}",
            crate::domain::id::new_id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        git::run(&dir, &["init", "-q"], None).unwrap();
        git::run(&dir, &["config", "user.email", "t@t"], None).unwrap();
        git::run(&dir, &["config", "user.name", "t"], None).unwrap();
        git::run(&dir, &["config", "commit.gpgsign", "false"], None).unwrap();
        dir
    }

    fn commit_file(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
        git::run(dir, &["add", name], None).unwrap();
        git::run(dir, &["commit", "-q", "-m", "initial"], None).unwrap();
    }

    #[test]
    fn the_tree_lists_tracked_and_new_files_but_not_ignored_ones() {
        let dir = fresh_repo();
        std::fs::write(dir.join(".gitignore"), "ignored.txt\n").unwrap();
        commit_file(&dir, "a.txt", "a");
        std::fs::write(dir.join("new.txt"), "new").unwrap();
        std::fs::write(dir.join("ignored.txt"), "ignored").unwrap();

        let listing = list(&dir, &Version::Tree).unwrap();
        let paths: Vec<&str> = listing
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();

        assert!(paths.contains(&"a.txt"));
        assert!(paths.contains(&"new.txt"));
        assert!(!paths.contains(&"ignored.txt"));
        assert!(!listing.truncated);

        assert!(
            listing
                .files
                .iter()
                .find(|file| file.path == "a.txt")
                .unwrap()
                .tracked
        );
        assert!(
            !listing
                .files
                .iter()
                .find(|file| file.path == "new.txt")
                .unwrap()
                .tracked
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_pointing_outside_the_project_is_refused() {
        let dir = fresh_repo();
        let outside = std::env::temp_dir().join(format!(
            "fennec-project-files-outside-{}",
            crate::domain::id::new_id()
        ));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("target.txt"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.join("target.txt"), dir.join("link")).unwrap();

        let path = RelPath::parse("link").unwrap();
        match read(&dir, &Version::Tree, &path) {
            Err(FileError::Invalid(message)) => {
                assert!(message.contains("regular") || message.contains("outside"));
            }
            other => panic!("expected Invalid, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_directory_pointing_outside_is_refused() {
        let dir = fresh_repo();
        let outside = std::env::temp_dir().join(format!(
            "fennec-project-files-outside-{}",
            crate::domain::id::new_id()
        ));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("f.txt"), "outside").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("link")).unwrap();

        let path = RelPath::parse("link/f.txt").unwrap();
        match read(&dir, &Version::Tree, &path) {
            Err(FileError::Invalid(message)) => assert!(message.contains("outside")),
            other => panic!("expected Invalid, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn saving_commits_only_that_file_and_keeps_the_rest_staged() {
        let dir = fresh_repo();
        commit_file(&dir, "a.txt", "a1");
        commit_file(&dir, "b.txt", "b1");

        std::fs::write(dir.join("b.txt"), "b2").unwrap();
        git::run(&dir, &["add", "b.txt"], None).unwrap();

        let base = read(&dir, &Version::Tree, &RelPath::parse("a.txt").unwrap())
            .unwrap()
            .blob
            .expect("a.txt should have a blob id");
        let committed = commit(
            &dir,
            &RelPath::parse("a.txt").unwrap(),
            "a2",
            &base,
            &CommitMessage::parse("edit a").unwrap(),
        )
        .unwrap();

        assert!(!committed.commit.is_empty());
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "a2");

        let show = git::stdout(&dir, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
        assert_eq!(show.trim(), "a.txt");

        let cached = git::stdout(&dir, &["diff", "--cached", "--name-only"]).unwrap();
        assert!(cached.lines().any(|line| line == "b.txt"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_base_is_refused_and_nothing_is_written() {
        let dir = fresh_repo();
        commit_file(&dir, "a.txt", "a");

        let head_before = git::stdout(&dir, &["rev-parse", "HEAD"]).unwrap();
        let before = std::fs::read_to_string(dir.join("a.txt")).unwrap();

        let error = commit(
            &dir,
            &RelPath::parse("a.txt").unwrap(),
            "new",
            "0000000000000000000000000000000000000000",
            &CommitMessage::parse("edit a").unwrap(),
        )
        .unwrap_err();

        assert!(matches!(error, FileError::Stale));
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), before);
        assert_eq!(
            git::stdout(&dir, &["rev-parse", "HEAD"]).unwrap(),
            head_before
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_untracked_file_cannot_be_saved() {
        let dir = fresh_repo();
        commit_file(&dir, "tracked.txt", "tracked");
        std::fs::write(dir.join("new.txt"), "new").unwrap();

        let error = commit(
            &dir,
            &RelPath::parse("new.txt").unwrap(),
            "newer",
            "0",
            &CommitMessage::parse("edit new").unwrap(),
        )
        .unwrap_err();

        assert!(matches!(error, FileError::Invalid(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_branch_is_read_from_git_not_from_disk() {
        let dir = fresh_repo();
        commit_file(&dir, "a.txt", "committed");
        git::run(&dir, &["branch", "fennec/s-1"], None).unwrap();
        std::fs::write(dir.join("a.txt"), "disk").unwrap();

        let branch = SessionBranch::parse("s-1").unwrap();
        let view = read(
            &dir,
            &Version::Session(branch),
            &RelPath::parse("a.txt").unwrap(),
        )
        .unwrap();

        assert_eq!(view.content.as_deref(), Some("committed"));
        assert!(!view.editable);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_gone_session_branch_resolves_to_the_working_tree() {
        let dir = fresh_repo();
        commit_file(&dir, "a.txt", "committed");
        git::run(&dir, &["branch", "fennec/kept"], None).unwrap();

        let kept = resolve(
            &dir,
            Version::Session(SessionBranch::parse("kept").unwrap()),
        );
        let gone = resolve(
            &dir,
            Version::Session(SessionBranch::parse("gone").unwrap()),
        );

        assert!(matches!(kept, Version::Session(_)));
        assert!(matches!(gone, Version::Tree));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pathspec_magic_name_is_taken_literally() {
        let dir = fresh_repo();
        commit_file(&dir, "x", "x");

        let head_before = git::stdout(&dir, &["rev-parse", "HEAD"]).unwrap();
        let base = read(&dir, &Version::Tree, &RelPath::parse("x").unwrap())
            .unwrap()
            .blob
            .expect("x should have a blob id");
        let path = RelPath::parse(":(glob)*").unwrap();

        let error = commit(
            &dir,
            &path,
            "new",
            &base,
            &CommitMessage::parse("edit magic").unwrap(),
        )
        .unwrap_err();

        assert!(matches!(error, FileError::NotFound));
        assert_eq!(
            git::stdout(&dir, &["rev-parse", "HEAD"]).unwrap(),
            head_before
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
