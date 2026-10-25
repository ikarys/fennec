//! The JSON API (contract: `docs/api.md`) and the UI's pages.

use super::App;
use super::http::{Request, error, send, send_json};
use super::{assets, event_stream};
use crate::domain::preferences::Preferences;
use crate::domain::project::{Project, SessionSummary};
use crate::domain::slash::{TurnInput, commands, resolve};
use crate::llm::list_models;
use crate::skills;
use crate::store::SqliteStore;
use crate::{folders, git};
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

pub fn handle(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let segments: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    match (req.method.as_str(), segments.as_slice()) {
        ("GET", ["api", "config"]) => {
            let store = app.store()?;
            let prefs = store.preferences()?;
            let params = app.settings.llm.for_turn(&prefs);
            send_json(
                stream,
                200,
                &json!({ "model": params.model, "contextWindow": params.context_window }),
            )
        }
        ("GET", ["api", "settings"]) => get_settings(app, stream),
        ("POST", ["api", "settings"]) => save_settings(app, req, stream),
        ("GET", ["api", "folders"]) => list_folders(req, stream),
        ("GET", ["api", "projects"]) => list_projects(app, stream),
        ("POST", ["api", "projects"]) => open_project(app, req, stream),
        ("GET", ["api", "projects", id, "commands"]) => list_commands(app, stream, id),
        ("POST", ["api", "sessions"]) => create_session(app, req, stream),
        ("GET", ["api", "sessions", id]) => get_session(app, stream, id),
        ("POST", ["api", "sessions", id, "messages"]) => send_message(app, req, stream, id),
        ("POST", ["api", "sessions", id, "compact"]) => compact_session(app, stream, id),
        ("POST", ["api", "sessions", id, "stop"]) => {
            app.turns.stop(id);
            send_json(stream, 202, &json!({}))
        }
        ("GET", ["api", "sessions", id, "events"]) => {
            if req.query_param("limit").is_some() {
                serve_events_page(app, req, stream, id)
            } else {
                event_stream::serve(&app.store()?, &app.notifier, req, stream, id)
            }
        }
        ("GET", ["api", ..]) | ("POST", _) => error(stream, 404, "not found"),
        ("GET", _) => {
            let asset = assets::get(&req.path);
            send(stream, 200, asset.content_type, asset.body)
        }
        _ => error(stream, 404, "not found"),
    }
}

fn get_settings(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    let prefs = store.preferences()?;
    // The endpoint may have loaded another model since start; fall back to
    // the startup listing when it cannot be reached now.
    let models = list_models(
        &app.settings.llm.base_url,
        app.settings.llm.api_key.as_deref(),
    )
    .unwrap_or_else(|_| app.settings.llm.listed.clone());
    let body = json!({
        "preferences": prefs,
        "models": models,
        "defaults": app.settings.llm.defaults,
    });
    send_json(stream, 200, &body)
}

fn save_settings(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let prefs: Preferences = match serde_json::from_value(body) {
        Ok(prefs) => prefs,
        Err(e) => return error(stream, 400, &format!("invalid preferences: {e}")),
    };
    if let Err(e) = prefs.validate() {
        return error(stream, 400, &e);
    }
    app.store()?.save_preferences(&prefs)?;
    get_settings(app, stream)
}

fn list_folders(req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let dir = req
        .query_param("path")
        .filter(|p| !p.is_empty())
        .map_or_else(folders::home, Into::into);
    match folders::list(&dir) {
        Ok(listing) => send_json(stream, 200, &json!(listing)),
        Err(e) => error(stream, 400, &e),
    }
}

fn list_projects(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    let mut out = Vec::new();
    for p in store.projects()? {
        let sessions: Vec<Value> = store
            .sessions(Some(&p.id))?
            .iter()
            .map(|s| session_json(app, s))
            .collect();
        let mut v = project_json(&p);
        v["branch"] = json!(git::current_branch(Path::new(&p.workdir)));
        v["sessions"] = Value::Array(sessions);
        out.push(v);
    }
    send_json(stream, 200, &Value::Array(out))
}

fn open_project(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let dir = Path::new(body["workdir"].as_str().unwrap_or(""));
    let Some(dir) = dir.canonicalize().ok().filter(|d| d.is_dir()) else {
        return error(stream, 400, "not a directory");
    };
    let p = app.store()?.project_for(&dir.display().to_string())?;
    send_json(stream, 200, &project_json(&p))
}

fn list_commands(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let list = commands(&skills::discover(Path::new(&project.workdir)));
    let out: Vec<Value> = list
        .into_iter()
        .map(|command| json!({ "name": command.name, "description": command.description }))
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

fn create_session(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let prompt = body["prompt"].as_str().unwrap_or("").trim().to_string();
    if prompt.is_empty() {
        return error(stream, 400, "empty prompt");
    }
    let store = app.store()?;
    let Some(project) = store.project(body["projectId"].as_str().unwrap_or(""))? else {
        return error(stream, 404, "no such project");
    };
    let title: String = prompt.chars().take(80).collect();
    let id = store.create_session(&project, &title)?;
    let input = resolve(&prompt, &skills::discover(Path::new(&project.workdir)));
    app.turns.start(&id, &project.workdir, input)?;
    send_json(stream, 200, &json!({ "id": id }))
}

fn get_session(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some((s, p)) = session_and_project(&app.store()?, id)? else {
        return error(stream, 404, "no such session");
    };
    let mut v = session_json(app, &s);
    v["project"] = project_json(&p);
    send_json(stream, 200, &v)
}

fn send_message(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let body = req.json()?;
    let content = body["content"].as_str().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return error(stream, 400, "empty message");
    }
    let Some((_, p)) = session_and_project(&app.store()?, id)? else {
        return error(stream, 404, "no such session");
    };
    let input = resolve(&content, &skills::discover(Path::new(&p.workdir)));
    match app.turns.start(id, &p.workdir, input) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
}

fn compact_session(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some((_, p)) = session_and_project(&app.store()?, id)? else {
        return error(stream, 404, "no such session");
    };
    match app.turns.start(id, &p.workdir, TurnInput::Compact) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
}

/// One page of a session's log as JSON, newest-first boundary: the last
/// `limit` events before `before` (the log's end when omitted), ascending.
/// `GET /api/sessions/{id}/events?limit=N&before=M`.
fn serve_events_page(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let Some(limit) = req.query_param("limit").and_then(|v| v.parse::<u64>().ok()) else {
        return error(stream, 400, "limit is not a number");
    };
    if !(1..=10_000).contains(&limit) {
        return error(stream, 400, "limit must be in 1..=10000");
    }
    let before = req
        .query_param("before")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(u64::MAX);
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let page = store.events_before(id, before, limit)?;
    let out: Vec<Value> = page
        .into_iter()
        .map(|(seq, event)| json!({ "seq": seq, "event": event }))
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

fn session_and_project(
    store: &SqliteStore,
    id: &str,
) -> Result<Option<(SessionSummary, Project)>, String> {
    let Some(s) = store.session(id)? else {
        return Ok(None);
    };
    Ok(store.project(&s.project_id)?.map(|p| (s, p)))
}

fn session_json(app: &App, s: &SessionSummary) -> Value {
    json!({
        "id": s.id, "title": s.title, "state": s.state(app.turns.is_running(&s.id)),
        "createdAt": s.created_at, "updatedAt": s.updated_at,
    })
}

fn project_json(p: &Project) -> Value {
    json!({ "id": p.id, "name": p.name, "workdir": p.workdir })
}
