//! Port of openfox `tools/path-security.ts`, simplified: a path outside the
//! working directory is denied (openfox asks the human to confirm).

use std::path::{Component, Path, PathBuf};

use crate::domain::protection::Protection;

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

pub(super) fn resolve_for_writing(
    workdir: &Path,
    protection: &Protection,
    p: &str,
) -> Result<PathBuf, String> {
    let resolved = resolve_in_workdir(workdir, p)?;
    let canonical_workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());

    // `resolved` is canonical: a symlink to a protected file is caught too.
    if let Some(relative) = resolved
        .strip_prefix(&canonical_workdir)
        .ok()
        .map(|relative| relative.to_string_lossy().into_owned().replace('\\', "/"))
        && protection.is_protected(&relative)
    {
        return Err(format!(
            "{p} is protected by the human (.agents/protected, or a default: fennec's definitions, AGENTS.md, lockfiles, CI). Do not work around it: say in your report what should change there and why."
        ));
    }

    Ok(resolved)
}

/// A read may also reach into a skill package by absolute path: a loaded
/// skill tells the model to read its own reference files from its directory,
/// which usually lies outside the project (`~/.agents/skills`).
pub(super) fn resolve_for_reading(
    workdir: &Path,
    packages: impl FnOnce() -> Vec<PathBuf>,
    p: &str,
) -> Result<PathBuf, String> {
    resolve_in_workdir(workdir, p).or_else(|denied| {
        if !Path::new(p).is_absolute() {
            return Err(denied);
        }
        packages()
            .iter()
            .find_map(|dir| resolve_in_workdir(dir, p).ok())
            .ok_or(denied)
    })
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
    use super::{resolve_for_reading, resolve_for_writing};
    use crate::domain::protection::Protection;
    use crate::tools::test_support::{no_output, run_tool, temp_dir};

    #[test]
    fn a_skill_package_is_readable_by_absolute_path_only() {
        let dir = temp_dir();
        let package = temp_dir();
        let outside = temp_dir();
        std::fs::write(package.join("ref.md"), "ref").unwrap();
        let packages = || vec![package.clone()];

        let absolute = package.join("ref.md");
        assert!(resolve_for_reading(&dir, packages, absolute.to_str().unwrap()).is_ok());
        let relative = resolve_for_reading(&dir, packages, "ref.md").unwrap();
        assert!(relative.starts_with(dir.canonicalize().unwrap()));
        let elsewhere = outside.join("secret.md");
        assert!(resolve_for_reading(&dir, packages, elsewhere.to_str().unwrap()).is_err());

        for d in [dir, package, outside] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

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
    fn writing_to_protected_paths_is_refused() {
        let dir = temp_dir();
        std::fs::write(dir.join("Cargo.lock"), "lock").unwrap();
        std::fs::create_dir_all(dir.join(".agents")).unwrap();
        std::fs::write(dir.join(".agents").join("protected"), "x").unwrap();
        let protection = Protection::new(None);

        for path in ["Cargo.lock", ".agents/protected", "./sub/../Cargo.lock"] {
            let error = resolve_for_writing(&dir, &protection, path).unwrap_err();
            assert!(error.contains("is protected"), "{path}: {error}");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_to_a_symlink_to_a_protected_file_is_refused() {
        let dir = temp_dir();
        std::fs::write(dir.join("Cargo.lock"), "lock").unwrap();
        std::os::unix::fs::symlink(dir.join("Cargo.lock"), dir.join("lock-link")).unwrap();
        let protection = Protection::new(None);

        let error = resolve_for_writing(&dir, &protection, "lock-link").unwrap_err();
        assert!(error.contains("is protected"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_to_a_normal_file_is_allowed() {
        let dir = temp_dir();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let protection = Protection::new(None);

        let resolved = resolve_for_writing(&dir, &protection, "src/main.rs").unwrap();
        assert_eq!(
            resolved,
            dir.canonicalize().unwrap().join("src").join("main.rs")
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
