//! The project page's routes (contract: `docs/api.md`): what fennec reads of a
//! project (its map, its ADRs) and the map written to `docs/map.md`.
//! `routes.rs` only dispatches here.

use std::net::TcpStream;
use std::path::Path;

use serde_json::json;

use super::App;
use super::http::{error, send_json};
use crate::{adr, doc, module_map, risk};

/// `GET /api/projects/{id}/overview`.
pub fn get_overview(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some(project) = app.store()?.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let workdir = Path::new(&project.workdir);
    let map = module_map::load(workdir);
    let body = json!({
        "mapFile": module_map::map_file_state(workdir, &map),
        "map": map,
    });
    send_json(stream, 200, &body)
}

/// `GET /api/projects/{id}/docs`: the project's documentation, read from disk
/// (verified) — its markdown files (`AGENTS.md`, `README.md`, `docs/`) and its
/// ADRs, for the project page's Doc tab.
pub fn get_docs(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some(project) = app.store()?.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let workdir = Path::new(&project.workdir);
    let body = json!({
        "files": doc::load(workdir),
        "adrs": adr::load(workdir),
    });
    send_json(stream, 200, &body)
}

/// `POST /api/projects/{id}/map-file`: the human's click writes `docs/map.md`
/// in their tree; committing it stays theirs.
pub fn write_map_file(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some(project) = app.store()?.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let workdir = Path::new(&project.workdir);
    let map = module_map::load(workdir);
    module_map::write_map_file(workdir, &map).map_err(|e| e.to_string())?;
    let state = module_map::map_file_state(workdir, &map);
    send_json(stream, 200, &json!({ "mapFile": state }))
}

/// `GET /api/projects/{id}/risks`: risk zones for a project workdir.
pub fn get_risks(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some(project) = app.store()?.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let workdir = Path::new(&project.workdir);
    let body = serde_json::to_value(risk::load(workdir)).map_err(|e| e.to_string())?;
    send_json(stream, 200, &body)
}
