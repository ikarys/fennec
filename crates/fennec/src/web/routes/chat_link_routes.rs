//! Linking a chat to a project: a database reference for grouping only.

use crate::chat;
use crate::domain::chat_link;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use serde_json::json;
use std::net::TcpStream;

pub(crate) fn link(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let (session, _) = app.require_session(id)?;
    let body = req.json()?;
    let target = match body["projectId"].as_str() {
        Some(project_id) => Some(project_id),
        None if body["projectId"].is_null() => None,
        None => return error(stream, 400, "projectId must be a string or null"),
    };
    if let Some(project_id) = target
        && app.store()?.project(project_id)?.is_none()
    {
        return error(stream, 404, "no such project");
    }
    if let Err(refusal) = chat_link::check(&session.project_id, target, chat::ID) {
        return match refusal {
            chat_link::LinkRefusal::NotAChat => {
                error(stream, 400, "only a chat can be linked to a project")
            }
            chat_link::LinkRefusal::ChatTarget => {
                error(stream, 400, "a chat cannot be linked to the chat")
            }
        };
    }
    app.store()?.link_session(id, target)?;
    send_json(stream, 200, &json!({ "linkedProjectId": target }))
}
