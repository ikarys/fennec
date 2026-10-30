//! The project page's ideas backlog (contract: `docs/api.md`): every write is
//! the human's click. `routes.rs` only dispatches here.

use std::net::TcpStream;

use serde_json::json;

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::id::new_id;
use crate::domain::idea::IdeaEvent;

/// An idea is a note, not a document: the plan holds the detail.
const MAX_IDEA_CHARS: usize = 4_000;

/// `GET /api/projects/{id}/ideas`.
pub fn list_ideas(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    send_ideas(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas` `{text}`.
pub fn add_idea(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    let body = req.json()?;
    let text = body["text"].as_str().unwrap_or("").trim();
    if text.is_empty() {
        return error(stream, 400, "empty idea");
    }
    if text.chars().count() > MAX_IDEA_CHARS {
        return error(stream, 400, "idea too long");
    }
    let event = IdeaEvent::Added {
        idea_id: new_id(),
        text: text.to_string(),
    };
    app.store()?.append_idea_event(project_id, &event)?;
    send_ideas(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/{iid}/{framing,ready,drop}`.
pub fn update_idea(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
    idea_id: &str,
    action: &str,
) -> Result<(), String> {
    let store = app.store()?;
    if store.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    if !store
        .ideas(project_id)?
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
            IdeaEvent::Framing {
                idea_id,
                session_id,
            }
        }
        "ready" => {
            let plan = field("plan");
            if plan.is_empty() {
                return error(stream, 400, "empty plan");
            }
            IdeaEvent::Ready { idea_id, plan }
        }
        "drop" => IdeaEvent::Dropped { idea_id },
        _ => return error(stream, 404, "not found"),
    };
    store.append_idea_event(project_id, &event)?;
    send_ideas(app, stream, project_id)
}

fn send_ideas(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    let ideas = app.store()?.ideas(project_id)?;
    send_json(stream, 200, &json!({ "ideas": ideas }))
}
