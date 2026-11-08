//! The disk the worktrees take, measured on demand, and the space freed on the human's click: build output emptied, orphans removed.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// What one directory under the worktrees root takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeUsage {
    /// The directory's name: the root session id for a session's worktree.
    pub name: String,
    pub path: PathBuf,
    /// Allocated bytes (st_blocks * 512, like `du`), symlinks not followed.
    pub bytes: u64,
    /// The part in generated directories (see GENERATED), regenerable by a build.
    pub generated_bytes: u64,
    /// Some file is owned by another user (a container wrote as root): only `sudo rm` frees it.
    pub foreign_owned: bool,
    /// The repository the worktree belongs to, read from its `.git` file; None when unreadable.
    pub repo_top: Option<PathBuf>,
    /// The repository still registers this worktree (its `gitdir` exists).
    pub registered: bool,
}

/// Directory names holding build output or installed dependencies.
pub const GENERATED: [&str; 3] = ["target", "node_modules", ".venv"];

/// The shared Cargo target directories (one per project, see `shared_target`): a cache, all of it regenerable.
pub const CARGO_CACHE: &str = ".cargo-target";

/// Every directory directly under `root`, largest first. A missing root is an empty list.
pub fn scan(root: &Path) -> Vec<WorktreeUsage> {
    if !root.is_dir() {
        return Vec::new();
    }

    let mut usages = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_dir() {
                continue;
            }

            let root_uid = meta.uid();
            let (bytes, mut generated_bytes, foreign_owned) = measure(&path, 0, false, root_uid);
            if entry.file_name() == CARGO_CACHE {
                generated_bytes = bytes;
            }
            let (repo_top, registered) = repository(&path);

            usages.push(WorktreeUsage {
                name: entry.file_name().to_string_lossy().into_owned(),
                path,
                bytes,
                generated_bytes,
                foreign_owned,
                repo_top,
                registered,
            });
        }
    }

    usages.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    usages
}

/// A directory name the routes may act on: non-empty, at most 128 chars, only ASCII alphanumerics and '-'
/// (session ids look like `18e33a927f514967-22e8-0`). Rejects anything that could escape `root`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Empties the generated directories of one worktree: `target` is removed, `node_modules` and `.venv`
/// are emptied but kept (the sandbox mounts the main checkout's copies on them). Returns the bytes freed.
/// A permission error is an Err that ends with the command to run: "`sudo rm -rf <dir>`".
/// The shared Cargo cache is emptied whole.
pub fn clean_generated(path: &Path) -> Result<u64, String> {
    let mut dirs = Vec::new();
    if path.file_name().is_some_and(|name| name == CARGO_CACHE) {
        dirs.push(path.to_path_buf());
    } else {
        generated_dirs(path, 0, &mut dirs);
    }
    let mut freed = 0;
    for dir in dirs {
        if dir.file_name().is_some_and(|name| name == "target") {
            freed += subtree(&dir, 0).0;
            fs::remove_dir_all(&dir).map_err(|e| map_remove_error(e, &dir))?;
            continue;
        }
        let entries = fs::read_dir(&dir).map_err(|e| map_remove_error(e, &dir))?;
        for entry in entries.flatten() {
            let child = entry.path();
            freed += subtree(&child, 0).0;
            remove_entry(&child).map_err(|e| map_remove_error(e, &child))?;
        }
    }
    Ok(freed)
}

/// The generated directories below `path`, by the same rule as `measure`.
fn generated_dirs(path: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        let name = entry.file_name();
        if !is_dir || name == ".git" || depth >= 4 {
            continue;
        }
        if GENERATED.iter().any(|g| name == *g) {
            found.push(child);
        } else {
            generated_dirs(&child, depth + 1, found);
        }
    }
}

/// Removes an orphan directory `root/name` and, when its repository is known and still exists, what git
/// keeps of it: `git worktree prune`, then (errors ignored, they may already be gone) `git branch -D fennec/<name>`,
/// `git update-ref -d refs/fennec/base/<name>`, `git symbolic-ref --delete refs/fennec/origin/<name>`.
/// Err when `name` is not valid_name. A permission error is an Err ending with "`sudo rm -rf <path>`".
pub fn remove_orphan(root: &Path, name: &str) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!("invalid name: {name}"));
    }

    let path = root.join(name);
    let (repo_top, _) = repository(&path);
    crate::sandbox::remove_private_tmp(&path);

    match fs::remove_dir_all(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(map_remove_error(e, &path)),
    }

    if let Some(repo) = repo_top.filter(|repo| repo.is_dir()) {
        let _ = crate::git::run(&repo, &["worktree", "prune"], None);

        let branch = format!("fennec/{name}");
        let _ = crate::git::run(&repo, &["branch", "-D", &branch], None);

        let base_ref = format!("refs/fennec/base/{name}");
        let _ = crate::git::run(&repo, &["update-ref", "-d", &base_ref], None);

        let origin_ref = format!("refs/fennec/origin/{name}");
        let _ = crate::git::run(&repo, &["symbolic-ref", "--delete", &origin_ref], None);
    }

    Ok(())
}

fn measure(path: &Path, depth: usize, in_git: bool, root_uid: u32) -> (u64, u64, bool) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return (0, 0, false);
    };

    let mut foreign = meta.uid() != root_uid;
    if !meta.is_dir() {
        return (meta.blocks() * 512, 0, foreign);
    }

    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let in_git = in_git || name == ".git";

    if depth > 0 && !in_git && depth <= 4 && GENERATED.contains(&name) {
        let (total, sub_foreign) = subtree(path, root_uid);
        return (total, total, foreign || sub_foreign);
    }

    let mut total = meta.blocks() * 512;
    let mut generated = 0;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let (t, g, f) = measure(&entry.path(), depth + 1, in_git, root_uid);
            total += t;
            generated += g;
            foreign |= f;
        }
    }

    (total, generated, foreign)
}

fn subtree(path: &Path, root_uid: u32) -> (u64, bool) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return (0, false);
    };

    let mut total = meta.blocks() * 512;
    let mut foreign = meta.uid() != root_uid;

    if meta.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            let (t, f) = subtree(&entry.path(), root_uid);
            total += t;
            foreign |= f;
        }
    }

    (total, foreign)
}

fn remove_entry(path: &Path) -> Result<(), std::io::Error> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        _ => fs::remove_file(path),
    }
}

fn map_remove_error(err: std::io::Error, path: &Path) -> String {
    if err.kind() == ErrorKind::PermissionDenied {
        format!(
            "remove {}: permission denied, some files are another user's (root?): `sudo rm -rf {}`",
            path.display(),
            path.display()
        )
    } else {
        format!("remove {}: {}", path.display(), err)
    }
}

/// The repository a worktree belongs to (from its `.git` file) and whether git still registers it.
pub fn repository(path: &Path) -> (Option<PathBuf>, bool) {
    let git_path = path.join(".git");
    let Ok(content) = fs::read_to_string(&git_path) else {
        return (None, false);
    };

    let Some(rest) = content.trim().strip_prefix("gitdir: ") else {
        return (None, false);
    };

    let gitdir = PathBuf::from(rest);
    let components: Vec<_> = gitdir.components().collect();

    let repo_top = if components.len() >= 3
        && components[components.len() - 3].as_os_str() == std::ffi::OsStr::new(".git")
        && components[components.len() - 2].as_os_str() == std::ffi::OsStr::new("worktrees")
    {
        gitdir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(PathBuf::from)
    } else {
        None
    };

    (repo_top, gitdir.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("fennec-usage-{}", crate::domain::id::new_id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_file(dir: &Path, name: &str, size: usize) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), vec![b'x'; size]).unwrap();
    }

    #[test]
    fn scan_missing_root_is_empty() {
        let root = temp_dir().join("missing");
        assert!(scan(&root).is_empty());
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn scan_counts_generated_and_sorts_largest_first() {
        let root = temp_dir();

        let a = root.join("a");
        write_file(&a.join("target"), "out.bin", 8192);
        write_file(&a, "src.rs", 4096);

        let b = root.join("b");
        write_file(&b, "big.bin", 32768);

        let usages = scan(&root);
        assert_eq!(usages.len(), 2);
        assert_eq!(usages[0].name, "b");

        let a_usage = usages.iter().find(|u| u.name == "a").unwrap();
        assert!(a_usage.generated_bytes > 0);
        assert!(a_usage.bytes > a_usage.generated_bytes);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nested_generated_directory_is_not_counted_twice() {
        let root = temp_dir();
        let worktree = root.join("worktree");

        write_file(&worktree.join("web/node_modules"), "x", 8192);
        write_file(&worktree.join("web/node_modules/target"), "y", 8192);

        let usage = scan(&root).into_iter().next().unwrap();
        assert_eq!(
            usage.generated_bytes,
            subtree(&worktree.join("web/node_modules"), 0).0
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clean_generated_removes_target_and_empties_node_modules() {
        let root = temp_dir();
        let worktree = root.join("worktree");

        write_file(&worktree.join("target"), "t", 8192);
        write_file(&worktree.join("web/node_modules"), "n", 8192);
        write_file(&worktree, "src", 4096);

        let freed = clean_generated(&worktree).unwrap();
        assert!(freed > 0);

        assert!(!worktree.join("target").exists());
        assert!(worktree.join("web/node_modules").is_dir());
        assert_eq!(
            fs::read_dir(worktree.join("web/node_modules"))
                .unwrap()
                .count(),
            0
        );
        assert!(worktree.join("src").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cargo_cache_is_all_generated_and_emptied_whole() {
        let root = temp_dir();
        let cache = root.join(CARGO_CACHE);
        write_file(&cache.join("home-me-project/debug"), "lib", 8192);

        let usage = scan(&root).into_iter().next().unwrap();
        assert_eq!(usage.generated_bytes, usage.bytes);

        assert!(clean_generated(&cache).unwrap() > 0);
        assert!(cache.is_dir());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn valid_name_accepts_session_ids_and_rejects_escapes() {
        assert!(valid_name("18e33a927f514967-22e8-0"));
        assert!(!valid_name(""));
        assert!(!valid_name(".."));
        assert!(!valid_name("a/b"));
        assert!(!valid_name("a b"));
    }

    #[test]
    fn remove_orphan_removes_worktree_directory_and_branch() {
        let base = temp_dir();
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();

        crate::git::run(&repo, &["init", "-q"], None).unwrap();
        crate::git::run(&repo, &["config", "user.name", "Fennec"], None).unwrap();
        crate::git::run(&repo, &["config", "user.email", "fennec@example.com"], None).unwrap();

        write_file(&repo, "file.txt", 8192);
        crate::git::run(&repo, &["add", "file.txt"], None).unwrap();
        crate::git::run(&repo, &["commit", "-q", "-m", "init"], None).unwrap();

        let root = base.join("worktrees");
        std::fs::create_dir_all(&root).unwrap();

        let name = "18e33a927f514967-22e8-0";
        let worktree = root.join(name);
        let branch = format!("fennec/{name}");

        crate::git::run(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                &branch,
                worktree.to_str().unwrap(),
                "HEAD",
            ],
            None,
        )
        .unwrap();

        let usage = scan(&root).into_iter().find(|u| u.name == name).unwrap();
        assert_eq!(usage.repo_top, Some(repo.clone()));
        assert!(usage.registered);

        remove_orphan(&root, name).unwrap();

        assert!(!worktree.exists());
        let branches = crate::git::run(&repo, &["branch", "--list", &branch], None).unwrap();
        assert!(branches.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }
}
