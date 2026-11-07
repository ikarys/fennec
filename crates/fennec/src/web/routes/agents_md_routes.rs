//! The human's click to write the agent's AGENTS.md draft: it writes the
//! human's tree only, never commits.

use crate::web::http::HttpError;
use std::fs;
use std::net::TcpStream;
use std::path::Path;

use serde_json::json;

use crate::agent::ports::EventLog;
use crate::domain::agents_md;
use crate::domain::conversation::last_assistant_text;
use crate::domain::event::Event;
use crate::notifier::NotifyingLog;
use crate::web::App;
use crate::web::http::{error, send_json};

/// `POST /api/sessions/{id}/agents-md` → `{replaced}`.
pub fn write(app: &App, stream: &mut TcpStream, session_id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let Some((session, project)) = app.session_and_project(session_id)? else {
        return error(stream, 404, "no such session");
    };

    let events = store.events(&session.id)?;
    let Some(draft) = agents_md::draft(&last_assistant_text(&events)) else {
        return error(stream, 400, "no AGENTS.md draft in the last answer");
    };

    let path = Path::new(&project.workdir).join("AGENTS.md");
    // A symlink planted by a command would make the human's click write elsewhere.
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return error(stream, 409, "AGENTS.md is a symlink: not written");
    }
    let replaced = path.exists();
    fs::write(&path, draft).map_err(|err| format!("cannot write {}: {err}", path.display()))?;

    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(&session.id, &Event::AgentsMdWritten { replaced })?;

    send_json(stream, 200, &json!({ "replaced": replaced }))
}
