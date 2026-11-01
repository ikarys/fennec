//! Deleting sessions on the human's request (contract: docs/api.md): the log, the child sessions and the worktree with its branch. The usage ledger keeps the calls. The session's rules go with the session.

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::project::SessionSummary;
use crate::worktree;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

/// `POST /api/sessions/delete`: the human's deletion of top-level sessions.
pub fn delete_sessions(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let Some(items) = body.get("ids").and_then(Value::as_array) else {
        return error(stream, 400, "ids must be a non-empty list of session ids");
    };
    if items.is_empty() || items.len() > 500 {
        return error(stream, 400, "ids must be a non-empty list of session ids");
    }
    let ids: Option<Vec<String>> = items
        .iter()
        .map(|item| item.as_str().map(String::from))
        .collect();
    let Some(ids) = ids else {
        return error(stream, 400, "ids must be a non-empty list of session ids");
    };
    let store = app.store()?;
    let mut plans: Vec<(String, SessionSummary, Vec<String>)> = Vec::new();
    for id in &ids {
        let Some(session) = store.session(id)? else {
            return error(stream, 404, &format!("no such session: {id}"));
        };
        if session.parent_id.is_some() {
            return error(stream, 400, "only top-level sessions can be deleted");
        }
        let tree = store.session_tree(id)?;
        plans.push((id.clone(), session, tree));
    }
    let mut running: Vec<String> = Vec::new();
    for (id, _, tree) in &plans {
        if tree.iter().any(|sid| app.turns.is_running(sid)) && !running.contains(id) {
            running.push(id.clone());
        }
    }
    if !running.is_empty() {
        return send_json(
            stream,
            409,
            &json!({ "error": "a turn is running", "running": running }),
        );
    }
    let mut deleted: Vec<String> = Vec::new();
    let mut failed: Vec<Value> = Vec::new();
    let rule_store =
        rules::RuleStore::open(&app.settings.rules_path()).map_err(|e| e.to_string())?;
    for (id, session, tree) in plans {
        // A child session shares its root's worktree: removing the root's removes theirs.
        if let Some(project) = store.project(&session.project_id)?
            && let Some(wt) = app.place(&store, &session, &project)?.worktree
        {
            let Some((top, _)) = worktree::top(Path::new(&project.workdir)) else {
                failed.push(json!({ "id": id, "error": "the project is not in a git repository" }));
                continue;
            };
            if let Err(e) = worktree::remove(&top, &wt) {
                failed.push(json!({ "id": id, "error": e }));
                continue;
            }
        }
        store.delete_sessions(&tree)?;
        for sid in &tree {
            rule_store.delete_session(sid).map_err(|e| e.to_string())?;
        }
        let mut folds = app.skill_folds.lock().unwrap();
        for sid in &tree {
            folds.remove(sid);
        }
        deleted.push(id);
    }
    send_json(
        stream,
        200,
        &json!({ "deleted": deleted, "failed": failed }),
    )
}
