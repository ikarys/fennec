//! The folders the human browses to open a project, and the one they create.

use super::App;
use super::http::{Request, error, send_json};
use crate::folders;
use crate::web::http::HttpError;
use serde_json::json;
use std::net::TcpStream;
use std::path::Path;

pub(super) fn list_folders(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
) -> Result<(), HttpError> {
    let dir = req
        .query_param("path")
        .filter(|p| !p.is_empty())
        .map_or_else(folders::home, Into::into);
    if folders::inside_store(&dir, store_dir(app)) {
        return error(
            stream,
            400,
            "fennec's own store cannot be browsed as a project",
        );
    }
    match folders::list(&dir) {
        Ok(listing) => send_json(stream, 200, &json!(listing)),
        Err(e) => error(stream, 400, &e),
    }
}

/// The directory holding fennec's database and the sessions' worktrees.
pub(super) fn store_dir(app: &App) -> &Path {
    app.settings.db_path.parent().unwrap_or(Path::new("/"))
}

pub(super) fn create_folder(req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let body = req.json()?;
    let base = body["path"].as_str().unwrap_or("");
    let name = body["name"].as_str().unwrap_or("");
    let base = if base.is_empty() {
        folders::home()
    } else {
        Path::new(base).into()
    };
    match folders::create(&base, name) {
        Ok(path) => send_json(stream, 200, &json!({ "path": path.display().to_string() })),
        Err(e) => error(stream, 400, &e),
    }
}
