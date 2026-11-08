//! The project page's ideas backlog (contract: `docs/api.md`): every write is
//! the human's click. `routes.rs` only dispatches here.

use crate::web::http::HttpError;
use std::net::TcpStream;
use std::sync::Arc;

use serde_json::Value;

use crate::domain::id::new_id;
use crate::web::App;
use crate::web::http::{Request, error, require_project, send_json};
use crate::web::jobs::idea_queue;
use crate::web::views::idea_board::{Board, board_json};
use backlog::{BacklogEvent, IdeaAttachment, IdeaState, Status, queue_order};

/// An idea is a note, not a document: the plan holds the detail.
const MAX_IDEA_CHARS: usize = 4_000;

/// The pinned files on an idea: how many, and how big one may be. The stored
/// payload is a base64 data URL, so a 5 MB file inflates to ~4/3 before the
/// `data:` prefix — bound the payload, not the raw bytes, or a full 5 MB file
/// the composer accepts would come back a 400.
const MAX_IDEA_ATTACHMENTS: usize = 10;
const MAX_IDEA_ATTACHMENT_BYTES: u64 = 5 * 1024 * 1024 * 4 / 3 + 1024;
// The request body is capped at 32 MB (http.rs); keep the total under it so an
// over-large idea gets a clean message, not the generic "body too large".
const MAX_IDEA_ATTACHMENTS_TOTAL: u64 = 30 * 1024 * 1024;

/// `{filename, mimeType, size, data}` items of an idea-create body.
fn parse_attachments(body: &Value) -> Result<Vec<IdeaAttachment>, String> {
    let Some(items) = body["attachments"].as_array() else {
        return Ok(Vec::new());
    };
    if items.len() > MAX_IDEA_ATTACHMENTS {
        return Err(format!(
            "at most {MAX_IDEA_ATTACHMENTS} attachments per idea"
        ));
    }
    let mut attachments = Vec::with_capacity(items.len());
    let mut total: u64 = 0;
    for item in items {
        let filename = item["filename"].as_str().unwrap_or("").trim().to_string();
        let mime_type = item["mimeType"].as_str().unwrap_or("").to_string();
        let data = item["data"].as_str().unwrap_or("").to_string();
        if filename.is_empty() || data.is_empty() {
            return Err("an attachment needs a filename and a data URL".into());
        }
        // The declared size is what the browser said; the payload is what
        // we store, so limit the payload (base64 ≈ 4/3 of the bytes).
        if data.len() as u64 > MAX_IDEA_ATTACHMENT_BYTES {
            return Err(format!("attachment {filename} larger than 5 MB"));
        }
        total += data.len() as u64;
        if total > MAX_IDEA_ATTACHMENTS_TOTAL {
            return Err("attachments total too large".into());
        }
        attachments.push(IdeaAttachment {
            filename,
            mime_type,
            // The raw file size the client measured; fall back to the payload
            // length when it is not in the body.
            size: item["size"].as_u64().unwrap_or(data.len() as u64),
            data,
        });
    }
    Ok(attachments)
}

/// `GET /api/projects/{id}/ideas`.
pub fn list_ideas(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), HttpError> {
    require_project(&app.store()?, project_id)?;
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas` `{text, sessionId?, plan?}`: with a
/// session and its plan, a plan written outside the backlog joins it, ready.
pub fn add_idea(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), HttpError> {
    let store = app.store()?;
    require_project(&store, project_id)?;
    let body = req.json()?;
    let field = |name: &str| body[name].as_str().unwrap_or("").trim().to_string();
    let text = field("text");
    if text.is_empty() {
        return error(stream, 400, "empty idea");
    }
    if text.chars().count() > MAX_IDEA_CHARS {
        return error(stream, 400, "idea too long");
    }
    let attachments = match parse_attachments(&body) {
        Ok(attachments) => attachments,
        Err(e) => return error(stream, 400, &e),
    };
    let (session_id, plan) = (field("sessionId"), field("plan"));
    if !session_id.is_empty() {
        match store.session(&session_id)? {
            Some(session) if session.project_id == project_id => {}
            _ => return error(stream, 404, "no such session"),
        }
        if plan.is_empty() {
            return error(stream, 400, "empty plan");
        }
    }
    let idea_id = new_id();
    store.append_idea_event(
        project_id,
        &BacklogEvent::Added {
            idea_id: idea_id.clone(),
            text,
            attachments,
        },
    )?;
    if !session_id.is_empty() {
        store.append_idea_event(
            project_id,
            &BacklogEvent::Framing {
                idea_id: idea_id.clone(),
                session_id,
            },
        )?;
        store.append_idea_event(project_id, &BacklogEvent::Ready { idea_id, plan })?;
    }
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/{iid}/{framing,ready,edit,drop,complete}`.
pub fn update_idea(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
    idea_id: &str,
    action: &str,
) -> Result<(), HttpError> {
    let store = app.store()?;
    require_project(&store, project_id)?;
    if !store
        .backlog(project_id)?
        .ideas
        .iter()
        .any(|idea| idea.id == idea_id)
    {
        return error(stream, 404, "no such idea");
    }
    let body = req.json()?;
    let field = |name: &str| body[name].as_str().unwrap_or("").trim().to_string();
    let idea_id = idea_id.to_string();
    let event = match action {
        "framing" => {
            let session_id = field("sessionId");
            // Only a session of this project may frame its ideas.
            match store.session(&session_id)? {
                Some(session) if session.project_id == project_id => {}
                _ => return error(stream, 404, "no such session"),
            }
            BacklogEvent::Framing {
                idea_id,
                session_id,
            }
        }
        "ready" => {
            let plan = field("plan");
            if plan.is_empty() {
                return error(stream, 400, "empty plan");
            }
            BacklogEvent::Ready { idea_id, plan }
        }
        "edit" => {
            let text = field("text");
            if text.is_empty() {
                return error(stream, 400, "empty idea");
            }
            if text.chars().count() > MAX_IDEA_CHARS {
                return error(stream, 400, "idea too long");
            }
            BacklogEvent::Edited { idea_id, text }
        }
        "drop" => BacklogEvent::Dropped { idea_id },
        "complete" => BacklogEvent::Completed { idea_id },
        _ => return error(stream, 404, "not found"),
    };
    store.append_idea_event(project_id, &event)?;
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/{iid}/start`: builds this idea now, framed
/// or not, outside the queue (which then skips it, no longer ready). A halted
/// or failed idea is relaunched from its plan when it has one.
pub fn start_one(
    app: &App,
    stream: &mut TcpStream,
    project_id: &str,
    idea_id: &str,
) -> Result<(), HttpError> {
    let store = app.store()?;
    let project = require_project(&store, project_id)?;
    let backlog = store.backlog(project_id)?;
    let Some(idea) = backlog.ideas.iter().find(|idea| idea.id == idea_id) else {
        return error(stream, 404, "no such idea");
    };
    let board = Board::new(app, &store, &backlog, &project);
    let restart = idea.state == IdeaState::Started;
    if restart {
        // Only a halted or failed idea can be relaunched; a live one is busy.
        if !matches!(board.status_of(idea)?, Status::Halted | Status::Failed) {
            return error(stream, 409, "idea already started");
        }
    }
    if let Err(e) = idea_queue::start_idea(app, &store, &project, idea, false, restart) {
        return error(stream, 409, &e);
    }
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/queue/start`.
///
/// Starts the project's idea queue if it is not already running.
pub fn start_queue(
    app: &Arc<App>,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), HttpError> {
    require_project(&app.store()?, project_id)?;
    let autonomous = req.json()?["autonomous"].as_bool().unwrap_or(false);
    idea_queue::start_queue(app, project_id, autonomous);
    send_board(app.as_ref(), stream, project_id)
}

/// `POST /api/projects/{id}/ideas/queue/stop`.
///
/// Stops the queue from starting the next idea; a running turn finishes.
pub fn stop_queue(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), HttpError> {
    require_project(&app.store()?, project_id)?;
    idea_queue::stop_queue(app, project_id);
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/reorder` `{ids: [string]}`.
pub fn reorder(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), HttpError> {
    let store = app.store()?;
    require_project(&store, project_id)?;
    let body = req.json()?;
    let Some(ids_value) = body["ids"].as_array() else {
        return error(stream, 400, "ids must be a list of idea ids");
    };
    let mut ids = Vec::with_capacity(ids_value.len());
    for value in ids_value {
        let Some(id) = value.as_str() else {
            return error(stream, 400, "ids must be a list of idea ids");
        };
        ids.push(id.to_string());
    }

    let backlog = store.backlog(project_id)?;
    let queued: std::collections::HashSet<&str> = queue_order(&backlog)
        .iter()
        .map(|idea| idea.id.as_str())
        .collect();
    if ids.iter().any(|id| !queued.contains(id.as_str())) {
        return error(stream, 400, "unknown idea in ids");
    }

    store.append_idea_event(project_id, &BacklogEvent::Reordered { ids })?;
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/report/read`.
pub fn report_read(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    require_project(&store, project_id)?;
    store.append_idea_event(project_id, &BacklogEvent::ReportRead {})?;
    send_board(app, stream, project_id)
}

fn send_board(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let project = require_project(&store, project_id)?;
    let backlog = store.backlog(project_id)?;
    send_json(
        stream,
        200,
        &board_json(&Board::new(app, &store, &backlog, &project))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_attachments_parses_and_validates() {
        let body = json!({
            "attachments": [
                { "filename": "a.png", "mimeType": "image/png", "size": 3, "data": "data:image/png;base64,AAA=" }
            ]
        });
        let parsed = parse_attachments(&body).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].filename, "a.png");
        assert_eq!(parsed[0].mime_type, "image/png");
        assert_eq!(parsed[0].size, 3);

        assert!(parse_attachments(&json!({})).unwrap().is_empty());

        let missing = json!({ "attachments": [ { "filename": "", "data": "" } ] });
        assert!(parse_attachments(&missing).is_err());

        let too_big = json!({
            "attachments": [ { "filename": "x", "mimeType": "image/png", "data": format!("data:image/png;base64,{}", "A".repeat(MAX_IDEA_ATTACHMENT_BYTES as usize + 1)) } ]
        });
        assert!(parse_attachments(&too_big).is_err());

        // Seven 5 MB payloads clear the per-attachment cap but exceed the total.
        let too_many = json!({
            "attachments": (0..7).map(|i| json!({
                "filename": format!("f{i}"), "mimeType": "image/png",
                "data": format!("data:image/png;base64,{}", "A".repeat(5 * 1024 * 1024))
            })).collect::<Vec<_>>()
        });
        assert!(parse_attachments(&too_many).is_err());
    }
}
