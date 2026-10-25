//! The JSON API (contract: `docs/api.md`) and the UI's pages.

use super::App;
use super::http::{Request, error, send, send_json};
use super::{assets, event_stream};
use crate::domain::project::{Project, SessionSummary};
use crate::git;
use crate::store::SqliteStore;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

pub fn handle(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let segments: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    match (req.method.as_str(), segments.as_slice()) {
        ("GET", ["api", "config"]) => {
            let llm = &app.settings.llm;
            let config = json!({ "model": llm.model, "contextWindow": llm.context_window });
            send_json(stream, 200, &config)
        }
        ("GET", ["api", "projects"]) => list_projects(app, stream),
        ("POST", ["api", "projects"]) => open_project(app, req, stream),
        ("POST", ["api", "sessions"]) => create_session(app, req, stream),
        ("GET", ["api", "sessions", id]) => get_session(app, stream, id),
        ("POST", ["api", "sessions", id, "messages"]) => send_message(app, req, stream, id),
        ("POST", ["api", "sessions", id, "stop"]) => {
            app.turns.stop(id);
            send_json(stream, 202, &json!({}))
        }
        ("GET", ["api", "sessions", id, "events"]) => {
            event_stream::serve(&app.store()?, req, stream, id)
        }
        ("GET", ["api", ..]) | ("POST", _) => error(stream, 404, "not found"),
        ("GET", _) => {
            let asset = assets::get(&req.path);
            send(stream, 200, asset.content_type, asset.body)
        }
        _ => error(stream, 404, "not found"),
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
    app.turns.start(&id, &project.workdir, prompt)?;
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
    match app.turns.start(id, &p.workdir, content) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
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
