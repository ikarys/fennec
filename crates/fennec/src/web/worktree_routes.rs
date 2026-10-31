//! The worktrees' disk usage (contract: docs/api.md): measured on demand, freed on the human's click.
//! A scan walks gigabytes: the home page's total reuses one for ten minutes; the project tab and
//! the human's actions measure again.

use std::collections::HashMap;
use std::net::TcpStream;
use std::path::Path;

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::clock;
use crate::domain::project::SessionSummary;
use crate::worktree::usage;
use crate::worktree::usage::WorktreeUsage;
use serde_json::{Value, json};

const CACHE_MS: u64 = 10 * 60 * 1000;
pub(super) const ALERT_BYTES: u64 = 5 * 1024 * 1024 * 1024;

pub fn summary(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let (_, entries) = cached_scan(app, false);
    let total_bytes = entries.iter().map(|entry| entry.bytes).sum::<u64>();
    let generated_bytes = entries
        .iter()
        .map(|entry| entry.generated_bytes)
        .sum::<u64>();
    let largest_project_id = largest_project_id(app, &entries)?;

    send_json(
        stream,
        200,
        &json!({
            "totalBytes": total_bytes,
            "generatedBytes": generated_bytes,
            "alert": total_bytes >= ALERT_BYTES,
            "largestProjectId": largest_project_id,
        }),
    )
}

pub fn list(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };

    let fresh = req.query_param("fresh").is_some_and(|value| value == "1");
    let (scanned_at, entries) = cached_scan(app, fresh);
    let project_workdir = Path::new(&project.workdir);

    let project_entries = project_entries(&store, &entries, id, project_workdir)?;

    let mut entry_values = Vec::new();
    let mut total_bytes = 0u64;
    let mut generated_bytes = 0u64;

    for item in project_entries {
        let session = item
            .session
            .as_ref()
            .map(|s| (s.project_id.as_str(), s.title.as_str(), s.updated_at));
        let running = item.session.is_some() && running(app, &store, &item.entry.name)?;

        entry_values.push(entry_json(item.entry, session, running));
        total_bytes += item.entry.bytes;
        generated_bytes += item.entry.generated_bytes;
    }

    send_json(
        stream,
        200,
        &json!({
            "scannedAt": scanned_at,
            "totalBytes": total_bytes,
            "generatedBytes": generated_bytes,
            "entries": entry_values,
        }),
    )
}

pub fn clean(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let Some(name) = body_name(req) else {
        return error(stream, 400, "invalid worktree name");
    };
    if !usage::valid_name(&name) {
        return error(stream, 400, "invalid worktree name");
    }

    let root = app.worktrees();
    let path = root.join(&name);
    if !path.is_dir() {
        return error(stream, 404, "no such worktree");
    }

    let store = app.store()?;
    if store.session(&name)?.is_some() && running(app, &store, &name)? {
        return error(stream, 409, "a turn is running");
    }

    match usage::clean_generated(&path) {
        Ok(freed_bytes) => {
            drop_scan_cache(app);
            send_json(stream, 200, &json!({ "freedBytes": freed_bytes }))
        }
        Err(message) => error(stream, 409, &message),
    }
}

pub fn remove_orphan(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let Some(name) = body_name(req) else {
        return error(stream, 400, "invalid worktree name");
    };
    if !usage::valid_name(&name) {
        return error(stream, 400, "invalid worktree name");
    }

    let root = app.worktrees();
    let path = root.join(&name);
    if !path.is_dir() {
        return error(stream, 404, "no such worktree");
    }

    let store = app.store()?;
    if store.session(&name)?.is_some() && usage::repository(&path).1 {
        return error(
            stream,
            409,
            "not an orphan: discard the session's work instead",
        );
    }

    match usage::remove_orphan(&root, &name) {
        Ok(()) => {
            drop_scan_cache(app);
            send_json(stream, 200, &json!({}))
        }
        Err(message) => error(stream, 409, &message),
    }
}

/// The project's disk usage from the cached scan only: the dashboard must not
/// trigger a scan.
pub(super) fn project_disk(
    app: &App,
    store: &crate::store::SqliteStore,
    project: &crate::domain::project::Project,
) -> Result<Option<(u64, u64)>, String> {
    // Cloned out so the store reads below do not hold the scan's lock.
    let Some((_, entries)) = app.worktree_scan.lock().unwrap().clone() else {
        return Ok(None);
    };

    let project_workdir = Path::new(&project.workdir);
    let project_entries = project_entries(store, &entries, &project.id, project_workdir)?;

    let total_bytes = project_entries
        .iter()
        .map(|item| item.entry.bytes)
        .sum::<u64>();
    let generated_bytes = project_entries
        .iter()
        .map(|item| item.entry.generated_bytes)
        .sum::<u64>();

    Ok(Some((total_bytes, generated_bytes)))
}

fn cached_scan(app: &App, fresh: bool) -> (u64, Vec<WorktreeUsage>) {
    let now = clock::now_ms();
    let mut scan = app.worktree_scan.lock().unwrap();

    if !fresh
        && let Some((scanned_at, entries)) = scan.as_ref()
        && now.saturating_sub(*scanned_at) < CACHE_MS
    {
        return (*scanned_at, entries.clone());
    }

    // ponytail: holding the lock during the ~1 s scan is simpler than a scan-in-progress token.
    let entries = usage::scan(&app.worktrees());
    let scanned_at = clock::now_ms();
    *scan = Some((scanned_at, entries.clone()));
    (scanned_at, entries)
}

fn drop_scan_cache(app: &App) {
    *app.worktree_scan.lock().unwrap() = None;
}

fn body_name(req: &Request) -> Option<String> {
    Some(req.json().ok()?.get("name")?.as_str()?.to_string())
}

fn largest_project_id(app: &App, entries: &[WorktreeUsage]) -> Result<Option<String>, String> {
    let store = app.store()?;
    let projects = store.projects()?;

    let mut sums: HashMap<String, u64> = HashMap::new();
    for entry in entries {
        if let Some(session) = store.session(&entry.name)? {
            *sums.entry(session.project_id).or_insert(0) += entry.bytes;
        }
    }

    Ok(sums
        .into_iter()
        .filter(|(project_id, _)| projects.iter().any(|project| project.id == *project_id))
        .max_by_key(|(_, bytes)| *bytes)
        .map(|(project_id, _)| project_id))
}

/// A scanned worktree of the project, with its session when the store knows it.
struct ProjectEntry<'a> {
    entry: &'a WorktreeUsage,
    session: Option<SessionSummary>,
}

/// The scan's entries that belong to the project: one filter for the tab and the dashboard.
fn project_entries<'a>(
    store: &crate::store::SqliteStore,
    entries: &'a [WorktreeUsage],
    project_id: &str,
    project_workdir: &Path,
) -> Result<Vec<ProjectEntry<'a>>, String> {
    let mut out = Vec::new();

    for entry in entries {
        let session = store.session(&entry.name)?;
        let session_project = session.as_ref().map(|s| s.project_id.as_str());

        if belongs(entry, session_project, project_id, project_workdir) {
            out.push(ProjectEntry { entry, session });
        }
    }

    Ok(out)
}

fn belongs(
    entry: &WorktreeUsage,
    session_project: Option<&str>,
    project_id: &str,
    project_workdir: &Path,
) -> bool {
    match session_project {
        Some(project) => project == project_id,
        // An orphan whose repository is gone has no project left: every page shows it.
        None => {
            repo_missing(entry)
                || entry
                    .repo_top
                    .as_ref()
                    .is_some_and(|repo_top| project_workdir.starts_with(repo_top))
        }
    }
}

/// A turn of the session or of one of its child sessions runs.
fn running(app: &App, store: &crate::store::SqliteStore, session_id: &str) -> Result<bool, String> {
    Ok(store
        .session_tree(session_id)?
        .iter()
        .any(|id| app.turns.is_running(id)))
}

fn repo_missing(entry: &WorktreeUsage) -> bool {
    match entry.repo_top.as_deref() {
        None => true,
        Some(repo_top) => !repo_top.is_dir(),
    }
}

fn entry_json(entry: &WorktreeUsage, session: Option<(&str, &str, u64)>, running: bool) -> Value {
    let orphan = session.is_none() || !entry.registered;
    let repo_missing = repo_missing(entry);
    let sudo_command = if orphan && entry.foreign_owned {
        Some(format!("sudo rm -rf {}", entry.path.display()))
    } else {
        None
    };

    json!({
        "name": entry.name,
        "sessionId": session.map(|_| entry.name.as_str()),
        "sessionTitle": session.map(|session| session.1),
        "running": running,
        "updatedAt": session.map(|session| session.2),
        "bytes": entry.bytes,
        "generatedBytes": entry.generated_bytes,
        "orphan": orphan,
        "repoMissing": repo_missing,
        "foreignOwned": entry.foreign_owned,
        "sudoCommand": sudo_command,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(name: &str, repo_top: Option<PathBuf>) -> WorktreeUsage {
        WorktreeUsage {
            name: name.to_string(),
            path: std::env::temp_dir().join(name),
            bytes: 1,
            generated_bytes: 0,
            foreign_owned: false,
            repo_top,
            registered: true,
        }
    }

    #[test]
    fn session_of_this_project_belongs() {
        let entry = entry("session", None);
        assert!(belongs(
            &entry,
            Some("project"),
            "project",
            Path::new("/tmp/work")
        ));
    }

    #[test]
    fn session_of_another_project_does_not_belong() {
        let repo = std::env::temp_dir().join("fennec-worktree-routes-test-repo");
        std::fs::create_dir_all(&repo).unwrap();
        let entry = entry("session", Some(repo));
        assert!(!belongs(
            &entry,
            Some("other"),
            "project",
            Path::new("/tmp/work")
        ));
    }

    #[test]
    fn session_of_another_project_with_missing_repo_does_not_belong() {
        let entry = entry("session", None);
        assert!(!belongs(
            &entry,
            Some("other"),
            "project",
            Path::new("/tmp/work")
        ));
    }

    #[test]
    fn orphan_with_missing_repo_belongs_everywhere() {
        let entry = entry("session", None);
        assert!(belongs(&entry, None, "project", Path::new("/tmp/work")));
    }

    #[test]
    fn repo_prefix_belongs() {
        let repo = std::env::temp_dir();
        let workdir = repo.join("x");
        let entry = entry("session", Some(repo));
        assert!(belongs(&entry, None, "project", &workdir));
    }

    #[test]
    fn repo_not_prefix_does_not_belong() {
        let repo = std::env::temp_dir().join("fennec-worktree-routes-test-repo");
        std::fs::create_dir_all(&repo).unwrap();
        let entry = entry("session", Some(repo));
        assert!(!belongs(
            &entry,
            None,
            "project",
            Path::new("/tmp/other-work")
        ));
    }
}
