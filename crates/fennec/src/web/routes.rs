//! The JSON API (contract: `docs/api.md`) and the UI's pages.
//!
//! A thin dispatcher: each route points at the module that owns it. The
//! session, project and folder handlers live in `session_routes`.

use super::App;
use super::audit_routes;
use super::http::{Request, error, send, send_json};
use super::{
    assets, backlog_routes, event_stream, idea_routes, integration, model_routes, project_routes,
    servers, session_delete, session_routes, settings_routes, terminal, workflow_routes,
};
use serde_json::json;
use std::net::TcpStream;
use std::sync::Arc;

pub fn handle(app: &Arc<App>, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let segments: Vec<&str> = req.path.trim_matches('/').split('/').collect();
    match (req.method.as_str(), segments.as_slice()) {
        ("GET", ["api", "config"]) => {
            let store = app.store()?;
            let prefs = store.preferences()?;
            let params = app.settings.llm.for_turn(&prefs, None);
            send_json(
                stream,
                200,
                &json!({ "model": params.model, "contextWindow": params.context_window, "compactAt": params.compact_at }),
            )
        }
        ("GET", ["api", "settings"]) => settings_routes::get_settings(app, stream),
        ("GET", ["api", "insight"]) => super::insight::serve(app, req, stream),
        ("GET", ["api", "notices"]) => super::notice_stream::serve(app, stream),
        ("POST", ["api", "settings"]) => settings_routes::save_settings(app, req, stream),
        ("POST", ["api", "settings", "providers", "list"]) => {
            settings_routes::list_provider(app, req, stream)
        }
        ("POST", ["api", "settings", "models", "probe"]) => {
            settings_routes::probe_model(app, req, stream)
        }
        ("GET", ["api", "models", "choices"]) => model_routes::choices(app, stream),
        ("GET", ["api", "folders"]) => session_routes::list_folders(req, stream),
        ("POST", ["api", "folders", "create"]) => session_routes::create_folder(req, stream),
        ("GET", ["api", "projects"]) => session_routes::list_projects(app, stream),
        ("GET", ["api", "backlog"]) => backlog_routes::list_backlog(app, stream),
        ("POST", ["api", "projects"]) => session_routes::open_project(app, req, stream),
        ("GET", ["api", "projects", id, "commands"]) => {
            session_routes::list_commands(app, stream, id)
        }
        ("GET", ["api", "projects", id, "agents"]) => session_routes::list_agents(app, stream, id),
        ("GET", ["api", "projects", id, "overview"]) => {
            project_routes::get_overview(app, stream, id)
        }
        ("GET", ["api", "projects", id, "docs"]) => project_routes::get_docs(app, stream, id),
        ("GET", ["api", "projects", id, "ideas"]) => idea_routes::list_ideas(app, stream, id),
        ("POST", ["api", "projects", id, "ideas"]) => idea_routes::add_idea(app, req, stream, id),
        ("POST", ["api", "projects", id, "ideas", "queue", "start"]) => {
            idea_routes::start_queue(app, req, stream, id)
        }
        ("POST", ["api", "projects", id, "ideas", "queue", "stop"]) => {
            idea_routes::stop_queue(app, stream, id)
        }
        ("POST", ["api", "projects", id, "ideas", "reorder"]) => {
            idea_routes::reorder(app, req, stream, id)
        }
        ("POST", ["api", "projects", id, "ideas", "report", "read"]) => {
            idea_routes::report_read(app, stream, id)
        }
        ("POST", ["api", "projects", id, "ideas", iid, "start"]) => {
            idea_routes::start_one(app, stream, id, iid)
        }
        ("POST", ["api", "projects", id, "ideas", iid, action]) => {
            idea_routes::update_idea(app, req, stream, id, iid, action)
        }
        ("POST", ["api", "projects", id, "map-file"]) => {
            project_routes::write_map_file(app, stream, id)
        }
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
        ("POST", ["api", "sessions"]) => session_routes::create_session(app, req, stream),
        ("POST", ["api", "sessions", "delete"]) => {
            session_delete::delete_sessions(app, req, stream)
        }
        ("GET", ["api", "sessions", id]) => session_routes::get_session(app, stream, id),
        ("GET", ["api", "sessions", id, "skills"]) => session_routes::serve_skills(app, stream, id),
        ("GET", ["api", "sessions", id, "usage"]) => session_routes::serve_usage(app, stream, id),
        ("GET", ["api", "sessions", id, "context"]) => {
            session_routes::serve_context(app, stream, id)
        }
        ("GET", ["api", "sessions", id, "usage", "workflow"]) => {
            session_routes::serve_usage_workflow(app, stream, id)
        }
        ("GET", ["api", "sessions", id, "usage", "calls"]) => {
            session_routes::serve_usage_calls(app, stream, id)
        }
        ("POST", ["api", "sessions", id, "messages"]) => {
            session_routes::send_message(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "replay"]) => {
            session_routes::replay_message(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "compact"]) => {
            session_routes::compact_session(app, stream, id)
        }
        ("POST", ["api", "sessions", id, "audit", "tasks"]) => {
            audit_routes::create_tasks(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "mode"]) => {
            session_routes::set_session_mode(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "model"]) => model_routes::set_model(app, req, stream, id),
        ("POST", ["api", "sessions", id, "workflow"]) => {
            workflow_routes::start_workflow(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "workflow", control]) => {
            workflow_routes::control_workflow(app, req, stream, id, control)
        }
        ("GET", ["api", "sessions", id, "integration"]) => {
            integration::get_integration(app, stream, id)
        }
        ("GET", ["api", "sessions", id, "repo"]) => integration::get_repo(app, stream, id),
        ("POST", ["api", "sessions", id, "integrate"]) => {
            integration::integrate(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "discard"]) => integration::discard(app, req, stream, id),
        ("POST", ["api", "sessions", id, "resolve-conflict"]) => {
            integration::resolve_conflict(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "terminal"]) => terminal::open(app, stream, id),
        ("GET", ["api", "sessions", id, "servers"]) => servers::list(app, stream, id),
        ("POST", ["api", "sessions", id, "servers"]) => servers::save(app, req, stream, id),
        ("POST", ["api", "sessions", id, "servers", "start"]) => {
            servers::start(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "servers", "stop"]) => servers::stop(app, req, stream, id),
        ("POST", ["api", "sessions", id, "stop"]) => {
            app.turns.stop(id);
            send_json(stream, 202, &json!({}))
        }
        ("GET", ["api", "sessions", id, "events"]) => {
            if req.query_param("limit").is_some() {
                session_routes::serve_events_page(app, req, stream, id)
            } else {
                event_stream::serve(&app.store()?, &app.notifier, req, stream, id)
            }
        }
        ("GET", ["api", "sessions", id, "workflow", "run"]) => {
            session_routes::serve_run_events(app, req, stream, id)
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
