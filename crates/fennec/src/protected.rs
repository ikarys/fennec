//! The project's protected files on disk: load the human's list and find the
//! existing paths to mount read-only in the sandbox.

use std::path::{Path, PathBuf};

use crate::domain::protection::Protection;
use crate::domain::test_files;

/// The project's protection: defaults plus `.agents/protected` when present.
pub fn load(workdir: &Path) -> Protection {
    let path = workdir.join(".agents").join("protected");
    let content = std::fs::read_to_string(path).ok();
    Protection::new(content.as_deref())
}

/// Protects the tests an integration brought: the new ones among `files`
/// (relative to the project root) are appended to `.agents/protected`.
/// Returns the paths added.
pub fn protect_tests(workdir: &Path, files: &[String]) -> std::io::Result<Vec<String>> {
    let path = workdir.join(".agents").join("protected");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let added = test_files::new_test_patterns(files, &Protection::new(Some(&existing)));
    if added.is_empty() {
        return Ok(added);
    }
    std::fs::create_dir_all(workdir.join(".agents"))?;
    std::fs::write(&path, test_files::append_to(&existing, &added))?;
    Ok(added)
}

/// Existing protected paths under `workdir`, for the sandbox's read-only mounts.
pub fn existing_paths(workdir: &Path, protection: &Protection) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut visited = 0;
    // ponytail: a huge tree loses the read-only mounts past the cap; the file
    // tools still refuse.
    walk(
        workdir,
        workdir,
        protection,
        &mut found,
        &mut visited,
        100_000,
    );
    found.sort();
    found
}

fn walk(
    root: &Path,
    dir: &Path,
    protection: &Protection,
    found: &mut Vec<PathBuf>,
    visited: &mut usize,
    cap: usize,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries {
        if *visited >= cap {
            return;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        *visited += 1;

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if file_type.is_dir() {
            // ponytail: a protected path inside them is not mounted read-only;
            // walking them costs too much.
            if name == ".git" || name == "node_modules" || name == "target" {
                continue;
            }

            let Some(relative) = relative_to(root, &path) else {
                continue;
            };

            if protection.is_protected(&relative) {
                found.push(path);
            } else {
                walk(root, &path, protection, found, visited, cap);
            }
        } else {
            let Some(relative) = relative_to(root, &path) else {
                continue;
            };

            if protection.is_protected(&relative) {
                found.push(path);
            }
        }
    }
}

fn relative_to(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| relative.to_string_lossy().into_owned().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fennec-protected-test-{}-{nanos}-{counter}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_missing_file_uses_defaults() {
        let workdir = temp_dir();
        let protection = load(&workdir);

        assert!(protection.is_protected("Cargo.lock"));
        assert!(!protection.is_protected("src/main.rs"));

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn load_reads_project_protected_file() {
        let workdir = temp_dir();
        std::fs::create_dir_all(workdir.join(".agents")).unwrap();
        std::fs::write(
            workdir.join(".agents").join("protected"),
            "# project protected files\n\nsecrets/\n!Cargo.lock\n",
        )
        .unwrap();

        let protection = load(&workdir);
        assert!(protection.is_protected("secrets/api.key"));
        assert!(protection.is_protected("Cargo.lock"));

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn existing_paths_returns_protected_paths_sorted() {
        let workdir = temp_dir();
        std::fs::write(workdir.join("Cargo.lock"), "lock").unwrap();
        std::fs::create_dir_all(workdir.join(".agents").join("agents")).unwrap();
        std::fs::write(
            workdir
                .join(".agents")
                .join("agents")
                .join("builder.agent.md"),
            "agent",
        )
        .unwrap();
        std::fs::create_dir_all(workdir.join("web")).unwrap();
        std::fs::write(workdir.join("web").join("package-lock.json"), "{}").unwrap();
        std::fs::create_dir_all(workdir.join("src")).unwrap();
        std::fs::write(workdir.join("src").join("main.rs"), "fn main() {}").unwrap();

        let protection = Protection::new(None);
        let paths = existing_paths(&workdir, &protection);

        assert_eq!(
            paths,
            vec![
                workdir.join(".agents"),
                workdir.join("Cargo.lock"),
                workdir.join("web").join("package-lock.json"),
            ]
        );

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn existing_paths_skips_heavy_directories() {
        let workdir = temp_dir();
        for name in [".git", "node_modules", "target"] {
            std::fs::create_dir_all(workdir.join(name)).unwrap();
            std::fs::write(workdir.join(name).join("Cargo.lock"), "lock").unwrap();
        }

        let protection = Protection::new(None);
        assert!(existing_paths(&workdir, &protection).is_empty());

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn existing_paths_does_not_follow_symlinked_directories() {
        let workdir = temp_dir();
        let outside = temp_dir();
        std::fs::create_dir_all(outside.join(".agents")).unwrap();
        std::fs::write(outside.join(".agents").join("protected"), "x").unwrap();
        std::os::unix::fs::symlink(&outside, workdir.join("link")).unwrap();

        let protection = Protection::new(None);
        assert!(existing_paths(&workdir, &protection).is_empty());

        let _ = std::fs::remove_dir_all(&workdir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn walk_stops_at_the_cap() {
        let workdir = temp_dir();
        for i in 0..5 {
            std::fs::write(workdir.join(format!("file{i}.txt")), "x").unwrap();
        }

        let protection = Protection::new(None);
        let mut found = Vec::new();
        let mut visited = 0;
        walk(&workdir, &workdir, &protection, &mut found, &mut visited, 2);

        assert_eq!(visited, 2);
        assert!(found.is_empty());

        let _ = std::fs::remove_dir_all(&workdir);
    }
}
