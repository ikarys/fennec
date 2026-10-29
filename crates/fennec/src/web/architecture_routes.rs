//! The Architecture page's route (contract: `docs/api.md`): a project's ADRs.
//! `routes.rs` only dispatches here.

use std::net::TcpStream;
use std::path::Path;

use serde_json::json;

use super::App;
use super::http::{error, send_json};
use crate::adr;

/// `GET /api/projects/{id}/architecture`.
pub fn get_architecture(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some(project) = app.store()?.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let adrs = adr::load(Path::new(&project.workdir));
    send_json(stream, 200, &json!({ "adrs": adrs }))
}
