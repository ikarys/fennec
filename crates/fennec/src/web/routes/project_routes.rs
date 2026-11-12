//! The project page's routes (contract: `docs/devs/api.md`): what fennec reads of a
//! project (its map, its ADRs) and the map written to `docs/map.md`.
//! `routes.rs` only dispatches here.

use crate::web::http::{HttpError, require_project};
use std::net::TcpStream;
use std::path::Path;

use serde_json::json;

use crate::domain::drift;
use crate::web::App;
use crate::web::http::send_json;
use crate::{adr, architecture, module_map, risk};

/// `GET /api/projects/{id}/overview`.
pub fn get_overview(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let workdir = Path::new(&project.workdir);
    let mut map = module_map::load(workdir);
    let map_file = module_map::map_file_state(workdir, &map);
    let drift = drift::measure(
        &mut map,
        &architecture::load(workdir),
        module_map::committed_map_file(workdir).as_deref(),
    );
    let body = json!({
        "mapFile": map_file,
        "map": map,
        "drift": drift,
    });
    send_json(stream, 200, &body)
}

/// `GET /api/projects/{id}/adrs`: the project's ADRs, read from disk
/// (verified), for the project page's Decisions tab.
pub fn get_adrs(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let body = json!({ "adrs": adr::load(Path::new(&project.workdir)) });
    send_json(stream, 200, &body)
}

/// `POST /api/projects/{id}/map-file`: the human's click writes `docs/map.md`
/// in their tree; committing it stays theirs.
pub fn write_map_file(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let workdir = Path::new(&project.workdir);
    let map = module_map::load(workdir);
    module_map::write_map_file(workdir, &map).map_err(|e| e.to_string())?;
    let state = module_map::map_file_state(workdir, &map);
    send_json(stream, 200, &json!({ "mapFile": state }))
}

/// `GET /api/projects/{id}/risks`: risk zones for a project workdir.
pub fn get_risks(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let project = require_project(&app.store()?, id)?;
    let workdir = Path::new(&project.workdir);
    let body = serde_json::to_value(risk::load(workdir)).map_err(|e| e.to_string())?;
    send_json(stream, 200, &body)
}
