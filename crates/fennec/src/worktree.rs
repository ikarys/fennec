//! A session's own git worktree.
//!
//! An agent run used to write straight into the human's working tree and
//! erased uncommitted human work. A session that runs a workflow now works in
//! its own worktree, on its own branch: the human's tree and branch are never
//! written, and the human later cherry-picks the agent's commits.

use std::path::{Path, PathBuf};

use crate::git;

pub mod usage;

/// A session's worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    /// The worktree's top directory: `<root>/<session_id>`.
    pub path: PathBuf,
    /// Where the session's tools run: `path` joined with the project's
    /// prefix inside its repository (empty when the project is the repo's top).
    pub workdir: PathBuf,
    /// `fennec/<session_id>`.
    pub branch: String,
    /// `refs/fennec/base/<session_id>`: the commit the agent's work starts from
    /// (after the snapshot of the human's uncommitted work), so
    /// `git cherry-pick <base>..<branch>` replays only the agent's commits.
    pub base: String,
    /// `refs/fennec/origin/<session_id>`: a symbolic ref to the branch the
    /// human had checked out when the worktree was made, so the gate can tell
    /// a branch switch.
    pub origin: String,
}

/// The directory holding every worktree: `worktrees/` next to the database file.
pub fn root(db_path: &Path) -> PathBuf {
    db_path.parent().unwrap_or(Path::new(".")).join("worktrees")
}

/// The session's worktree if it was already created; `None` otherwise, or when
/// `project_dir` is not inside a git repository. Never creates anything.
pub fn find(root: &Path, project_dir: &Path, session_id: &str) -> Option<Worktree> {
    let (_, prefix) = repository(project_dir)?;
    let path = root.join(session_id);
    // A worktree's top holds a `.git` file; a bare folder left at its path (a
    // file tool writing to a discarded worktree's old path) is not one.
    path.join(".git")
        .is_file()
        .then(|| build(path, &prefix, session_id))
}

/// The session's worktree, created on first call; `Ok(None)` when `project_dir`
/// is not inside a git repository (the caller then runs in place).
pub fn open(root: &Path, project_dir: &Path, session_id: &str) -> Result<Option<Worktree>, String> {
    let Some((top, prefix)) = repository(project_dir) else {
        return Ok(None);
    };
    if let Some(existing) = find(root, project_dir, session_id) {
        return Ok(Some(existing));
    }
    std::fs::create_dir_all(root).map_err(|e| format!("create {}: {e}", root.display()))?;
    let path = root.join(session_id);
    let branch = format!("fennec/{session_id}");
    let path_arg = path.to_string_lossy().into_owned();
    git::run(
        &top,
        &["worktree", "add", "-b", &branch, &path_arg, "HEAD"],
        None,
    )?;
    let wt = build(path, &prefix, session_id);
    // A half-built worktree must not survive: `find` would hand it out.
    fill(&top, &wt).inspect_err(|_| {
        let _ = git::run(&top, &["worktree", "remove", "--force", &path_arg], None);
        let _ = git::run(&top, &["branch", "-D", &branch], None);
    })?;
    Ok(Some(wt))
}

/// Adopts a branch someone else wrote into a session worktree, so it goes
/// through the same integration gate as an agent's own work.
pub fn adopt(
    root: &Path,
    project_dir: &Path,
    session_id: &str,
    source: &str,
) -> Result<Worktree, String> {
    let (top, prefix) =
        repository(project_dir).ok_or_else(|| "not a git repository".to_string())?;
    if find(root, project_dir, session_id).is_some() {
        return Err("worktree already exists".to_string());
    }
    if source.is_empty() || source.starts_with('-') {
        return Err("source must not be empty or start with '-'".to_string());
    }
    let verify = format!("{source}^{{commit}}");
    if git::run(&top, &["rev-parse", "--verify", "--quiet", &verify], None).is_err() {
        return Err(format!("no such branch: {source}"));
    }
    std::fs::create_dir_all(root).map_err(|e| format!("create {}: {e}", root.display()))?;
    let path = root.join(session_id);
    let branch = format!("fennec/{session_id}");
    let path_arg = path.to_string_lossy().into_owned();
    git::run(
        &top,
        &[
            "worktree",
            "add",
            "--no-track",
            "-b",
            &branch,
            &path_arg,
            source,
        ],
        None,
    )?;
    let wt = build(path, &prefix, session_id);
    // A half-built worktree must not survive: `find` would hand it out.
    (|| -> Result<(), String> {
        let base = git::run(&top, &["merge-base", "HEAD", source], None)?;
        let base = String::from_utf8_lossy(&base).trim().to_string();
        git::run(&wt.path, &["update-ref", &wt.base, &base], None)?;
        record_origin(&top, &wt)
    })()
    .inspect_err(|_| {
        let _ = git::run(&top, &["worktree", "remove", "--force", &path_arg], None);
        let _ = git::run(&top, &["branch", "-D", &branch], None);
    })?;
    Ok(wt)
}

/// Removes the session's worktree, its branch, and its base and origin refs:
/// the human's discard. The agent's commits stay in the object store only
/// until git gc, which is why the route records their shas.
pub fn remove(top: &Path, wt: &Worktree) -> Result<(), String> {
    let path_arg = wt.path.to_string_lossy().into_owned();
    crate::tools::remove_private_tmp(&wt.path);
    let removed = git::run(top, &["worktree", "remove", "--force", &path_arg], None);
    if let Err(e) = &removed
        && !e.contains("Permission denied")
    {
        return Err(e.clone());
    }
    // A directory git could not empty (files a container wrote as root through a bind mount)
    // is no longer a worktree: drop its registration, branch and refs all the same.
    let _ = git::run(top, &["worktree", "prune"], None);
    git::run(top, &["branch", "-D", &wt.branch], None)?;
    git::run(top, &["update-ref", "-d", &wt.base], None)?;
    // Older worktrees have no origin ref; deleting it is optional.
    let _ = git::run(top, &["symbolic-ref", "--delete", &wt.origin], None);
    if let Err(e) = removed {
        return Err(format!(
            "{e}\nThe branch is removed; some files are not yours (owned by root?): `sudo rm -rf {path_arg}`."
        ));
    }
    Ok(())
}

/// The branch the human had checked out when the worktree was made; `None`
/// for a detached HEAD or worktrees made before this ref existed.
pub fn origin_branch(top: &Path, wt: &Worktree) -> Option<String> {
    let out = git::run(top, &["symbolic-ref", "--short", &wt.origin], None).ok()?;
    let branch = String::from_utf8_lossy(&out).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

/// Copies the human's uncommitted work into the fresh worktree and records
/// the base ref and the human's checked-out branch, if it is on one.
fn fill(top: &Path, wt: &Worktree) -> Result<(), String> {
    // The human's uncommitted work is copied, never moved: their tree stays as is.
    let mut copied = false;
    let diff = git::run(top, &["diff", "HEAD", "--binary"], None)?;
    if !diff.is_empty() {
        git::run(&wt.path, &["apply", "--binary"], Some(&diff))?;
        copied = true;
    }
    let untracked = git::run(
        top,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        None,
    )?;
    for rel in untracked.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let rel = String::from_utf8_lossy(rel).into_owned();
        let target = wt.path.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        std::fs::copy(top.join(&rel), &target).map_err(|e| format!("copy {rel}: {e}"))?;
        copied = true;
    }
    if copied {
        commit(
            wt,
            "fennec: the human's uncommitted work at the run's start",
        )?;
    }
    git::run(&wt.path, &["update-ref", &wt.base, "HEAD"], None)?;
    record_origin(top, wt)
}

/// Records the human's checked-out branch as the worktree's origin ref, if
/// the human is on a branch.
fn record_origin(top: &Path, wt: &Worktree) -> Result<(), String> {
    if let Ok(head) = git::run(top, &["symbolic-ref", "-q", "HEAD"], None) {
        let head_ref = String::from_utf8_lossy(&head).trim().to_string();
        if !head_ref.is_empty() {
            git::run(top, &["symbolic-ref", &wt.origin, &head_ref], None)?;
        }
    }
    Ok(())
}

/// What the model looked at or used as input, not work to integrate: the
/// browser's captures, the files an idea pinned, and the dependency links
/// `link_dependencies` makes. The human would cherry-pick them otherwise
/// (`.gitignore`'s `node_modules/` does not match a symlink, which git sees
/// as a file).
const EXCLUDED_INPUTS: &[&str] = &[
    ":(exclude,glob)**/.playwright-cli/**",
    ":(exclude,glob)**/.fennec/**",
    ":(exclude,glob)**/node_modules",
    ":(exclude,glob)**/.venv",
];

/// Commits everything in the worktree (`git add -A`) but the model's inputs,
/// under the human's git identity, with `message`; `Ok(false)` when there was
/// nothing to commit.
pub fn commit(worktree: &Worktree, message: &str) -> Result<bool, String> {
    let dir = &worktree.path;
    // The captures, the idea's attached files, and the dependency links are
    // what the agent consumed, not its work: the human would cherry-pick them
    // otherwise.
    let mut args: Vec<&str> = vec!["add", "-A", "--", "."];
    args.extend_from_slice(EXCLUDED_INPUTS);
    git::run(dir, &args, None)?;
    // The staged changes, not `git status`: the excluded captures stay untracked.
    if git::run(dir, &["diff", "--cached", "--name-only"], None)?.is_empty() {
        return Ok(false);
    }
    // The human's git identity: fennec is the harness, not an author.
    // Provenance stays in the `fennec/<session>` branch and the integration's
    // `Fennec-Session` trailer.
    git::run(dir, &["commit", "-q", "-m", message], None)?;
    Ok(true)
}

fn build(path: PathBuf, prefix: &str, session_id: &str) -> Worktree {
    Worktree {
        workdir: path.join(prefix),
        path,
        branch: format!("fennec/{session_id}"),
        base: format!("refs/fennec/base/{session_id}"),
        origin: format!("refs/fennec/origin/{session_id}"),
    }
}

/// The repository's top directory and the project's prefix inside it.
/// `integration.rs` reuses it instead of duplicating the lookup.
pub fn top(project_dir: &Path) -> Option<(PathBuf, String)> {
    repository(project_dir)
}

/// Read-only binds for dependency directories the worktree lacks.
///
/// A linked worktree does not contain gitignored dependency directories such
/// as `node_modules` or `.venv`, and the sandbox has no network, so checks
/// that need them must see the main checkout's copies read-only.
pub fn dependency_mounts(workdir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut mounts = Vec::new();
    if let Some(main_dir) = main_checkout(workdir) {
        collect_dependency_mounts(&main_dir, workdir, 0, &mut mounts);
    }
    mounts
}

/// One shared Cargo target directory for every worktree of a project.
///
/// Worktrees are many and short-lived, while a target directory is large;
/// sharing it per project avoids one target per session.
pub fn shared_target(root: &Path, workdir: &Path) -> Option<PathBuf> {
    // ponytail: sessions of one project share build outputs, so one session's build can affect another's; per-session overlays if that matters.
    let (top, prefix) = repository(workdir)?;
    if !top.join("Cargo.toml").is_file() {
        return None;
    }
    let main = main_checkout(workdir)?;
    // `main` is the main checkout's copy of `workdir`; dropping the worktree's
    // prefix leaves the main checkout's repository top, the project key.
    let main_top = main
        .ancestors()
        .nth(Path::new(&prefix).components().count())?;
    let key = main_top
        .to_string_lossy()
        .trim_start_matches('/')
        .replace('/', "-");
    let dir = root.join(".cargo-target").join(key);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Links the main checkout's dependency directories into the worktree.
///
/// A server runs on the host, not in bwrap, so it cannot get the sandbox's
/// read-only binds; a symlink to the main checkout's copy gives it the same
/// dependencies (shared, not copied: a worktree stays light).
pub fn link_dependencies(workdir: &Path) -> Result<(), String> {
    for (source, target) in dependency_mounts(workdir) {
        match std::fs::symlink_metadata(&target) {
            // An empty directory is the mount point bwrap left behind for the
            // sandbox's bind: replace it. A filled one is a real install: keep it.
            Ok(meta) if meta.is_dir() => {
                if std::fs::remove_dir(&target).is_err() {
                    continue;
                }
            }
            Ok(_) => continue,
            Err(_) => {}
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        std::os::unix::fs::symlink(&source, &target)
            .map_err(|e| format!("link {}: {e}", target.display()))?;
    }
    Ok(())
}

/// The main checkout's directory matching `workdir` when `workdir` is in a
/// linked worktree; `None` in the main checkout or outside a repository.
fn main_checkout(workdir: &Path) -> Option<PathBuf> {
    let (_, prefix) = repository(workdir)?;
    // `--path-format` applies to the options after it.
    let out = git::run(
        workdir,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
        ],
        None,
    )
    .ok()?;
    let text = String::from_utf8_lossy(&out);
    let mut lines = text.lines();
    let (git_dir, common_dir) = (lines.next()?, lines.next()?);
    if git_dir == common_dir {
        return None;
    }
    Some(Path::new(common_dir).parent()?.join(prefix))
}

fn collect_dependency_mounts(
    main_dir: &Path,
    workdir: &Path,
    depth: u32,
    mounts: &mut Vec<(PathBuf, PathBuf)>,
) {
    if depth >= 4 {
        return;
    }
    let entries = match std::fs::read_dir(main_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = workdir.join(&name);
        if name == "node_modules" || name == ".venv" {
            // A symlink already reaches the main checkout's copy, which the
            // sandbox sees read-only through its `/` bind; binding onto a
            // symlink is not wanted.
            if target.symlink_metadata().is_ok_and(|m| m.is_symlink()) {
                continue;
            }
            mounts.push((path, target));
        } else if name != "target" && !name.starts_with('.') {
            collect_dependency_mounts(&path, &target, depth + 1, mounts);
        }
    }
}

/// The repository's top directory and the project's prefix inside it.
fn repository(project_dir: &Path) -> Option<(PathBuf, String)> {
    let text = |args: &[&str]| {
        let out = git::run(project_dir, args, None).ok()?;
        Some(String::from_utf8_lossy(&out).trim().to_string())
    };
    let top = text(&["rev-parse", "--show-toplevel"])?;
    let prefix = text(&["rev-parse", "--show-prefix"])?;
    Some((PathBuf::from(top), prefix))
}

/// A worktree lacks the gitignored `.env` files a dev server reads; a link,
/// never a copy, so the secret stays in one place; only ignored files, so the
/// link is never committed.
pub fn link_env_files(workdir: &Path) -> Result<(), String> {
    let Some(main) = main_checkout(workdir) else {
        return Ok(());
    };

    let entries = std::fs::read_dir(&main).map_err(|e| format!("read {main:?}: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };

        if !name.starts_with(".env") {
            continue;
        }

        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        if git::run(&main, &["check-ignore", "-q", name], None).is_err() {
            continue;
        }

        let target = workdir.join(name);
        if std::fs::symlink_metadata(&target).is_ok() {
            continue;
        }

        std::os::unix::fs::symlink(&path, &target).map_err(|e| format!("link {name}: {e}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;

    fn git(dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
        git::run(dir, args, stdin)
    }

    fn text(dir: &Path, args: &[&str]) -> String {
        String::from_utf8_lossy(&git(dir, args, None).unwrap()).into_owned()
    }

    /// A temp directory with a repository inside (`repo/`), one commit.
    fn setup() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("fennec-worktree-{}", new_id()));
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
    fn adopt_brings_a_contribution_branch_into_a_session_worktree() {
        let (base, repo) = setup();
        let initial = text(&repo, &["symbolic-ref", "--short", "HEAD"])
            .trim()
            .to_string();
        git(&repo, &["checkout", "-q", "-b", "contrib"], None).unwrap();
        std::fs::write(repo.join("c.txt"), "contrib\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "contrib"], None).unwrap();
        git(&repo, &["checkout", "-q", initial.as_str()], None).unwrap();
        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "main"], None).unwrap();

        let sid = new_id();
        let wt = adopt(&base, &repo, &sid, "contrib").unwrap();
        assert!(wt.path.join("c.txt").is_file());
        let count = text(
            &repo,
            &[
                "rev-list",
                "--count",
                &format!("{}..{}", wt.base, wt.branch),
            ],
        );
        assert_eq!(count.trim(), "1");
        assert_eq!(
            text(&repo, &["symbolic-ref", "--short", "HEAD"]).trim(),
            initial
        );
    }

    #[test]
    fn adopt_rejects_option_injection_and_missing_branches() {
        let (base, repo) = setup();
        let sid = new_id();
        assert!(adopt(&base, &repo, &sid, "-x").is_err());
        assert!(adopt(&base, &repo, &sid, "nope").is_err());
    }

    #[test]
    fn open_copies_uncommitted_work_and_leaves_the_human_tree_alone() {
        let (base, repo) = setup();
        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        std::fs::create_dir_all(repo.join("new")).unwrap();
        std::fs::write(repo.join("new/b.txt"), "b\n").unwrap();
        let status = text(&repo, &["status", "--porcelain"]);
        let head = text(&repo, &["rev-parse", "HEAD"]);

        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();

        assert_eq!(
            std::fs::read_to_string(wt.path.join("a.txt")).unwrap(),
            "two\n"
        );
        assert_eq!(
            std::fs::read_to_string(wt.path.join("new/b.txt")).unwrap(),
            "b\n"
        );
        assert!(text(&repo, &["branch", "--list", "fennec/s1"]).contains("fennec/s1"));
        assert_eq!(
            text(&repo, &["rev-parse", &wt.base]),
            text(&wt.path, &["rev-parse", "HEAD"])
        );
        assert_eq!(text(&repo, &["status", "--porcelain"]), status);
        assert!(status.contains(" M a.txt") && status.contains("?? new/"));
        assert_eq!(text(&repo, &["rev-parse", "HEAD"]), head);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn open_is_idempotent_and_find_sees_it() {
        let (base, repo) = setup();
        let root = base.join("wt");
        assert_eq!(find(&root, &repo, "s1"), None);
        let first = open(&root, &repo, "s1").unwrap().unwrap();
        let second = open(&root, &repo, "s1").unwrap().unwrap();
        assert_eq!(first, second);
        assert_eq!(find(&root, &repo, "s1"), Some(first));
        std::fs::create_dir_all(root.join("s2/web")).unwrap();
        assert_eq!(find(&root, &repo, "s2"), None);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn remove_deletes_the_worktree_branch_and_base() {
        let (base, repo) = setup();
        let root = base.join("wt");
        let status = text(&repo, &["status", "--porcelain"]);
        let head = text(&repo, &["rev-parse", "HEAD"]);

        let wt = open(&root, &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("agent.txt"), "x\n").unwrap();
        assert_eq!(commit(&wt, "agent work"), Ok(true));

        assert_eq!(remove(&repo, &wt), Ok(()));

        assert_eq!(find(&root, &repo, "s1"), None);
        assert!(text(&repo, &["branch", "--list", "fennec/s1"]).is_empty());
        assert!(git(&repo, &["rev-parse", "-q", "--verify", &wt.base], None).is_err());
        assert_eq!(text(&repo, &["status", "--porcelain"]), status);
        assert_eq!(text(&repo, &["rev-parse", "HEAD"]), head);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn commit_records_the_agents_work_as_the_human() {
        let (base, repo) = setup();
        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        std::fs::write(wt.workdir.join("agent.txt"), "x\n").unwrap();

        assert_eq!(commit(&wt, "agent work"), Ok(true));
        assert_eq!(
            text(&wt.path, &["log", "-1", "--format=%an"]).trim(),
            "human"
        );
        assert_eq!(commit(&wt, "again"), Ok(false));
        std::fs::create_dir_all(wt.workdir.join(".playwright-cli")).unwrap();
        std::fs::write(wt.workdir.join(".playwright-cli/page.png"), "png").unwrap();
        assert_eq!(commit(&wt, "captures only"), Ok(false));
        std::fs::create_dir_all(wt.workdir.join(".fennec/assets")).unwrap();
        std::fs::write(wt.workdir.join(".fennec/assets/design.zip"), "zip").unwrap();
        assert_eq!(commit(&wt, "idea assets only"), Ok(false));
        let range = format!("{}..{}", wt.base, wt.branch);
        assert_eq!(
            text(&wt.path, &["log", "--format=%s", &range]).trim(),
            "agent work"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_project_in_a_subdirectory_maps_to_the_same_subdirectory() {
        let (base, repo) = setup();
        std::fs::create_dir_all(repo.join("sub")).unwrap();
        std::fs::write(repo.join("sub/c.txt"), "c\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "sub"], None).unwrap();

        let wt = open(&base.join("wt"), &repo.join("sub"), "s1")
            .unwrap()
            .unwrap();

        assert!(wt.workdir.ends_with("sub"));
        assert!(wt.workdir.join("c.txt").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_failed_open_leaves_no_worktree_behind() {
        use std::os::unix::fs::PermissionsExt;
        let (base, repo) = setup();
        let secret = repo.join("secret.txt");
        std::fs::write(&secret, "s\n").unwrap();
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&secret).is_ok() {
            // Running as root: nothing is unreadable, the copy cannot fail.
            let _ = std::fs::remove_dir_all(&base);
            return;
        }
        let root = base.join("wt");

        assert!(open(&root, &repo, "s1").is_err());

        assert_eq!(find(&root, &repo, "s1"), None);
        assert!(text(&repo, &["branch", "--list", "fennec/s1"]).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn outside_a_repository_runs_in_place() {
        let base = std::env::temp_dir().join(format!("fennec-worktree-{}", new_id()));
        std::fs::create_dir_all(&base).unwrap();
        let root = base.join("wt");
        assert_eq!(open(&root, &base, "s1"), Ok(None));
        assert_eq!(find(&root, &base, "s1"), None);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dependency_mounts_maps_ignored_dependencies_from_the_main_checkout() {
        let (base, repo) = setup();
        std::fs::create_dir_all(repo.join("web/node_modules")).unwrap();
        std::fs::write(repo.join("web/node_modules/package.json"), "{}").unwrap();
        std::fs::write(repo.join(".gitignore"), "node_modules\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "ignore dependencies"], None).unwrap();

        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();

        assert_eq!(
            dependency_mounts(&wt.workdir),
            vec![(
                repo.join("web/node_modules"),
                wt.path.join("web/node_modules")
            )]
        );
        assert!(dependency_mounts(&repo).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn shared_target_shares_one_target_per_project_for_worktrees() {
        let (base, repo) = setup();
        std::fs::write(repo.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "cargo"], None).unwrap();

        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();

        let dir = shared_target(&base, &wt.workdir).unwrap();
        assert!(dir.starts_with(base.join(".cargo-target")));
        assert!(dir.is_dir());
        assert!(shared_target(&base, &repo).is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn link_dependencies_links_main_checkout_dependencies_and_excludes_them() {
        let (base, repo) = setup();
        std::fs::create_dir_all(repo.join("web/node_modules")).unwrap();
        std::fs::write(repo.join("web/node_modules/package.json"), "{}").unwrap();
        std::fs::write(repo.join(".gitignore"), "node_modules\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "ignore dependencies"], None).unwrap();

        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        let link = wt.path.join("web/node_modules");

        // The empty mount point a sandboxed command leaves is replaced.
        std::fs::create_dir_all(&link).unwrap();
        assert_eq!(link_dependencies(&wt.workdir), Ok(()));
        assert!(link.symlink_metadata().unwrap().is_symlink());
        assert_eq!(
            std::fs::read_link(&link).unwrap(),
            repo.join("web/node_modules")
        );
        assert_eq!(link_dependencies(&wt.workdir), Ok(()));
        assert!(dependency_mounts(&wt.workdir).is_empty());
        assert_eq!(commit(&wt, "nothing"), Ok(false));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn link_env_files_links_only_ignored_env_files() {
        let (base, repo) = setup();

        std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
        std::fs::write(repo.join(".env.example"), "EXAMPLE=1\n").unwrap();
        git(&repo, &["add", "-A"], None).unwrap();
        git(&repo, &["commit", "-q", "-m", "x"], None).unwrap();
        std::fs::write(repo.join(".env"), "SECRET=1\n").unwrap();

        let wt = open(&base.join("wt"), &repo, "s1").unwrap().unwrap();
        link_env_files(&wt.workdir).unwrap();

        let env_link = wt.workdir.join(".env");
        let meta = std::fs::symlink_metadata(&env_link).unwrap();
        assert!(meta.is_symlink());
        assert_eq!(std::fs::read_link(&env_link).unwrap(), repo.join(".env"));

        let example = wt.workdir.join(".env.example");
        assert!(std::fs::metadata(&example).unwrap().is_file());
        assert!(!std::fs::symlink_metadata(&example).unwrap().is_symlink());

        link_env_files(&wt.workdir).unwrap();
        assert_eq!(commit(&wt, "nothing"), Ok(false));

        let _ = std::fs::remove_dir_all(&base);
    }
}
