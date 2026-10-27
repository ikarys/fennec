//! The JSON API (contract: `docs/api.md`) and the UI's pages.

use super::App;
use super::editor;
use super::http::{Request, error, send, send_json};
use super::notifier::NotifyingLog;
use super::{assets, event_stream, integration, session_name, turns::Job, workflow_routes};
use crate::agent::ports::EventLog;
use crate::agent::workflow::WorkflowJob;
use crate::agents;
use crate::domain::agent::{resolve as agent_resolve, switch_to};
use crate::domain::attachment::{Attachment, from_data_url};
use crate::domain::event::Event;
use crate::domain::id::new_id;
use crate::domain::preferences::Preferences;
use crate::domain::project::{Project, SessionSummary};
use crate::domain::replay::replayable;
use crate::domain::slash::{TurnInput, commands, resolve};
use crate::domain::workflow::run::Waiting;
use crate::llm::list_models;
use crate::skills;
use crate::store::SqliteStore;
use crate::{folders, git, workflows};
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
        ("GET", ["api", "projects", id, "agents"]) => list_agents(app, stream, id),
        ("GET", ["api", "projects", id, "workflows"]) => {
            workflow_routes::list_workflows(app, stream, id)
        }
        ("GET", ["api", "projects", id, "workflows", wid]) => {
            workflow_routes::get_workflow(app, stream, id, wid)
        }
        ("PUT", ["api", "projects", id, "workflows", wid]) => {
            workflow_routes::save_workflow(app, req, stream, id, wid)
        }
        ("POST", ["api", "workflows", "validate"]) => {
            workflow_routes::validate_workflow_route(app, req, stream)
        }
        ("POST", ["api", "sessions"]) => create_session(app, req, stream),
        ("GET", ["api", "sessions", id]) => get_session(app, stream, id),
        ("GET", ["api", "sessions", id, "skills"]) => serve_skills(app, stream, id),
        ("POST", ["api", "sessions", id, "messages"]) => send_message(app, req, stream, id),
        ("POST", ["api", "sessions", id, "replay"]) => replay_message(app, req, stream, id),
        ("POST", ["api", "sessions", id, "compact"]) => compact_session(app, stream, id),
        ("POST", ["api", "sessions", id, "mode"]) => set_session_mode(app, req, stream, id),
        ("POST", ["api", "sessions", id, "workflow"]) => {
            workflow_routes::start_workflow(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "workflow", control]) => {
            workflow_routes::control_workflow(app, req, stream, id, control)
        }
        ("GET", ["api", "sessions", id, "integration"]) => {
            integration::get_integration(app, stream, id)
        }
        ("POST", ["api", "sessions", id, "integrate"]) => {
            integration::integrate(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "discard"]) => integration::discard(app, req, stream, id),
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
        ("GET", ["api", "sessions", id, "workflow", "run"]) => {
            serve_run_events(app, req, stream, id)
        }
        ("GET", ["api", "sessions", id, "preview", rest @ ..]) => {
            super::preview::serve(app, stream, id, rest)
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

fn list_agents(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let list = agents::discover(Path::new(&project.workdir));
    let out: Vec<Value> = list
        .iter()
        .map(|agent| {
            json!({
                "id": agent.id,
                "name": agent.name,
                "description": agent.description,
                "color": agent.color,
                "subagent": agent.subagent,
            })
        })
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

pub(super) fn parse_attachments(body: &Value) -> Result<Vec<Attachment>, String> {
    let Some(attachments) = body.get("attachments").filter(|v| !v.is_null()) else {
        return Ok(Vec::new());
    };
    let Some(items) = attachments.as_array() else {
        return Err("attachments must be an array".into());
    };
    if items.len() > 10 {
        return Err("at most 10 attachments".into());
    }
    let mut out = Vec::new();
    for item in items {
        let filename = item["filename"].as_str().unwrap_or("").to_string();
        let data = item["data"].as_str().unwrap_or("");
        out.push(from_data_url(new_id(), filename, data)?);
    }
    Ok(out)
}

fn create_session(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let prompt = body["prompt"].as_str().unwrap_or("").trim().to_string();
    let attachments = match parse_attachments(&body) {
        Ok(attachments) => attachments,
        Err(e) => return error(stream, 400, &e),
    };
    let workflow_id = body
        .get("workflowId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let step_by_step = body["stepByStep"].as_bool().unwrap_or(false);
    let worktree = body["worktree"].as_bool().unwrap_or(true);
    if prompt.is_empty() && attachments.is_empty() && workflow_id.is_none() {
        return error(stream, 400, "empty prompt");
    }
    let store = app.store()?;
    let Some(project) = store.project(body["projectId"].as_str().unwrap_or(""))? else {
        return error(stream, 404, "no such project");
    };
    // A workflow start is validated before the session is created.
    let workflow = match &workflow_id {
        Some(id) => {
            let Some(found) = workflows::find(Path::new(&project.workdir), id) else {
                return error(stream, 404, "no such workflow");
            };
            Some(found)
        }
        None => None,
    };
    let mode = body.get("mode").and_then(Value::as_str).map(str::to_string);
    let mode_event = match &mode {
        Some(mode) => {
            let list = agents::discover(Path::new(&project.workdir));
            match switch_to(&list, &[], mode) {
                Ok(event) => event,
                Err(e) => return error(stream, 400, &e),
            }
        }
        None => None,
    };
    let title: String = if prompt.is_empty() {
        attachments
            .first()
            .map(|attachment| attachment.filename.clone())
            .unwrap_or_default()
    } else {
        prompt.chars().take(80).collect()
    };
    let id = store.create_session(&project, &title)?;
    if !prompt.is_empty() {
        session_name::spawn(
            app.settings.clone(),
            app.notifier.clone(),
            id.clone(),
            prompt.clone(),
        );
    }
    append_mode_event(app, &store, &id, mode_event)?;
    let session = store.session(&id)?.ok_or("the new session vanished")?;
    match workflow {
        Some(found) => {
            let job = Job::Workflow(WorkflowJob::Start {
                workflow: found.workflow,
                task: prompt,
                step_by_step,
                attachments,
            });
            let place = app.open_place(&store, &session, &project, worktree)?;
            app.turns.start(&id, place, job)?;
        }
        None => {
            let input = resolve(&prompt, &skills::discover(Path::new(&project.workdir)));
            if matches!(input, TurnInput::Compact) && !attachments.is_empty() {
                return error(stream, 400, "/compact takes no attachments");
            }
            let place = app.place(&store, &session, &project)?;
            app.turns
                .start(&id, place, Job::Input(input, attachments))?;
        }
    }
    send_json(stream, 200, &json!({ "id": id }))
}

fn get_session(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some((s, p)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    let events = store.events(&s.id)?;
    let list = agents::discover(Path::new(&p.workdir));
    let mode = agent_resolve(&list, &events).map(|agent| agent.id.clone());
    let mut v = session_json(app, &s);
    v["project"] = project_json(&p);
    v["mode"] = json!(mode);
    v["worktree"] = match app.place(&store, &s, &p)?.worktree {
        Some(wt) => json!({
            "path": wt.path, "branch": wt.branch, "base": wt.base,
            "editorUrl": editor::vscode_link(
                &wt.path.to_string_lossy(),
                std::env::var("WSL_DISTRO_NAME").ok().as_deref(),
            ),
        }),
        None => Value::Null,
    };
    send_json(stream, 200, &v)
}

/// The session's skills folded from its log: loaded so far, and those
/// still in the model's view (not dropped by a compaction, not cut by a
/// replay). `GET /api/sessions/{id}/skills`.
fn serve_skills(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some((_, p)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    let skills = skills::discover(Path::new(&p.workdir));
    let folded = app.folded_skills(&store, id, &skills)?;
    send_json(
        stream,
        200,
        &json!({ "loaded": folded.loaded, "active": folded.active }),
    )
}

fn set_session_mode(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let mode = body["mode"].as_str().unwrap_or("").to_string();
    let store = app.store()?;
    let Some((_, p)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    if workflow_routes::waiting_for(app, id)?.is_some() {
        return error(stream, 409, "a workflow is running in this session");
    }
    let list = agents::discover(Path::new(&p.workdir));
    let events = store.events(id)?;
    let event = match switch_to(&list, &events, &mode) {
        Ok(event) => event,
        Err(e) => return error(stream, 400, &e),
    };
    append_mode_event(app, &store, id, event)?;
    send_json(stream, 200, &json!({ "mode": mode }))
}

fn send_message(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let body = req.json()?;
    let content = body["content"].as_str().unwrap_or("").trim().to_string();
    let attachments = match parse_attachments(&body) {
        Ok(attachments) => attachments,
        Err(e) => return error(stream, 400, &e),
    };
    if content.is_empty() && attachments.is_empty() {
        return error(stream, 400, "empty message");
    }
    let store = app.store()?;
    let Some((session, p)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let place = app.place(&store, &session, &p)?;
    // While a run is active the typed message answers where it waits.
    if let Some(waiting) = workflow_routes::waiting_for(app, id)? {
        return match waiting {
            Waiting::AgentQuestion => {
                let job = Job::Workflow(WorkflowJob::AgentInput {
                    prompt: content,
                    attachments,
                });
                match app.turns.start(id, place, job) {
                    Ok(()) => send_json(stream, 202, &json!({})),
                    Err(e) => error(stream, 409, &e),
                }
            }
            Waiting::Human => error(stream, 409, "the workflow waits for a choice"),
            Waiting::Pause => error(stream, 409, "the workflow is paused: continue or stop it"),
            Waiting::Interrupted => error(
                stream,
                409,
                "the workflow was interrupted: resume or stop it",
            ),
        };
    }
    let input = resolve(&content, &skills::discover(Path::new(&p.workdir)));
    if matches!(input, TurnInput::Compact) && !attachments.is_empty() {
        return error(stream, 400, "/compact takes no attachments");
    }
    match app.turns.start(id, place, Job::Input(input, attachments)) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
}

fn replay_message(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let message_id = body["messageId"].as_str().unwrap_or("").to_string();
    let store = app.store()?;
    let Some((session, p)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let events = store.events(id)?;
    if replayable(&events, &message_id).is_none() {
        return error(
            stream,
            400,
            "only a message you typed in the current window can be replayed",
        );
    }
    let place = app.place(&store, &session, &p)?;
    match app.turns.start(id, place, Job::Replay(message_id)) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
}

fn compact_session(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some((session, p)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    if workflow_routes::waiting_for(app, id)?.is_some() {
        return error(stream, 409, "a workflow is running in this session");
    }
    let place = app.place(&store, &session, &p)?;
    match app
        .turns
        .start(id, place, Job::Input(TurnInput::Compact, Vec::new()))
    {
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

/// The `workflow.*` events of the latest run started before `before`
/// (the log's end when omitted), ascending.
/// `GET /api/sessions/{id}/workflow/run?before=<seq>`.
fn serve_run_events(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let before = req
        .query_param("before")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(u64::MAX);
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let page = store.workflow_run_before(id, before)?;
    let out: Vec<Value> = page
        .into_iter()
        .map(|(seq, event)| json!({ "seq": seq, "event": event }))
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

pub(super) fn session_and_project(
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
        "createdAt": s.created_at, "updatedAt": s.updated_at, "parentId": s.parent_id,
    })
}

fn project_json(p: &Project) -> Value {
    json!({ "id": p.id, "name": p.name, "workdir": p.workdir })
}

fn append_mode_event(
    app: &App,
    store: &SqliteStore,
    session_id: &str,
    event: Option<Event>,
) -> Result<(), String> {
    if let Some(event) = event {
        let log = NotifyingLog {
            log: store,
            notifier: &app.notifier,
        };
        log.append(session_id, &event)?;
    }
    Ok(())
}
