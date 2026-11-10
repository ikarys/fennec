//! The project file explorer's routes (contract: `docs/api.md`): the tree, a file,
//! the changed tracked files, a file saved to disk, and one commit of the ticked
//! files. `session` in a reply says whether the session's branch was read, or the
//! working tree once that branch is gone. `project_files.rs` does the work.

use crate::domain::project_file::{CommitMessage, MAX_COMMIT_PATHS, RelPath, SessionBranch};
use crate::project_files::{self, Version};
use crate::web::App;
use crate::web::http::{HttpError, Request, error, require_project, require_session, send_json};
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

/// `GET /api/projects/{id}/files[?session=…]`.
pub fn list(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let workdir = Path::new(&project.workdir);
    let version = project_files::resolve(workdir, version(app, req, id)?);
    let listing = project_files::list(workdir, &version).map_err(|e| HttpError::new(500, e))?;
    let mut body = serde_json::to_value(listing).map_err(|e| e.to_string())?;
    body["session"] = json!(matches!(version, Version::Session(_)));
    send_json(stream, 200, &body)
}

/// `GET /api/projects/{id}/file?path=…[&session=…]`.
pub fn read(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let path_raw = req
        .query_param("path")
        .ok_or_else(|| HttpError::new(400, "missing path"))?;
    let path = RelPath::parse(&path_raw).map_err(|e| HttpError::new(400, e))?;
    let workdir = Path::new(&project.workdir);
    let version = project_files::resolve(workdir, version(app, req, id)?);

    let view = match project_files::read(workdir, &version, &path) {
        Ok(view) => view,
        Err(file_error) => return error(stream, file_error.status(), &file_error.message()),
    };

    let mut body = serde_json::to_value(view).map_err(|e| e.to_string())?;
    body["session"] = json!(matches!(version, Version::Session(_)));
    send_json(stream, 200, &body)
}

/// `POST /api/projects/{id}/file`: the human's save writes the file to disk.
pub fn save(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let body = req.json()?;

    let path_raw = body
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| HttpError::new(400, "missing path"))?;
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| HttpError::new(400, "missing content"))?;
    let base = body
        .get("base")
        .and_then(Value::as_str)
        .ok_or_else(|| HttpError::new(400, "missing base"))?;

    let path = RelPath::parse(path_raw).map_err(|e| HttpError::new(400, e))?;
    let workdir = Path::new(&project.workdir);

    let blob = match project_files::write(workdir, &path, content, base) {
        Ok(blob) => blob,
        Err(file_error) => return error(stream, file_error.status(), &file_error.message()),
    };

    send_json(stream, 200, &json!({ "blob": blob }))
}

/// `GET /api/projects/{id}/changes`.
pub fn changes(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let workdir = Path::new(&project.workdir);
    let files = project_files::changes(workdir).map_err(|e| HttpError::new(500, e))?;
    send_json(stream, 200, &json!({ "files": files }))
}

/// `POST /api/projects/{id}/commit`: one commit of the ticked files.
pub fn commit(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let body = req.json()?;

    let paths_raw = body
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| HttpError::new(400, "paths must be an array of strings"))?;
    if paths_raw.len() > MAX_COMMIT_PATHS {
        return Err(HttpError::new(400, "too many paths to commit"));
    }

    let mut paths = Vec::with_capacity(paths_raw.len());
    for value in paths_raw {
        let raw = value
            .as_str()
            .ok_or_else(|| HttpError::new(400, "paths must be an array of strings"))?;
        paths.push(RelPath::parse(raw).map_err(|e| HttpError::new(400, e))?);
    }

    let message_raw = body
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| HttpError::new(400, "missing message"))?;
    let message = CommitMessage::parse(message_raw).map_err(|e| HttpError::new(400, e))?;
    let workdir = Path::new(&project.workdir);

    let commit_id = match project_files::commit(workdir, &paths, &message) {
        Ok(commit_id) => commit_id,
        Err(file_error) => return error(stream, file_error.status(), &file_error.message()),
    };

    send_json(stream, 200, &json!({ "commit": commit_id }))
}

fn version(app: &App, req: &Request, project_id: &str) -> Result<Version, HttpError> {
    let Some(session_id) = req.query_param("session") else {
        return Ok(Version::Tree);
    };
    if session_id.is_empty() {
        return Ok(Version::Tree);
    }

    let branch = SessionBranch::parse(&session_id).map_err(|e| HttpError::new(400, e))?;
    let session = require_session(&app.store()?, &session_id)?;
    if session.project_id != project_id {
        return Err(HttpError::new(404, "no such session"));
    }

    Ok(Version::Session(branch))
}
