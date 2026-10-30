//! The project's documentation files on disk: the markdown the human commits
//! (`AGENTS.md`, `README.md`, `docs/`), read for the project page's Doc tab.
//! Its ADRs are read separately (`adr.rs`) and rendered on their own.

use std::fs;
use std::path::Path;

use crate::domain::doc::{self, DocFile};

/// The project's doc files, sorted by path so the tab's list is stable.
pub fn load(workdir: &Path) -> Vec<DocFile> {
    let mut files = Vec::new();
    push_root(workdir, "AGENTS.md", &mut files);
    push_root(workdir, "README.md", &mut files);
    let docs_dir = workdir.join("docs");
    collect(&docs_dir, "docs", &mut files);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

fn push_root(workdir: &Path, name: &str, files: &mut Vec<DocFile>) {
    let full = workdir.join(name);
    if let Ok(content) = fs::read_to_string(&full) {
        files.push(DocFile {
            path: name.to_string(),
            name: name.to_string(),
            content,
        });
    }
}

fn collect(dir: &Path, rel: &str, files: &mut Vec<DocFile>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        let path = entry.path();
        // A symlinked directory (e.g. `docs/self -> .`) would recurse forever.
        if path.is_symlink() {
            continue;
        }
        let rel_path = format!("{rel}/{file_name}");
        if path.is_dir() {
            collect(&path, &rel_path, files);
        } else if doc::is_doc_path(&rel_path)
            && let Ok(content) = fs::read_to_string(&path)
        {
            files.push(DocFile {
                path: rel_path,
                name: file_name,
                content,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("fennec-doc-{}-{name}", std::process::id()))
    }

    #[test]
    fn load_missing_is_empty() {
        let workdir = temp_dir("load_missing_is_empty");
        assert!(load(&workdir).is_empty());
        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn load_reads_docs_sorted_and_skips_adrs() {
        let workdir = temp_dir("load_reads_docs_sorted_and_skips_adrs");
        for file in [
            "AGENTS.md",
            "README.md",
            "docs/api.md",
            "docs/sub/notes.md",
            "docs/adr/0001-x.md",
            "docs/readme.txt",
        ] {
            let full = workdir.join(file);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full, "# body").unwrap();
        }

        let paths: Vec<String> = load(&workdir).into_iter().map(|f| f.path).collect();
        assert_eq!(
            paths,
            vec!["AGENTS.md", "README.md", "docs/api.md", "docs/sub/notes.md"]
        );

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[cfg(unix)]
    #[test]
    fn load_skips_symlinked_dirs() {
        let workdir = temp_dir("load_skips_symlinked_dirs");
        fs::create_dir_all(workdir.join("docs")).unwrap();
        fs::write(workdir.join("AGENTS.md"), "# agents").unwrap();
        fs::write(workdir.join("docs/api.md"), "# api").unwrap();
        // `docs/self -> .` points back to the workdir (which has `docs/`):
        // without the symlink guard this recurses forever.
        std::os::unix::fs::symlink(workdir.as_path(), workdir.join("docs/self")).unwrap();

        let paths: Vec<String> = load(&workdir).into_iter().map(|f| f.path).collect();
        assert_eq!(paths, vec!["AGENTS.md", "docs/api.md"]);

        let _ = std::fs::remove_dir_all(&workdir);
    }
}
