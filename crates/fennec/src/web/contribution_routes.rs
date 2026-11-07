//! Incoming contributions: a branch someone else wrote, adopted into a session
//! worktree so it goes through the same integration gate.

use crate::web::http::HttpError;
use std::net::TcpStream;
use std::path::Path;

use serde_json::json;

use super::App;
use super::http::{Request, error, send_json};

/// `GET /api/projects/{id}/contributions`.
pub fn list(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let Some((top, _)) = crate::worktree::top(Path::new(&project.workdir)) else {
        return send_json(stream, 200, &json!([]));
    };
    let contributions = crate::git::contributions(&top);
    let view = contributions
        .into_iter()
        .map(|c| json!({ "branch": c.branch, "ahead": c.ahead, "subject": c.subject }))
        .collect::<Vec<_>>();
    send_json(stream, 200, &json!(view))
}

/// `POST /api/projects/{id}/contributions` with `{"branch": string}`.
pub fn adopt(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let body = req.json()?;
    let Some(branch) = body.get("branch").and_then(|value| value.as_str()) else {
        return error(stream, 400, "branch is missing");
    };
    let branch = branch.to_string();
    let listed = crate::worktree::top(Path::new(&project.workdir))
        .map(|(top, _)| crate::git::contributions(&top))
        .unwrap_or_default();
    if !listed.iter().any(|c| c.branch == branch) {
        return error(stream, 400, "branch is not a listed contribution");
    }
    let sid = store.create_session(&project, &format!("Contribution: {branch}"))?;
    if let Err(message) =
        crate::worktree::adopt(&app.worktrees(), Path::new(&project.workdir), &sid, &branch)
    {
        let _ = store.delete_sessions(&[sid]);
        return error(stream, 500, &message);
    }
    send_json(stream, 200, &json!({ "id": sid }))
}
