//! The folders the human browses to pick a project directory.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct Folder {
    pub name: String,
    pub path: String,
    pub hidden: bool,
    /// A git repository: the likely project roots.
    pub repo: bool,
}

#[derive(Serialize)]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub folders: Vec<Folder>,
}

/// The sub-folders of `dir`, by name. Entries that cannot be read (broken
/// links, no permission) are left out rather than failing the listing.
pub fn list(dir: &Path) -> Result<Listing, String> {
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut folders: Vec<Folder> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            Folder {
                hidden: name.starts_with('.'),
                repo: path.join(".git").exists(),
                path: path.display().to_string(),
                name,
            }
        })
        .collect();
    folders.sort_by_key(|f| f.name.to_lowercase());
    Ok(Listing {
        parent: dir.parent().map(|p| p.display().to_string()),
        path: dir.display().to_string(),
        folders,
    })
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// A new project directory `base/name`: an empty git repository with a first
/// empty commit, ready for fennec's worktree-based integration. `name` is a
/// single level: no `/`, not starting with `.`, and not taken.
pub fn create(base: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("empty name".into());
    }
    if name.contains('/') || name.contains(std::path::MAIN_SEPARATOR) {
        return Err("a name without a separator".into());
    }
    if name.starts_with('.') {
        return Err("a name not starting with .".into());
    }
    let base = base
        .canonicalize()
        .map_err(|e| format!("{}: {e}", base.display()))?;
    let path = base.join(name);
    if path.exists() {
        return Err(format!("{}: already exists", path.display()));
    }
    std::fs::create_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    // A missing git identity leaves the folder created but uncommitted; the
    // project stays openable either way.
    crate::worktree::git(&path, &["init", "-q"], None).map_err(|e| format!("git init: {e}"))?;
    let _ = crate::worktree::git(
        &path,
        &["commit", "--allow-empty", "-q", "-m", "fennec: initial"],
        None,
    );
    Ok(path)
}

/// fennec's own store (its database, the sessions' worktrees) is never a
/// project: an integration or a clean-up could delete it under a session.
pub fn inside_store(dir: &Path, store_dir: &Path) -> bool {
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    canon(dir).starts_with(canon(store_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_sub_folders_sorted_with_repos_and_hidden_marked() {
        let root =
            std::env::temp_dir().join(format!("fennec-folders-{}", crate::domain::id::new_id()));
        for d in ["b", "A/.git", ".cache"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("file.txt"), "").unwrap();
        let listing = list(&root).unwrap();
        let names: Vec<_> = listing
            .folders
            .iter()
            .map(|f| (f.name.as_str(), f.repo, f.hidden))
            .collect();
        assert_eq!(
            names,
            [
                (".cache", false, true),
                ("A", true, false),
                ("b", false, false)
            ]
        );
        assert_eq!(
            listing.parent.as_deref(),
            root.canonicalize()
                .unwrap()
                .parent()
                .and_then(|p| p.to_str())
        );
        assert!(list(&root.join("missing")).is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn create_makes_a_new_git_repo_and_rejects_bad_names() {
        let root = std::env::temp_dir().join(format!(
            "fennec-folders-create-{}",
            crate::domain::id::new_id()
        ));
        std::fs::create_dir_all(&root).unwrap();

        let created = create(&root, "proj").unwrap();
        assert_eq!(created, root.join("proj"));
        assert!(created.is_dir());
        assert!(created.join(".git").exists());

        assert!(create(&root, "proj").is_err());
        assert!(create(&root, "").is_err());
        assert!(create(&root, "a/b").is_err());
        assert!(create(&root, ".hidden").is_err());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn refuses_the_store_and_what_it_holds() {
        let root = std::env::temp_dir().join(format!(
            "fennec-folders-store-{}",
            crate::domain::id::new_id()
        ));
        let store = root.join("store");
        std::fs::create_dir_all(store.join("worktrees/x")).unwrap();
        let other = root.join("other");
        std::fs::create_dir_all(&other).unwrap();

        assert!(inside_store(&store.join("worktrees/x"), &store));
        assert!(inside_store(&store, &store));
        assert!(!inside_store(&other, &store));

        std::fs::remove_dir_all(root).ok();
    }
}
