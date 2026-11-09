//! One changed file of a session's branch as diff lines, for the Changes page.

use crate::domain::diff;
use crate::integration;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use crate::worktree;
use serde_json::json;
use std::net::TcpStream;
use std::path::Path;

pub fn get_diff(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let store = app.store()?;
    let (session, project) = app.require_session(id)?;
    let Some(wt) = app.place(&store, &session, &project)?.worktree else {
        return error(stream, 409, "the session has no worktree");
    };
    let Some((top, _prefix)) = worktree::top(Path::new(&project.workdir)) else {
        return error(stream, 409, "the project is not in a git repository");
    };

    let Some(path) = req.query_param("path") else {
        return error(stream, 400, "path is required");
    };
    if path.is_empty() {
        return error(stream, 400, "path is required");
    }

    let raw = match integration::file_diff(&top, &wt, &path) {
        Ok(Some(raw)) => raw,
        Ok(None) => return error(stream, 404, "the branch did not change this file"),
        Err(e) => return error(stream, 409, &e),
    };

    let file_diff = diff::file_diff(&raw);
    let editor_url = crate::web::os::editor::link(
        store.preferences()?.display.editor,
        &wt.path.join(&path).to_string_lossy(),
        std::env::var("WSL_DISTRO_NAME").ok().as_deref(),
    );

    let value = json!({
        "path": path,
        "binary": file_diff.binary,
        "truncated": file_diff.truncated,
        "hunks": file_diff.hunks,
        "editorUrl": editor_url,
    });
    send_json(stream, 200, &value)
}
