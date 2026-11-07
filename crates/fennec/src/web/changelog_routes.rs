//! The changelog over HTTP: the About tab lists it, the top bar counts what is new since the human last looked.

use super::App;
use super::http::send_json;
use crate::domain::changelog;
use crate::web::http::HttpError;
use serde_json::json;
use std::net::TcpStream;

const CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");

pub(super) fn serve(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    let store = app.store()?;
    let releases = changelog::parse(CHANGELOG);
    let current = env!("CARGO_PKG_VERSION");

    // A fresh install has nothing new to show: its first look records the running version.
    let seen = match store.seen_version()? {
        Some(seen) => seen,
        None => {
            store.set_seen_version(current)?;
            current.to_string()
        }
    };
    let unseen = changelog::unseen(&releases, &seen, current);

    send_json(
        stream,
        200,
        &json!({
            "current": current,
            "releases": releases,
            "unseen": unseen,
        }),
    )
}

pub(super) fn mark_seen(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    let store = app.store()?;
    let current = env!("CARGO_PKG_VERSION");

    store.set_seen_version(current)?;

    send_json(stream, 200, &json!({ "unseen": 0 }))
}
