//! The first-run checklist over HTTP: the home page shows it until nothing required is missing.

use super::App;
use super::http::send_json;
use serde_json::json;
use std::net::TcpStream;

pub(super) fn serve(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    let checks = crate::domain::setup::checks(&crate::setup::facts(&app.settings, &store)?);
    send_json(
        stream,
        200,
        &json!({
            "checks": checks,
            "complete": crate::domain::setup::complete(&checks)
        }),
    )
}
