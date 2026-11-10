//! The JSON API (contract: `docs/api.md`) and the UI's pages.
//!
//! A thin dispatcher: each route points at the module that owns it. The
//! session, project and folder handlers live in `session_routes`.

pub(super) mod agent_routes;
pub(super) mod agents_md_routes;
pub(super) mod audit_routes;
pub(super) mod backlog_routes;
pub(super) mod budget_routes;
pub(super) mod canvas_routes;
pub(super) mod changelog_routes;
pub(super) mod contribution_routes;
pub(super) mod diff_routes;
pub(super) mod folder_routes;
pub(super) mod home_routes;
pub(super) mod idea_routes;
pub(super) mod indicator_routes;
pub(super) mod integration;
pub(super) mod mcp_oauth_routes;
pub(super) mod mcp_routes;
pub(super) mod model_routes;
pub(super) mod plugin_routes;
pub(super) mod preview;
pub(super) mod project_delete;
pub(super) mod project_list_routes;
pub(super) mod project_routes;
pub(super) mod proposal_routes;
pub(super) mod rule_approval_routes;
pub(super) mod rule_bridge_routes;
pub(super) mod rule_routes;
pub(super) mod server_routes;
pub(super) mod session_delete;
pub(super) mod session_routes;
pub(super) mod settings_routes;
pub(super) mod setup_routes;
pub(super) mod update_routes;
pub(super) mod waiver_routes;
pub(super) mod workflow_routes;
pub(super) mod worktree_routes;

use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send, send_json};
use crate::web::{
    assets, os::file_manager, os::servers, os::terminal, streams::event_stream,
    views::project_dashboard,
};
use serde_json::json;
use std::net::TcpStream;
use std::sync::Arc;

pub fn handle(app: &Arc<App>, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
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
        ("GET", ["api", "version"]) => send_json(
            stream,
            200,
            &json!({
                "version": env!("CARGO_PKG_VERSION"),
                "build": crate::update::BUILD,
                "changelogUrl": crate::update::CHANGELOG_URL,
            }),
        ),
        ("GET", ["api", "settings"]) => settings_routes::get_settings(app, stream),
        ("GET", ["api", "setup"]) => setup_routes::serve(app, stream),
        ("GET", ["api", "changelog"]) => changelog_routes::serve(app, stream),
        ("POST", ["api", "changelog", "seen"]) => changelog_routes::mark_seen(app, stream),
        ("GET", ["api", "insight"]) => crate::web::views::insight::serve(app, req, stream),
        ("GET", ["api", "notices"]) => crate::web::streams::notice_stream::serve(app, stream),
        ("POST", ["api", "settings"]) => settings_routes::save_settings(app, req, stream),
        ("POST", ["api", "settings", "providers", "list"]) => {
            settings_routes::list_provider(app, req, stream)
        }
        ("POST", ["api", "settings", "models", "probe"]) => {
            settings_routes::probe_model(app, req, stream)
        }
        ("POST", ["api", "settings", "mcp", "connect"]) => mcp_routes::connect(app, req, stream),
        ("POST", ["api", "mcp"]) => mcp_routes::draft_mcp(app, req, stream),
        ("POST", ["api", "settings", "mcp", "authorize"]) => {
            mcp_oauth_routes::authorize(app, req, stream)
        }
        ("GET", ["api", "mcp", "oauth", "callback"]) => {
            mcp_oauth_routes::callback(app, req, stream)
        }
        ("GET", ["api", "models", "choices"]) => model_routes::choices(app, stream),
        ("GET", ["api", "folders"]) => folder_routes::list_folders(app, req, stream),
        ("POST", ["api", "folders", "create"]) => folder_routes::create_folder(req, stream),
        ("GET", ["api", "projects"]) => project_list_routes::list_projects(app, stream),
        ("GET", ["api", "home"]) => home_routes::serve(app, stream),
        ("GET", ["api", "backlog"]) => backlog_routes::list_backlog(app, stream),
        ("POST", ["api", "projects"]) => project_list_routes::open_project(app, req, stream),
        ("GET", ["api", "projects", id, "commands"]) => {
            project_list_routes::list_commands(app, stream, id)
        }
        ("GET", ["api", "projects", id, "agents"]) => {
            project_list_routes::list_agents(app, stream, id)
        }
        ("GET", ["api", "projects", id, "overview"]) => {
            project_routes::get_overview(app, stream, id)
        }
        ("GET", ["api", "projects", id, "dashboard"]) => project_dashboard::serve(app, stream, id),
        ("GET", ["api", "projects", id, "docs"]) => project_routes::get_docs(app, stream, id),
        ("GET", ["api", "projects", id, "risks"]) => project_routes::get_risks(app, stream, id),
        ("GET", ["api", "projects", id, "worktrees"]) => {
            worktree_routes::list(app, req, stream, id)
        }
        ("GET", ["api", "server"]) => server_routes::show(app, stream),
        ("POST", ["api", "server"]) => server_routes::set(app, req, stream),
        ("POST", ["api", "server", "password"]) => server_routes::set_password(app, req, stream),
        ("POST", ["api", "server", "restart"]) => server_routes::restart(app, stream),
        ("GET", ["api", "update"]) => update_routes::check(app, req, stream),
        ("POST", ["api", "update"]) => update_routes::apply(app, stream),
        ("GET", ["api", "worktrees"]) => worktree_routes::summary(app, stream),
        ("POST", ["api", "worktrees", "clean"]) => worktree_routes::clean(app, req, stream),
        ("POST", ["api", "worktrees", "remove-orphan"]) => {
            worktree_routes::remove_orphan(app, req, stream)
        }
        ("GET", ["api", "agents"]) => agent_routes::list(app, req, stream),
        ("POST", ["api", "agents"]) => agent_routes::save(app, req, stream),
        ("GET", ["api", "agents", id]) => agent_routes::get(stream, id),
        ("POST", ["api", "agents", id, "delete"]) => agent_routes::delete(app, req, stream, id),
        ("GET", ["api", "rules"]) => rule_routes::list(app, stream),
        ("POST", ["api", "rules"]) => rule_routes::add(app, req, stream),
        ("POST", ["api", "rules", id, "delete"]) => rule_routes::delete(app, stream, id),
        ("GET", ["api", "projects", id, "rules"]) => rule_routes::list_project(app, stream, id),
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
        ("POST", ["api", "projects", id, "delete"]) => {
            project_delete::delete_project(app, req, stream, id)
        }
        ("POST", ["api", "projects", id, "favorite"]) => {
            project_list_routes::set_favorite(app, req, stream, id)
        }
        ("GET", ["api", "projects", id, "budget"]) => budget_routes::get(app, stream, id),
        ("POST", ["api", "projects", id, "budget"]) => budget_routes::set(app, req, stream, id),
        ("GET", ["api", "projects", id, "indicators"]) => indicator_routes::serve(app, stream, id),
        ("GET", ["api", "projects", id, "contributions"]) => {
            contribution_routes::list(app, stream, id)
        }
        ("POST", ["api", "projects", id, "contributions"]) => {
            contribution_routes::adopt(app, req, stream, id)
        }
        ("GET", ["api", "plugins"]) => plugin_routes::list(app, stream),
        ("POST", ["api", "plugins"]) => plugin_routes::set(app, req, stream),
        ("GET", ["api", "plugins", "status-bar"]) => plugin_routes::status_bar(app, req, stream),
        ("GET", ["api", "sessions", id, "plugins"]) => {
            plugin_routes::session_blocks(app, stream, id)
        }
        ("GET", ["api", "projects", id, "plugins"]) => {
            plugin_routes::project_blocks(app, stream, id)
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
        ("GET", ["api", "sessions", id, "vision"]) => session_routes::serve_vision(app, stream, id),
        ("GET", ["api", "sessions", id, "thread"]) => session_routes::serve_thread(app, stream, id),
        ("GET", ["api", "sessions", id, "usage", "workflow"]) => {
            session_routes::serve_usage_workflow(app, stream, id)
        }
        ("GET", ["api", "sessions", id, "usage", "calls"]) => {
            session_routes::serve_usage_calls(app, stream, id)
        }
        ("POST", ["api", "sessions", id, "messages"]) => {
            session_routes::send_message(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "queue", queue_id, "cancel"]) => {
            session_routes::cancel_queued(app, stream, id, queue_id)
        }
        ("POST", ["api", "sessions", id, "rule-approval"]) => {
            rule_approval_routes::answer_rule_approval(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "waiver"]) => {
            waiver_routes::answer_waiver(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "replay"]) => {
            session_routes::replay_message(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "retry"]) => {
            session_routes::retry_session(app, stream, id)
        }
        ("POST", ["api", "sessions", id, "compact"]) => {
            session_routes::compact_session(app, stream, id)
        }
        ("POST", ["api", "sessions", id, "audit", "tasks"]) => {
            audit_routes::create_tasks(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "proposals", call_id, "accept"]) => {
            proposal_routes::accept(app, stream, id, call_id)
        }
        ("POST", ["api", "sessions", id, "agents-md"]) => agents_md_routes::write(app, stream, id),
        ("POST", ["api", "sessions", id, "architecture-rules"]) => {
            rule_bridge_routes::add(app, stream, id)
        }
        ("POST", ["api", "sessions", id, "mode"]) => {
            session_routes::set_session_mode(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "model"]) => model_routes::set_model(app, req, stream, id),
        ("GET", ["api", "sessions", id, "mcp"]) => mcp_routes::session_mcp(app, stream, id),
        ("POST", ["api", "sessions", id, "mcp"]) => {
            mcp_routes::set_session_mcp(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "workflow"]) => {
            workflow_routes::start_workflow(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "workflow", control]) => {
            workflow_routes::control_workflow(app, req, stream, id, control)
        }
        ("GET", ["api", "sessions", id, "integration"]) => {
            integration::get_integration(app, stream, id)
        }
        ("GET", ["api", "sessions", id, "diff"]) => diff_routes::get_diff(app, req, stream, id),
        ("POST", ["api", "sessions", id, "integration", "message"]) => {
            integration::suggest_message(app, stream, id)
        }
        ("GET", ["api", "sessions", id, "repo"]) => integration::get_repo(app, stream, id),
        ("POST", ["api", "sessions", id, "integrate"]) => {
            integration::integrate(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "discard"]) => integration::discard(app, req, stream, id),
        ("POST", ["api", "sessions", id, "resolve-conflict"]) => {
            integration::resolve_conflict(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "understanding"]) => {
            integration::start_understanding(app, req, stream, id)
        }
        ("POST", ["api", "sessions", id, "terminal"]) => terminal::open(app, stream, id),
        ("POST", ["api", "sessions", id, "files"]) => file_manager::open(app, stream, id),
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
        ("GET", ["api", "sessions", id, "canvas"]) => canvas_routes::list_session(app, stream, id),
        ("POST", ["api", "sessions", id, "canvas"]) => canvas_routes::create(app, req, stream, id),
        ("GET", ["api", "projects", id, "canvas"]) => canvas_routes::list_project(app, stream, id),
        ("GET", ["api", "canvas", id]) => canvas_routes::detail(app, stream, id),
        ("GET", ["api", "canvas", id, "versions", n]) => canvas_routes::content(app, stream, id, n),
        ("POST", ["api", "canvas", id, "delete"]) => canvas_routes::delete(app, stream, id),
        ("POST", ["api", "canvas", id, "annotations"]) => {
            canvas_routes::annotate(app, req, stream, id)
        }
        ("GET", ["api", "sessions", id, "preview", rest @ ..]) => {
            crate::web::routes::preview::serve(app, stream, id, rest)
        }
        ("GET", ["api", ..]) | ("POST", _) => error(stream, 404, "not found"),
        ("GET", _) => {
            let asset = assets::get(&req.path);
            send(stream, 200, asset.content_type, asset.body)
        }
        _ => error(stream, 404, "not found"),
    }
}
