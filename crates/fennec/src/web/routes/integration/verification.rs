//! The verification route: the run's verification block, independent of the preference.

use crate::agent::ports::EventLog;
use crate::web::App;
use crate::web::http::{HttpError, send_json};
use serde_json::json;
use std::net::TcpStream;

use super::gate::target;

/// `GET /api/sessions/{id}/verification`: the run's verification and its block.
pub fn get_verification(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    crate::web::http::require_session(&store, id)?;
    let changed = match target(&store, app, id) {
        Ok(target) => crate::integration::changed_files(
            &target.top,
            &target.wt,
            &target.preview.files,
            &target.prefix,
        )
        .ok()
        .map(|changed| {
            changed
                .into_iter()
                .map(|(path, _)| path)
                .collect::<Vec<_>>()
        }),
        Err(_) => None,
    };
    let events = store.events(id)?;
    let verification = crate::domain::verification::of(&events, changed.as_deref());
    let mut body = json!(verification);
    body["block"] = json!(verification.block());
    send_json(stream, 200, &body)
}
