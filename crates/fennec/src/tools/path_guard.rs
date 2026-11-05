//! The working-directory path guard: a path outside the
//! working directory is denied unless the human's rules allow it; the ask
//! flow comes later.

use std::path::{Component, Path, PathBuf};

use crate::domain::protection::Protection;
use rules::{Access, Decision, PathRequest, Request, Rules};

fn resolve(workdir: &Path, p: &str) -> Result<PathBuf, String> {
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

    let mut resolved = canonical_ancestor;
    for component in remaining.iter().rev() {
        resolved.push(component);
    }

    Ok(resolved)
}

pub(super) fn resolve_in_workdir(workdir: &Path, p: &str) -> Result<PathBuf, String> {
    let resolved = resolve(workdir, p)?;
    let canonical_workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());

    if resolved != canonical_workdir && !resolved.starts_with(&canonical_workdir) {
        return Err(format!("Path outside working directory: {p}"));
    }

    Ok(resolved)
}

pub(super) fn path_request(
    workdir: &Path,
    home: Option<&str>,
    p: &str,
    access: Access,
) -> Option<PathRequest> {
    let resolved = resolve(workdir, p).ok()?;
    let canonical_workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());

    let relative = resolved
        .strip_prefix(&canonical_workdir)
        .ok()
        .map(|stripped| stripped.to_string_lossy().replace('\\', "/"));

    Some(PathRequest {
        absolute: resolved.to_string_lossy().replace('\\', "/"),
        relative,
        home: home.map(str::to_owned),
        access,
    })
}

/// The paths outside the workdir the human's rules grant.
pub(super) struct Outside<'a> {
    pub rules: &'a Rules,
    pub home: Option<&'a str>,
    pub(super) approved: bool,
}

impl<'a> Outside<'a> {
    /// No grant: tests of the file tools run inside their workdir.
    #[cfg(test)]
    pub(super) fn none() -> Outside<'static> {
        Outside {
            rules: &rules::NO_RULES,
            home: None,
            approved: false,
        }
    }

    fn grants(&self, workdir: &Path, p: &str, access: Access) -> Option<PathBuf> {
        let request = path_request(workdir, self.home, p, access)?;
        if request.relative.is_some() {
            return None;
        }

        if self.approved {
            return Some(PathBuf::from(request.absolute));
        }

        let decision = self.rules.decide(&Request::Path(&request));
        if matches!(decision, Decision::Allow(Some(_))) {
            Some(PathBuf::from(request.absolute))
        } else {
            None
        }
    }
}

pub(super) fn resolve_for_writing(
    workdir: &Path,
    protection: &Protection,
    outside: &Outside,
    p: &str,
) -> Result<PathBuf, String> {
    let resolved = match resolve_in_workdir(workdir, p) {
        Ok(resolved) => resolved,
        Err(denied) => return outside.grants(workdir, p, Access::Write).ok_or(denied),
    };

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
            "{p} is protected by the human (.agents/protected, or a default: fennec's definitions, AGENTS.md, lockfiles, CI). Do not work around it. If the change is required, call request_waiver with this path and why; otherwise say in your report what should change there and why."
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
    outside: &Outside,
    p: &str,
) -> Result<PathBuf, String> {
    resolve_in_workdir(workdir, p).or_else(|denied| {
        if let Some(granted) = outside.grants(workdir, p, Access::Read) {
            return Ok(granted);
        }

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

/// The message for a file the model named but which does not exist: it
/// points at the search to run rather than letting the model guess paths.
pub(super) fn not_found(path: &str) -> String {
    let name = Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    format!("File not found: {path}. Find it with: rg --files | rg '{name}'")
}

#[cfg(test)]
mod tests {
    use super::{Outside, resolve_for_reading, resolve_for_writing};
    use crate::domain::protection::Protection;
    use crate::tools::test_support::{no_output, run_tool, temp_dir};
    use rules::{Access, Action, Origin, Rule, Rules, Scope, Target, TargetKind};

    #[test]
    fn a_skill_package_is_readable_by_absolute_path_only() {
        let dir = temp_dir();
        let package = temp_dir();
        let outside = temp_dir();
        std::fs::write(package.join("ref.md"), "ref").unwrap();
        let packages = || vec![package.clone()];
        let outside_rules = Outside::none();

        let absolute = package.join("ref.md");
        assert!(
            resolve_for_reading(&dir, packages, &outside_rules, absolute.to_str().unwrap()).is_ok()
        );
        let relative = resolve_for_reading(&dir, packages, &outside_rules, "ref.md").unwrap();
        assert!(relative.starts_with(dir.canonicalize().unwrap()));
        let elsewhere = outside.join("secret.md");
        assert!(
            resolve_for_reading(&dir, packages, &outside_rules, elsewhere.to_str().unwrap())
                .is_err()
        );

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
        let outside = Outside::none();

        for path in ["Cargo.lock", ".agents/protected", "./sub/../Cargo.lock"] {
            let error = resolve_for_writing(&dir, &protection, &outside, path).unwrap_err();
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
        let outside = Outside::none();

        let error = resolve_for_writing(&dir, &protection, &outside, "lock-link").unwrap_err();
        assert!(error.contains("is protected"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_to_a_normal_file_is_allowed() {
        let dir = temp_dir();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let protection = Protection::new(None);
        let outside = Outside::none();

        let resolved = resolve_for_writing(&dir, &protection, &outside, "src/main.rs").unwrap();
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

    #[test]
    fn an_approved_call_reaches_outside_the_workdir_without_rules() {
        let workdir = temp_dir();
        let outside_dir = temp_dir();
        std::fs::write(outside_dir.join("note.txt"), "note").unwrap();
        let outside = Outside {
            rules: &rules::NO_RULES,
            home: None,
            approved: true,
        };
        let file = outside_dir.join("note.txt").to_string_lossy().into_owned();

        assert!(resolve_for_reading(&workdir, Vec::new, &outside, &file).is_ok());

        let _ = std::fs::remove_dir_all(&workdir);
        let _ = std::fs::remove_dir_all(&outside_dir);
    }

    #[test]
    fn an_allow_read_rule_grants_reading_outside_the_workdir_only() {
        let workdir = temp_dir();
        let outside_dir = temp_dir();
        std::fs::write(outside_dir.join("note.txt"), "note").unwrap();
        let rule = Rule {
            id: 1,
            scope: Scope::Global,
            target: Target::parse(
                TargetKind::Path,
                &format!("{}/", outside_dir.to_string_lossy()),
                Some(Access::Read),
            )
            .unwrap(),
            action: Action::Allow,
            note: None,
            origin: Origin::Human,
        };
        let rules = Rules::new(vec![rule]);
        let outside = Outside {
            rules: &rules,
            home: None,
            approved: false,
        };
        let protection = Protection::new(None);
        let file = outside_dir.join("note.txt").to_string_lossy().into_owned();

        assert!(resolve_for_reading(&workdir, Vec::new, &outside, &file).is_ok());
        let error = resolve_for_writing(&workdir, &protection, &outside, &file).unwrap_err();
        assert!(error.contains("Path outside working directory"));

        let _ = std::fs::remove_dir_all(&workdir);
        let _ = std::fs::remove_dir_all(&outside_dir);
    }

    #[test]
    fn an_absolute_path_outside_the_workdir_is_denied_for_reading() {
        let workdir = temp_dir();
        let outside = temp_dir();
        let path = outside.join("file.txt").to_string_lossy().into_owned();

        let result = resolve_for_reading(&workdir, Vec::new, &Outside::none(), &path);

        assert!(
            result
                .unwrap_err()
                .contains("Path outside working directory")
        );
        let _ = std::fs::remove_dir_all(workdir);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn an_absolute_path_outside_the_workdir_is_denied_for_writing() {
        let workdir = temp_dir();
        let outside = temp_dir();
        let path = outside.join("file.txt").to_string_lossy().into_owned();

        let result = resolve_for_writing(&workdir, &Protection::new(None), &Outside::none(), &path);

        assert!(
            result
                .unwrap_err()
                .contains("Path outside working directory")
        );
        let _ = std::fs::remove_dir_all(workdir);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn a_tilde_path_is_treated_as_literal_inside_the_workdir() {
        let workdir = temp_dir();
        let canonical = workdir.canonicalize().unwrap();

        // `~` is not expanded; it is an ordinary path component.
        let resolved =
            resolve_for_writing(&workdir, &Protection::new(None), &Outside::none(), "~/x").unwrap();

        assert_eq!(resolved, canonical.join("~/x"));
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn a_deep_traversal_outside_the_workdir_is_denied() {
        let workdir = temp_dir();

        let result = resolve_for_writing(
            &workdir,
            &Protection::new(None),
            &Outside::none(),
            "a/b/../../../../etc/passwd",
        );

        assert!(
            result
                .unwrap_err()
                .contains("Path outside working directory")
        );
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn a_traversal_that_comes_back_inside_is_allowed() {
        let workdir = temp_dir();
        let canonical = workdir.canonicalize().unwrap();

        let resolved = resolve_for_writing(
            &workdir,
            &Protection::new(None),
            &Outside::none(),
            "sub/../inner.txt",
        )
        .unwrap();

        assert_eq!(resolved, canonical.join("inner.txt"));
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn dot_and_slash_resolve_to_the_canonical_workdir() {
        let workdir = temp_dir();
        let canonical = workdir.canonicalize().unwrap();
        let protection = Protection::new(None);
        let outside = Outside::none();

        assert_eq!(
            resolve_for_writing(&workdir, &protection, &outside, ".").unwrap(),
            canonical
        );
        assert_eq!(
            resolve_for_writing(&workdir, &protection, &outside, "./").unwrap(),
            canonical
        );

        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn a_symlinked_directory_pointing_outside_denies_files_under_it() {
        let workdir = temp_dir();
        let outside = temp_dir();
        std::os::unix::fs::symlink(&outside, workdir.join("link-dir")).unwrap();

        let result = resolve_for_writing(
            &workdir,
            &Protection::new(None),
            &Outside::none(),
            "link-dir/file.txt",
        );

        assert!(
            result
                .unwrap_err()
                .contains("Path outside working directory")
        );
        let _ = std::fs::remove_dir_all(workdir);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn a_dangling_symlink_pointing_outside_is_denied_for_writing() {
        let workdir = temp_dir();
        let outside = temp_dir();
        std::os::unix::fs::symlink(outside.join("missing"), workdir.join("link")).unwrap();

        // A dangling symlink still exists as a path entry.
        let result =
            resolve_for_writing(&workdir, &Protection::new(None), &Outside::none(), "link");

        // A dangling link cannot be resolved, so it is refused before any wall is checked.
        assert!(result.unwrap_err().contains("Cannot resolve path"));
        let _ = std::fs::remove_dir_all(workdir);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn a_symlink_to_a_sibling_file_inside_the_workdir_is_allowed() {
        let workdir = temp_dir();
        let canonical = workdir.canonicalize().unwrap();
        std::fs::File::create(workdir.join("real.txt")).unwrap();
        std::os::unix::fs::symlink("real.txt", workdir.join("link.txt")).unwrap();

        let resolved = resolve_for_writing(
            &workdir,
            &Protection::new(None),
            &Outside::none(),
            "link.txt",
        )
        .unwrap();

        assert_eq!(resolved, canonical.join("real.txt"));
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn a_new_file_in_a_missing_subdirectory_resolves_inside_the_workdir() {
        let workdir = temp_dir();
        let canonical = workdir.canonicalize().unwrap();

        let resolved = resolve_for_writing(
            &workdir,
            &Protection::new(None),
            &Outside::none(),
            "new/dir/file.txt",
        )
        .unwrap();

        assert_eq!(resolved, canonical.join("new/dir/file.txt"));
        let _ = std::fs::remove_dir_all(workdir);
    }

    #[test]
    fn a_denial_error_mentions_the_path_as_given() {
        let workdir = temp_dir();
        let outside = temp_dir();
        std::os::unix::fs::symlink(&outside, workdir.join("link-dir")).unwrap();

        let err = resolve_for_writing(
            &workdir,
            &Protection::new(None),
            &Outside::none(),
            "link-dir/file.txt",
        )
        .unwrap_err();

        assert!(err.contains("link-dir/file.txt"));
        let _ = std::fs::remove_dir_all(workdir);
        let _ = std::fs::remove_dir_all(outside);
    }
}
