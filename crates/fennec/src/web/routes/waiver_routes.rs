//! The human's answer to a `waiver.requested`: write the grant's ADR into the
//! session's workdir, record the answer, then produce the call's result in a turn.

use crate::agent::ports::EventLog;
use crate::domain::clock;
use crate::domain::event::Event;
use crate::domain::waiver::{self, MAX_HOURS};
use crate::notifier::NotifyingLog;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use crate::web::turns::Job;
use serde_json::json;
use std::net::TcpStream;

pub(crate) fn answer_waiver(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let body = req.json()?;
    let call_id = body["callId"].as_str().unwrap_or("").to_string();
    let grant = body["grant"].as_bool().unwrap_or(false);
    let store = app.store()?;
    let (session, project) = app.require_session(id)?;
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let events = store.events(id)?;
    let Some(pending) = waiver::pending(&events).filter(|pending| pending.call_id == call_id)
    else {
        return error(stream, 409, "no pending waiver for this call");
    };

    let place = app.place(&store, &session, &project)?;
    let granted = if grant {
        let Some(hours) = body["hours"]
            .as_u64()
            .filter(|hours| (1..=u64::from(MAX_HOURS)).contains(hours))
        else {
            return error(stream, 400, "grant requires hours between 1 and 168");
        };
        let expires_at = clock::now_ms() + hours * 3_600_000;
        let number = crate::adr::next_number(&place.workdir);
        let file_name = waiver::adr_file_name(number, &pending.path);
        let (year, month, day) = clock::today();
        let date = format!("{year:04}-{month:02}-{day:02}");
        let expires = clock::format_utc(expires_at);
        let content = waiver::adr(&pending, id, &date, &expires);
        let adr =
            crate::adr::write(&place.workdir, &file_name, &content).map_err(|e| e.to_string())?;
        Some((expires_at, adr))
    } else {
        None
    };

    // Recorded now, before the turn starts: the turn's protection is built from
    // the log, so a granted path is already writable when the agent resumes.
    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        id,
        &Event::WaiverAnswered {
            call_id: call_id.clone(),
            granted: granted.is_some(),
            expires_at: granted.as_ref().map(|(expires_at, _)| *expires_at),
            adr: granted.map(|(_, adr)| adr),
        },
    )?;

    match app.turns.start(id, place, Job::Waiver { call_id }) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
}
