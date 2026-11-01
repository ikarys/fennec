//! Deleting a project on the human's request (contract: docs/api.md): its sessions, rules, ideas
//! backlog, and, on request, its source folder.

use super::App;
use super::http::{Request, error, send_json};
use super::session_delete;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

/// `POST /api/projects/{id}/delete`: the human's deletion of a project and,
/// on request, its source folder.
pub(super) fn delete_project(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let delete_folder = body
        .get("deleteFolder")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    if super::idea_queue::queue_running(app, id) {
        return send_json(
            stream,
            409,
            &json!({ "error": "the project's queue is running" }),
        );
    }
    let top_sessions = store.sessions(Some(id))?; // top-level only
    let mut running: Vec<String> = Vec::new();
    for session in &top_sessions {
        let tree = store.session_tree(&session.id)?;
        for sid in &tree {
            if app.turns.is_running(sid) && !running.contains(sid) {
                running.push(sid.clone());
            }
        }
    }
    if !running.is_empty() {
        return send_json(
            stream,
            409,
            &json!({ "error": "a turn is running", "running": running }),
        );
    }
    if delete_folder {
        let workdir = std::fs::canonicalize(&project.workdir).map_err(|e| e.to_string())?;
        let home = std::env::var_os("HOME").map(PathBuf::from);
        // Canonical like the workdir, so a symlinked store is still recognised.
        let store_dir = app.settings.db_path.parent().unwrap_or(Path::new("/"));
        let store_dir =
            std::fs::canonicalize(store_dir).unwrap_or_else(|_| store_dir.to_path_buf());
        if let Err(reason) = removable_folder(&workdir, home.as_deref(), &store_dir) {
            return error(stream, 400, &reason);
        }
    }
    let rule_store =
        rules::RuleStore::open(&app.settings.rules_path()).map_err(|e| e.to_string())?;
    for session in &top_sessions {
        let tree = store.session_tree(&session.id)?;
        match session_delete::remove_tree(app, &store, &rule_store, session, &tree) {
            Ok(None) => {}
            Ok(Some(e)) => {
                return send_json(
                    stream,
                    409,
                    &json!({
                        "error": format!("a worktree could not be removed: {e}"),
                        "sessionId": session.id
                    }),
                );
            }
            Err(e) => return Err(e),
        }
    }
    store.delete_project(id).map_err(|e| e.to_string())?;
    rule_store.delete_project(id).map_err(|e| e.to_string())?;
    let (folder_deleted, folder_error) = if delete_folder {
        match std::fs::remove_dir_all(&project.workdir) {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e.to_string())),
        }
    } else {
        (false, None)
    };
    send_json(
        stream,
        200,
        &json!({
            "deleted": true,
            "folderDeleted": folder_deleted,
            "folderError": folder_error
        }),
    )
}

/// Refuses a folder whose deletion would take fennec's own data or the
/// human's home with it. Pure: the caller passes canonicalized paths.
fn removable_folder(workdir: &Path, home: Option<&Path>, store_dir: &Path) -> Result<(), String> {
    if workdir.parent().is_none() {
        return Err("the folder is the filesystem root".into());
    }
    if let Some(home) = home
        && (workdir == home || home.starts_with(workdir))
    {
        return Err("the folder is the home directory or an ancestor of it".into());
    }
    if workdir == store_dir || store_dir.starts_with(workdir) {
        return Err("the folder contains fennec's store".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::removable_folder;
    use std::path::Path;

    #[test]
    fn refuses_the_filesystem_root() {
        assert!(
            removable_folder(
                Path::new("/"),
                Some(Path::new("/home/user")),
                Path::new("/home/user/.local/share/fennec")
            )
            .is_err()
        );
    }

    #[test]
    fn refuses_the_home_directory_and_its_ancestors() {
        let home = Path::new("/home/user");
        let store_dir = Path::new("/home/user/.local/share/fennec");
        assert!(removable_folder(Path::new("/home/user"), Some(home), store_dir).is_err());
        assert!(removable_folder(Path::new("/home"), Some(home), store_dir).is_err());
    }

    #[test]
    fn refuses_the_store_directory_and_its_ancestors() {
        let home = Path::new("/home/user");
        let store_dir = Path::new("/home/user/.local/share/fennec");
        assert!(removable_folder(store_dir, Some(home), store_dir).is_err());
        assert!(
            removable_folder(Path::new("/home/user/.local/share"), Some(home), store_dir).is_err()
        );
    }

    #[test]
    fn accepts_a_project_folder_inside_the_home() {
        let home = Path::new("/home/user");
        let store_dir = Path::new("/home/user/.local/share/fennec");
        assert!(
            removable_folder(Path::new("/home/user/projects/foo"), Some(home), store_dir).is_ok()
        );
    }
}
