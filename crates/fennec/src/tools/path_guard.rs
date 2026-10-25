//! Port of openfox `tools/path-security.ts`, simplified: a path outside the
//! working directory is denied (openfox asks the human to confirm).

use std::path::{Component, Path, PathBuf};

pub(super) fn resolve_in_workdir(workdir: &Path, p: &str) -> Result<PathBuf, String> {
    let canonical_workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let normalized = lexical_normalize(workdir, p);

    let mut ancestor = normalized.clone();
    let mut remaining = Vec::new();

    loop {
        // symlink_metadata, not exists(): a dangling symlink must count as
        // existing so canonicalize below rejects it instead of letting a
        // write follow it out of the workdir.
        if ancestor.symlink_metadata().is_ok() {
            break;
        }

        if let Some(component) = ancestor.components().next_back() {
            remaining.push(component.as_os_str().to_owned());
        }

        if !ancestor.pop() {
            if ancestor.as_os_str().is_empty() {
                ancestor = PathBuf::from("/");
            }
            break;
        }
    }

    let canonical_ancestor = ancestor
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path {p}: {e}"))?;

    if canonical_ancestor != canonical_workdir
        && !canonical_ancestor.starts_with(&canonical_workdir)
    {
        return Err(format!("Path outside working directory: {p}"));
    }

    let mut resolved = canonical_ancestor;
    for component in remaining.iter().rev() {
        resolved.push(component);
    }

    Ok(resolved)
}

fn lexical_normalize(workdir: &Path, p: &str) -> PathBuf {
    let path = Path::new(p);
    let mut normalized = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        workdir.to_path_buf()
    };

    for component in path.components() {
        match component {
            Component::RootDir => {
                normalized = PathBuf::from("/");
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && normalized.as_os_str().is_empty() {
                    normalized = PathBuf::from("/");
                }
            }
            Component::Normal(_) => normalized.push(component.as_os_str()),
            Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use crate::tools::test_support::{no_output, run_tool, temp_dir};

    #[test]
    fn path_escape_via_parent_dir_denied() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let args = serde_json::json!({
            "path": "../escape.txt",
            "content": "outside"
        });
        let result = run_tool("write_file", &args, &dir, &mut on_output);
        assert!(!result.success);
        assert!(
            result
                .error
                .unwrap()
                .contains("Path outside working directory")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn symlink_escape_denied() {
        let dir = temp_dir();
        let outside = temp_dir();
        let mut on_output = no_output();

        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), dir.join("link.txt")).unwrap();

        let args = serde_json::json!({
            "path": "link.txt"
        });
        let result = run_tool("read_file", &args, &dir, &mut on_output);
        assert!(!result.success);
        assert!(
            result
                .error
                .unwrap()
                .contains("Path outside working directory")
        );

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
