//! The human's click to append the agent's proposed architecture rules: it
//! writes the human's tree only, never commits.

use crate::web::http::HttpError;
use std::fs;
use std::net::TcpStream;
use std::path::Path;

use serde_json::json;

use super::App;
use super::http::{error, send_json};
use crate::agent::ports::EventLog;
use crate::domain::conversation::last_assistant_text;
use crate::domain::event::Event;
use crate::domain::rule_bridge;
use crate::notifier::NotifyingLog;

/// `POST /api/sessions/{id}/architecture-rules` → `{count}`.
pub fn add(app: &App, stream: &mut TcpStream, session_id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let Some((session, project)) = app.session_and_project(session_id)? else {
        return error(stream, 404, "no such session");
    };

    let events = store.events(&session.id)?;
    let proposed = rule_bridge::proposed_lines(&last_assistant_text(&events));
    let agents_dir = Path::new(&project.workdir).join(".agents");
    let path = agents_dir.join("architecture");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let new_lines = rule_bridge::new_lines(&proposed, &existing);
    let count = new_lines
        .iter()
        .filter(|line| !line.trim().starts_with('#'))
        .count();
    if count == 0 {
        return error(stream, 400, "no rule to add");
    }

    // A symlink planted by a command would make the human's click write elsewhere.
    if fs::symlink_metadata(&agents_dir).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return error(stream, 409, ".agents is a symlink: not written");
    }
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return error(stream, 409, "architecture is a symlink: not written");
    }
    fs::create_dir_all(&agents_dir)
        .map_err(|err| format!("cannot create {}: {err}", agents_dir.display()))?;

    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for line in &new_lines {
        content.push_str(line);
        content.push('\n');
    }
    fs::write(&path, content).map_err(|err| format!("cannot write {}: {err}", path.display()))?;

    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(&session.id, &Event::ArchitectureRulesAdded { count })?;

    send_json(stream, 200, &json!({ "count": count }))
}
