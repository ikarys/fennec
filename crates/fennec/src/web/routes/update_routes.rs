//! Routes that report update availability and apply a self-update.

use crate::web::http::HttpError;
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use crate::web::App;
use crate::web::http::{Request, error, send_json};

// GitHub is asked at most that often: the UI asks on load and when the tab
// regains focus.
const CACHE: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateView {
    current: String,
    latest: String,
    newer: bool,
    release_url: String,
}

fn send_check(stream: &mut TcpStream, check: &crate::update::Check) -> Result<(), HttpError> {
    let release_url = crate::update::release_url(&check.latest);
    let view = UpdateView {
        current: check.current.to_string(),
        latest: check.latest.clone(),
        newer: check.newer,
        release_url,
    };
    send_json(stream, 200, &json!(view))
}

pub fn check(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    // `refresh=1` is the Settings button's manual bypass.
    let refresh = req.query_param("refresh").as_deref() == Some("1");
    let cached = {
        let guard = crate::locks::lock(&app.update_check);
        guard
            .as_ref()
            .filter(|(checked_at, _)| !refresh && checked_at.elapsed() < CACHE)
            .map(|(_, check)| check.clone())
    };

    match cached {
        Some(check) => send_check(stream, &check),
        None => match crate::update::check() {
            Ok(check) => {
                *crate::locks::lock(&app.update_check) = Some((Instant::now(), check.clone()));
                send_check(stream, &check)
            }
            Err(message) => error(stream, 502, &message),
        },
    }
}

pub fn apply(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    // A development build lives in target/: git updates it, the installer would overwrite it.
    if cfg!(debug_assertions) {
        return error(
            stream,
            409,
            "a development build updates with git, not the installer",
        );
    }
    // A local build is installed by `just install`; the GitHub installer would replace it.
    if crate::update::BUILD.is_some() {
        return error(
            stream,
            409,
            "a local build updates with just install, not the installer",
        );
    }
    if !app.turns.running_ids().is_empty() {
        return error(stream, 409, "a turn is running: update when it ends");
    }

    let check = match crate::update::check() {
        Ok(check) => check,
        Err(message) => return error(stream, 502, &message),
    };

    *crate::locks::lock(&app.update_check) = Some((Instant::now(), check.clone()));

    if !check.newer {
        return error(stream, 409, "fennec is already up to date");
    }

    let binary = match crate::update::install(&check.latest, true) {
        Ok(binary) => binary,
        Err(message) => return error(stream, 502, &message),
    };

    let latest = check.latest;
    send_json(
        stream,
        202,
        &json!({
            "restarting": true,
            "latest": latest.clone()
        }),
    )?;

    std::thread::spawn(move || {
        // Let the HTTP response flush before the process is replaced.
        std::thread::sleep(Duration::from_millis(300));
        eprintln!("fennec · restarting on {latest}");
        let e = crate::update::restart(&binary);
        eprintln!("fennec: restart failed: {e}");
    });

    Ok(())
}
