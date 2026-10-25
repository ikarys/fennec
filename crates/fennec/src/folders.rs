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
}
