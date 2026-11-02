//! The session, project and folder handlers the dispatcher in `routes` routes
//! to. Kept in their own module so `routes` stays a thin dispatcher.

use super::App;
use super::editor;
use super::http::{Request, error, send_json};
use super::{
    context_view::context_view,
    model_routes, session_name,
    session_usage::{SessionUsageView, call_views},
    turns::Job,
    workflow_routes, workflow_usage,
};
use crate::agent::ports::EventLog;
use crate::agent::workflow::WorkflowJob;
use crate::agents;
use crate::chat;
use crate::domain::agent::{
    resolve as agent_resolve, resolve_mode as agent_resolve_mode, switch_to,
};
use crate::domain::agent_suggestion;
use crate::domain::agents_md;
use crate::domain::attachment::{Attachment, from_data_url};
use crate::domain::audit;
use crate::domain::clock;
use crate::domain::conversation::{first_request, last_assistant_text};
use crate::domain::event::{Event, QueueMode};
use crate::domain::id::new_id;
use crate::domain::preferences::ModelRef;
use crate::domain::project::{Project, SessionSummary};
use crate::domain::queue;
use crate::domain::replay::replayable;
use crate::domain::session_skills::missed_always_on;
use crate::domain::slash::{TurnInput, commands, resolve};
use crate::domain::task_proposal;
use crate::domain::vision::seen_images;
use crate::domain::workflow::run::Waiting;
use crate::notifier::NotifyingLog;
use crate::skills;
use crate::store::SqliteStore;
use crate::{folders, git, workflows};
use backlog::idea_for_session;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

pub(super) fn list_folders(req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let dir = req
        .query_param("path")
        .filter(|p| !p.is_empty())
        .map_or_else(folders::home, Into::into);
    match folders::list(&dir) {
        Ok(listing) => send_json(stream, 200, &json!(listing)),
        Err(e) => error(stream, 400, &e),
    }
}

pub(super) fn create_folder(req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let base = body["path"].as_str().unwrap_or("");
    let name = body["name"].as_str().unwrap_or("");
    let base = if base.is_empty() {
        folders::home()
    } else {
        Path::new(base).into()
    };
    match folders::create(&base, name) {
        Ok(path) => send_json(stream, 200, &json!({ "path": path.display().to_string() })),
        Err(e) => error(stream, 400, &e),
    }
}

pub(super) fn list_projects(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    let mut out = Vec::new();
    for p in store.projects()? {
        // The log says a workflow ended; git says whether its work still waits.
        let pending = git::sessions_with_unintegrated_work(Path::new(&p.workdir));
        // Each session's agent, so the project page can tell an analysis apart.
        let list = agents::discover(Path::new(&p.workdir));
        let sessions: Vec<Value> = store
            .sessions(Some(&p.id))?
            .iter()
            .map(|s| {
                let mut v = session_json(app, s);
                v["awaitingIntegration"] = json!(s.awaiting_integration && pending.contains(&s.id));
                let mode = store.last_mode(&s.id).ok().flatten().and_then(|m| {
                    agent_resolve_mode(&list, Some(m.as_str())).map(|a| a.id.clone())
                });
                v["mode"] = json!(mode);
                v
            })
            .collect();
        let mut v = project_json(&p);
        v["branch"] = json!(git::current_branch(Path::new(&p.workdir)));
        v["remote"] = json!(
            git::remote_url(Path::new(&p.workdir))
                .as_deref()
                .and_then(git::web_url)
        );
        v["editorUrl"] = json!(editor::vscode_link(
            &p.workdir,
            std::env::var("WSL_DISTRO_NAME").ok().as_deref(),
        ));
        v["sessions"] = Value::Array(sessions);
        out.push(v);
    }
    send_json(stream, 200, &Value::Array(out))
}

pub(super) fn open_project(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let body = req.json()?;
    let dir = Path::new(body["workdir"].as_str().unwrap_or(""));
    let Some(dir) = dir.canonicalize().ok().filter(|d| d.is_dir()) else {
        return error(stream, 400, "not a directory");
    };
    let p = app.store()?.project_for(&dir.display().to_string())?;
    send_json(stream, 200, &project_json(&p))
}

pub(super) fn list_commands(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
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

pub(super) fn list_agents(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let mut list = agents::discover(Path::new(&project.workdir));
    // The menu follows a project's order, so phase comes first.
    list.sort_by_key(|agent| (agent.phase.is_none(), agent.phase));
    let out: Vec<Value> = list
        .iter()
        .map(|agent| {
            json!({
                "id": agent.id,
                "name": agent.name,
                "description": agent.description,
                "color": agent.color,
                "subagent": agent.subagent,
                "phase": agent.phase,
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

pub(super) fn create_session(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
) -> Result<(), String> {
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
    let project = if body["projectId"].as_str().unwrap_or("") == chat::ID {
        // The chat's scratch project: its per-session scratch directory is
        // created only when a tool first needs it.
        let dir = chat::workdir(&app.settings.db_path);
        store.chat_project(&dir.display().to_string())?
    } else {
        let Some(project) = store.project(body["projectId"].as_str().unwrap_or(""))? else {
            return error(stream, 404, "no such project");
        };
        project
    };
    // A workflow start is validated before the session is created.
    let workflow = match &workflow_id {
        Some(id) => {
            let enabled = &app.store()?.preferences()?.enabled_workflows;
            let Some(found) = workflows::find(Path::new(&project.workdir), id, enabled) else {
                return error(stream, 404, "no such workflow");
            };
            Some(found)
        }
        None => None,
    };
    let prefs = store.preferences()?;
    let model_ref = match body.get("model") {
        Some(v) if !v.is_null() => {
            let provider = v["provider"].as_str().unwrap_or("").to_string();
            let model = v["model"].as_str().unwrap_or("").to_string();
            if model.is_empty() {
                return error(stream, 400, "model is empty");
            }
            if app.settings.endpoint(&prefs, &provider).is_err() {
                return error(stream, 400, "unknown provider");
            }
            Some(ModelRef { provider, model })
        }
        _ => None,
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
    append_human_event(app, &store, &id, mode_event)?;
    if let Some(model) = model_ref {
        append_human_event(app, &store, &id, Some(Event::ModelChanged { model }))?;
    }
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

pub(super) fn get_session(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some((s, p)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let events = store.events(&s.id)?;
    let list = agents::discover(Path::new(&p.workdir));
    let mode = agent_resolve(&list, &events).map(|agent| agent.id.clone());
    let mut v = session_json(app, &s);
    v["project"] = project_json(&p);
    v["mode"] = json!(mode);
    v["model"] = model_routes::session_model(app, &store, &s.id)?;
    // The idea this session frames, so its plan card can mark it ready.
    v["idea"] = match idea_for_session(&store.ideas(&p.id)?, &s.id) {
        Some(idea) => json!({ "id": idea.id, "state": idea.state }),
        None => Value::Null,
    };
    // The first human request, so the context panel can show what was asked.
    v["request"] = json!(first_request(&events));
    // The audit report of the last answer, if it is one: the context panel lists its findings.
    let mut audit_json = json!(audit::parse(&last_assistant_text(&events)));
    if audit_json.is_object() {
        audit_json["taken"] = json!(audit::taken(&events));
    }
    v["audit"] = audit_json;
    // The agent's proposed tasks, with the idea id if the human accepted one.
    v["proposedTasks"] = json!(task_proposal::proposed_tasks(&events));
    // The last answer's AGENTS.md draft, if any, and whether the file exists / was written.
    v["agentsMdDraft"] = match agents_md::draft(&last_assistant_text(&events)) {
        Some(_) => json!({
            "exists": Path::new(&p.workdir).join("AGENTS.md").exists(),
            "written": agents_md::written_since_last_answer(&events),
        }),
        None => Value::Null,
    };
    // The agent switch suggested in this turn, with the target's display name; dropped if that agent is gone.
    v["agentSuggestion"] = agent_suggestion::last_suggestion(&events)
        .and_then(|s| {
            let target = list.iter().find(|a| a.id == s.agent && !a.subagent)?;
            Some(json!({
                "callId": s.call_id,
                "agent": s.agent,
                "name": target.name,
                "color": target.color,
                "reason": s.reason,
                "brief": s.brief,
            }))
        })
        .unwrap_or(Value::Null);
    v["worktree"] = match app.place(&store, &s, &p)?.worktree {
        Some(wt) => json!({
            "path": wt.path, "branch": wt.branch, "base": wt.base,
            // What the integration would bring: none means nothing to review.
            "commits": wt.branch.strip_prefix("fennec/")
                .and_then(|id| git::branch_stats(Path::new(&p.workdir), id))
                .map_or(0, |stats| stats.commits),
            "editorUrl": editor::vscode_link(
                &wt.path.to_string_lossy(),
                std::env::var("WSL_DISTRO_NAME").ok().as_deref(),
            ),
        }),
        None => Value::Null,
    };
    send_json(stream, 200, &v)
}

/// The session's skills folded from its log: loaded so far, those frozen
/// into its prompt as always-on, those still in the model's view, and the
/// always-on ones chosen after its prompt froze. `GET /api/sessions/{id}/skills`.
pub(super) fn serve_skills(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some((_, p)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let skills = skills::discover(Path::new(&p.workdir));
    let folded = app.folded_skills(&store, id, &skills)?;
    let chosen = store.preferences()?.always_on_skills;
    let missed = missed_always_on(&folded, &chosen, &skills);
    send_json(
        stream,
        200,
        &json!({
            "loaded": folded.loaded,
            "active": folded.active,
            "alwaysOn": folded.always_on,
            "missedAlwaysOn": missed
        }),
    )
}

/// The session's model calls from the usage ledger, which outlives the log.
/// `GET /api/sessions/{id}/usage`.
/// The header's gauge; events from before `compactAt` was recorded fall back
/// to the current settings.
pub(super) fn serve_context(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let (chosen, _) = model_routes::session_choice(&store, id)?;
    let params = app
        .settings
        .llm
        .for_turn(&store.preferences()?, chosen.as_ref());
    let view = context_view(&store.events(id)?, params.context_window, params.compact_at);
    send_json(stream, 200, &json!(view))
}

pub(super) fn serve_vision(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    send_json(stream, 200, &json!(seen_images(&store.events(id)?)))
}

pub(super) fn serve_usage(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let ledger =
        token_usage::Ledger::open(&app.settings.usage_path()).map_err(|e| e.to_string())?;
    let filter = token_usage::Filter {
        run_id: Some(id.to_string()),
        ..Default::default()
    };
    let summary = ledger.summary(&filter).map_err(|e| e.to_string())?;
    let latest = ledger.latest(&filter).map_err(|e| e.to_string())?;
    let rows = ledger
        .series(&filter, token_usage::GroupBy::Model)
        .map_err(|e| e.to_string())?;
    let pricing = super::pricing::priced(&rows, &super::pricing::prices(&store.preferences()?));
    send_json(
        stream,
        200,
        &json!(SessionUsageView::new(&summary, latest.as_ref(), &pricing)),
    )
}

pub(super) fn serve_usage_calls(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let events = store.events(id)?;
    let ledger =
        token_usage::Ledger::open(&app.settings.usage_path()).map_err(|e| e.to_string())?;
    let filter = token_usage::Filter {
        run_id: Some(id.to_string()),
        ..Default::default()
    };
    let calls = ledger.calls(&filter).map_err(|e| e.to_string())?;
    send_json(stream, 200, &json!(call_views(&events, &calls)))
}

pub(super) fn serve_usage_workflow(
    app: &App,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let events = store.events(id)?;
    let ledger =
        token_usage::Ledger::open(&app.settings.usage_path()).map_err(|e| e.to_string())?;
    let mut calls = std::collections::HashMap::new();
    for session in std::iter::once(id.to_string()).chain(workflow_usage::child_ids(&events)) {
        let filter = token_usage::Filter {
            run_id: Some(session.clone()),
            ..Default::default()
        };
        calls.insert(session, ledger.calls(&filter).map_err(|e| e.to_string())?);
    }
    send_json(
        stream,
        200,
        &json!(workflow_usage::workflow_usage(id, &events, &calls)),
    )
}

pub(super) fn set_session_mode(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let mode = body["mode"].as_str().unwrap_or("").to_string();
    let store = app.store()?;
    let Some((_, p)) = app.session_and_project(id)? else {
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
    append_human_event(app, &store, id, event)?;
    send_json(stream, 200, &json!({ "mode": mode }))
}

pub(super) fn send_message(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
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
    let Some((session, p)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    if app.turns.is_running(id) {
        if !attachments.is_empty() {
            return error(stream, 409, "images wait for the end of the turn");
        }
        let queue_id = new_id();
        match app.turns.enqueue(id, || {
            NotifyingLog {
                log: &store,
                notifier: &app.notifier,
            }
            .append(
                id,
                &Event::QueueAdded {
                    queue_id: queue_id.clone(),
                    mode: QueueMode::Asap,
                    content: content.clone(),
                    queued_at: clock::now_ms(),
                },
            )
            .map(|_| ())
        }) {
            Ok(true) => return send_json(stream, 202, &json!({ "queued": queue_id })),
            Ok(false) => {}
            Err(e) => return error(stream, 409, &e),
        }
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

pub(super) fn cancel_queued(
    app: &App,
    stream: &mut TcpStream,
    id: &str,
    queue_id: &str,
) -> Result<(), String> {
    let store = app.store()?;
    if store.session(id)?.is_none() {
        return error(stream, 404, "no such session");
    }
    let events = store.events(id)?;
    if !queue::pending(&events)
        .iter()
        .any(|queued| queued.queue_id == queue_id)
    {
        return error(stream, 404, "no such queued message");
    }
    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        id,
        &Event::QueueCancelled {
            queue_id: queue_id.to_string(),
        },
    )?;
    send_json(stream, 200, &json!({}))
}

pub(super) fn replay_message(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let message_id = body["messageId"].as_str().unwrap_or("").to_string();
    let store = app.store()?;
    let Some((session, p)) = app.session_and_project(id)? else {
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

pub(super) fn compact_session(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some((session, p)) = app.session_and_project(id)? else {
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
pub(super) fn serve_events_page(
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
pub(super) fn serve_run_events(
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

fn session_json(app: &App, s: &SessionSummary) -> Value {
    json!({
        "id": s.id, "title": s.title, "state": s.state(app.turns.is_running(&s.id)),
        "createdAt": s.created_at, "updatedAt": s.updated_at, "parentId": s.parent_id,
        "awaitingIntegration": s.awaiting_integration,
    })
}

fn project_json(p: &Project) -> Value {
    json!({ "id": p.id, "name": p.name, "workdir": p.workdir, "chat": chat::is_chat(p) })
}

/// Appends a choice the human made (mode, model) and wakes the session's streams.
pub(super) fn append_human_event(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmSettings, Settings};
    use crate::domain::preferences::ModelParams;
    use crate::notifier::Notifier;
    use crate::web::turns::Turns;
    use std::collections::HashMap;
    use std::io::Read;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[test]
    fn project_json_marks_only_the_chat() {
        let chat = Project {
            id: chat::ID.into(),
            name: chat::NAME.into(),
            workdir: "/scratch".into(),
        };
        let other = Project {
            id: "other".into(),
            name: "o".into(),
            workdir: "/w".into(),
        };
        assert_eq!(project_json(&chat)["chat"], true);
        assert_eq!(project_json(&other)["chat"], false);
    }

    #[test]
    fn create_session_routes_chat_id_to_the_scratch_project() {
        // Its own store directory: the scratch root is `chat/` next to the
        // database, so a shared `/tmp/chat` would race with other tests.
        let base = std::env::temp_dir().join(format!("fennec-chat-route-{}", new_id()));
        std::fs::create_dir_all(&base).unwrap();
        let db_path = base.join("fennec.db");
        let scratch = chat::workdir(&db_path);
        let _ = std::fs::remove_dir_all(&scratch);
        let app = app(db_path.clone());
        let store = app.store().unwrap();

        let req = Request {
            method: "POST".into(),
            path: "/api/sessions".into(),
            query: String::new(),
            headers: Vec::new(),
            body: json!({ "prompt": "hello", "projectId": "chat" })
                .to_string()
                .into_bytes(),
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let mut server = listener.accept().unwrap().0;
        {
            let _client = &client;
            create_session(&app, &req, &mut server).unwrap();
        }

        // The chat project now exists with the scratch workdir and holds a session.
        let chat = store
            .project(chat::ID)
            .unwrap()
            .expect("chat project created");
        assert_eq!(
            chat.workdir,
            chat::workdir(&app.settings.db_path).display().to_string()
        );
        assert_eq!(store.sessions(Some(chat::ID)).unwrap().len(), 1);
        // Talking creates nothing on disk: the scratch appears only when a
        // tool first needs it.
        assert!(!scratch.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A file-backed store: `App::store` opens one connection per call, so an
    /// in-memory DB would be empty on every call.
    fn app(db_path: PathBuf) -> App {
        let settings = Settings {
            llm: LlmSettings {
                base_url: "http://localhost".into(),
                api_key: None,
                defaults: ModelParams {
                    provider: "local".into(),
                    model: "test".into(),
                    max_tokens: 100,
                    context_window: 1000,
                    compact_at: 0.85,
                    reasoning_effort: None,
                },
                listed: Vec::new(),
            },
            db_path,
        };
        let notifier = Arc::new(Notifier::default());
        App {
            turns: Turns::new(settings.clone(), notifier.clone()),
            settings,
            port: 0,
            notifier,
            skill_folds: Mutex::new(HashMap::new()),
            idea_reports: Mutex::new(HashMap::new()),
            idea_queues: Mutex::default(),
            worktree_scan: Mutex::default(),
            server_runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[test]
    fn list_projects_includes_session_mode() {
        let db_path = std::env::temp_dir().join(format!("fennec-routes-test-{}.db", new_id()));
        let app = app(db_path.clone());
        let store = app.store().unwrap();
        let project = store.project_for("/tmp/analyze-mode").unwrap();
        let session = store.create_session(&project, "analyze").unwrap();
        store
            .append(
                &session,
                &Event::ModeChanged {
                    mode: "analyze".into(),
                    auto: false,
                    reason: None,
                },
            )
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let mut server = listener.accept().unwrap().0;
        {
            let mut client = client;
            list_projects(&app, &mut client).unwrap();
        }
        let mut body = String::new();
        server.read_to_string(&mut body).unwrap();
        let json = body
            .rsplit("\r\n\r\n")
            .next()
            .expect("a response body after the headers");
        let projects: Value = serde_json::from_str(json).unwrap();
        let mode = projects[0]["sessions"]
            .as_array()
            .and_then(|s| s.iter().find(|s| s["id"] == session))
            .and_then(|s| s["mode"].as_str());
        assert_eq!(mode, Some("analyze"));
        let _ = std::fs::remove_file(&db_path);
    }
}
