//! The workflow routes (contract: `docs/api.md`): discovery, validation and
//! the run controls. `routes.rs` only dispatches here.

use super::App;
use super::http::{Request, error, send_json};
use super::routes::session_and_project;
use super::turns::{Job, Place};
use crate::agent::ports::EventLog;
use crate::agent::workflow::WorkflowJob;
use crate::agents;
use crate::domain::project::Project;
use crate::domain::workflow::definition::Workflow;
use crate::domain::workflow::run::{self as run, Waiting};
use crate::domain::workflow::validate;
use crate::workflows::{self, Origin};
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

fn project(app: &App, id: &str) -> Result<Option<Project>, String> {
    Ok(app.store()?.project(id)?)
}

pub fn list_workflows(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some(project) = project(app, id)? else {
        return error(stream, 404, "no such project");
    };
    let found = workflows::discover(Path::new(&project.workdir));
    let out: Vec<Value> = found
        .iter()
        .map(|f| {
            json!({
                "id": f.workflow.id,
                "name": f.workflow.name,
                "description": f.workflow.description,
                "origin": f.origin,
            })
        })
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

pub fn get_workflow(app: &App, stream: &mut TcpStream, id: &str, wid: &str) -> Result<(), String> {
    let Some(project) = project(app, id)? else {
        return error(stream, 404, "no such project");
    };
    let Some(found) = workflows::find(Path::new(&project.workdir), wid) else {
        return error(stream, 404, "no such workflow");
    };
    send_json(
        stream,
        200,
        &json!({ "origin": found.origin, "workflow": found.raw }),
    )
}

pub fn save_workflow(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
    wid: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let Some(project) = project(app, id)? else {
        return error(stream, 404, "no such project");
    };
    let scope = body["scope"].as_str().unwrap_or("");
    let origin = match scope {
        "project" => Origin::Project,
        "global" => Origin::Global,
        _ => return error(stream, 400, "scope must be \"project\" or \"global\""),
    };
    let raw = &body["workflow"];
    if raw.is_null() {
        return error(stream, 400, "workflow is required");
    }
    let issues = validate_workflow(Path::new(&project.workdir), wid, raw);
    if !issues.is_empty() {
        return send_json(
            stream,
            400,
            &json!({ "error": "invalid workflow", "errors": issues }),
        );
    }
    let path = workflows::save(Path::new(&project.workdir), origin, raw)?;
    let found = workflows::find(Path::new(&project.workdir), wid)
        .ok_or_else(|| format!("saved workflow not found at {}", path.display()))?;
    send_json(
        stream,
        200,
        &json!({ "origin": found.origin, "workflow": found.raw }),
    )
}

pub fn validate_workflow_route(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
) -> Result<(), String> {
    let body = req.json()?;
    let Some(project) = project(app, body["projectId"].as_str().unwrap_or(""))? else {
        return error(stream, 404, "no such project");
    };
    let raw = &body["workflow"];
    if raw.is_null() {
        return error(stream, 400, "workflow is required");
    }
    let id = raw.get("id").and_then(Value::as_str).unwrap_or("");
    let issues = validate_workflow(Path::new(&project.workdir), id, raw);
    send_json(stream, 200, &json!({ "errors": issues }))
}

/// The issues of `raw` as a workflow named `id`: a serde error is one issue
/// without node, then the id checks, then the graph validation.
fn validate_workflow(workdir: &Path, id: &str, raw: &Value) -> Vec<Value> {
    let workflow: Workflow = match serde_json::from_value(raw.clone()) {
        Ok(workflow) => workflow,
        Err(e) => {
            return vec![json!({ "message": format!("invalid workflow: {e}") })];
        }
    };
    let mut issues: Vec<Value> = Vec::new();
    if workflow.id != id {
        issues.push(json!({ "message": format!("workflow id is {}, expected {id}", workflow.id) }));
    }
    if !workflows::valid_id(&workflow.id) {
        issues.push(json!({
            "message": "workflow id must match ^[a-z0-9][a-z0-9-]*$".to_string()
        }));
    }
    let agents = agents::discover(workdir);
    let graph = validate::validate(&workflow, &agents);
    issues.extend(
        graph
            .iter()
            .filter_map(|issue| serde_json::to_value(issue).ok()),
    );
    issues
}

pub fn start_workflow(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let store = app.store()?;
    let Some((session, project)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    let workflow_id = body["workflowId"].as_str().unwrap_or("");
    let Some(found) = workflows::find(Path::new(&project.workdir), workflow_id) else {
        return error(stream, 404, "no such workflow");
    };
    let attachments = match super::routes::parse_attachments(&body) {
        Ok(attachments) => attachments,
        Err(e) => return error(stream, 400, &e),
    };
    let task = body["task"].as_str().unwrap_or("").trim().to_string();
    if task.is_empty() && attachments.is_empty() {
        return error(stream, 400, "empty task");
    }
    let worktree = body["worktree"].as_bool().unwrap_or(true);
    let plan = body["plan"].as_str().unwrap_or("").trim().to_string();
    let job = if plan.is_empty() {
        WorkflowJob::Start {
            workflow: found.workflow,
            task,
            step_by_step: body["stepByStep"].as_bool().unwrap_or(false),
            attachments,
        }
    } else {
        WorkflowJob::StartFromPlan {
            workflow: found.workflow,
            task,
            plan,
        }
    };
    // The workflow definition is the human's: found in the project, not the worktree.
    let place = app.open_place(&store, &session, &project, worktree)?;
    start_job(app, stream, id, place, job)
}

/// The run controls `/workflow/{answer,continue,resume,stop}`.
pub fn control_workflow(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
    control: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let text = |field: &str| body[field].as_str().unwrap_or("").to_string();
    let job = match control {
        "answer" => WorkflowJob::Answer {
            choice: text("choice"),
            comment: text("comment"),
        },
        "continue" => WorkflowJob::Continue {
            comment: text("comment"),
        },
        "resume" => WorkflowJob::Resume,
        "stop" => WorkflowJob::Stop,
        _ => return error(stream, 404, "not found"),
    };
    let store = app.store()?;
    let Some((session, project)) = session_and_project(&store, id)? else {
        return error(stream, 404, "no such session");
    };
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let current = run::current_run(&store.events(id)?);
    if current.is_none_or(|state| state.run_id != text("runId")) {
        return error(stream, 409, "not the session's current run");
    }
    let place = app.place(&store, &session, &project)?;
    start_job(app, stream, id, place, job)
}

fn start_job(
    app: &App,
    stream: &mut TcpStream,
    id: &str,
    place: Place,
    job: WorkflowJob,
) -> Result<(), String> {
    match app.turns.start(id, place, Job::Workflow(job)) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
}

/// Where an active run waits, to decide what a typed message means.
pub fn waiting_for(app: &App, id: &str) -> Result<Option<Waiting>, String> {
    let store = app.store()?;
    let events = store.events(id)?;
    let Some(state) = run::current_run(&events) else {
        return Ok(None);
    };
    Ok(run::waiting(&state, &events))
}
