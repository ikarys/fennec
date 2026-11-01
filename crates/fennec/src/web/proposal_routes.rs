//! A proposed task turned into a backlog idea on the human's click only: the
//! server reads the proposal from the session log and records the acceptance.

use std::net::TcpStream;

use serde_json::json;

use super::App;
use super::http::{error, send_json};
use crate::agent::ports::EventLog;
use crate::domain::event::Event;
use crate::domain::id::new_id;
use crate::domain::task_proposal;
use crate::notifier::NotifyingLog;
use backlog::BacklogEvent;

/// `POST /api/sessions/{id}/proposals/{call_id}/accept` → `{ideaId}`.
pub fn accept(
    app: &App,
    stream: &mut TcpStream,
    session_id: &str,
    call_id: &str,
) -> Result<(), String> {
    let store = app.store()?;
    let Some((session, project)) = app.session_and_project(session_id)? else {
        return error(stream, 404, "no such session");
    };

    let events = store.events(&session.id)?;
    let Some(proposal) = task_proposal::proposed_tasks(&events)
        .into_iter()
        .find(|task| task.call_id == call_id)
    else {
        return error(stream, 404, "no such proposal");
    };
    if proposal.idea_id.is_some() {
        return error(stream, 409, "proposal already accepted");
    }

    let idea_id = new_id();
    store.append_idea_event(
        &project.id,
        &BacklogEvent::Added {
            idea_id: idea_id.clone(),
            text: proposal.task_text(),
            attachments: Vec::new(),
        },
    )?;

    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        &session.id,
        &Event::TaskAccepted {
            call_id: call_id.into(),
            idea_id: idea_id.clone(),
        },
    )?;

    send_json(stream, 200, &json!({ "ideaId": idea_id }))
}
