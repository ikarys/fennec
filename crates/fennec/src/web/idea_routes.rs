//! The project page's ideas backlog (contract: `docs/api.md`): every write is
//! the human's click. `routes.rs` only dispatches here.

use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::json;

use super::App;
use super::http::{Request, error, send_json};
use super::turns::Job;
use crate::agent::workflow::WorkflowJob;
use crate::domain::id::new_id;
use crate::domain::idea::{Idea, IdeaEvent, IdeaState, next_ready};
use crate::domain::project::Project;
use crate::store::SqliteStore;
use crate::workflows::find;

/// The workflow an idea is built with: the plan card's "Construire" default.
const DEFAULT_WORKFLOW_ID: &str = "plan-build-verify";

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

/// `POST /api/projects/{id}/ideas/{iid}/start`: builds this idea now, framed
/// or not, outside the queue (which then skips it, no longer ready).
pub fn start_one(
    app: &App,
    stream: &mut TcpStream,
    project_id: &str,
    idea_id: &str,
) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(project_id)? else {
        return error(stream, 404, "no such project");
    };
    let ideas = store.ideas(project_id)?;
    let Some(idea) = ideas.iter().find(|idea| idea.id == idea_id) else {
        return error(stream, 404, "no such idea");
    };
    if let Err(e) = start_idea(app, &store, &project, idea) {
        return error(stream, 409, &e);
    }
    send_ideas(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/queue/start`.
///
/// Starts the project's idea queue if it is not already running.
pub fn start_queue(app: &Arc<App>, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }

    let stop = {
        let mut queues = app.idea_queues.lock().expect("idea queue lock poisoned");
        if queues.contains_key(project_id) {
            None
        } else {
            let stop = Arc::new(AtomicBool::new(false));
            queues.insert(project_id.to_string(), stop.clone());
            Some(stop)
        }
    };

    if let Some(stop) = stop {
        let app = app.clone();
        let project_id = project_id.to_string();
        thread::spawn(move || {
            let result = run_queue(app.as_ref(), &project_id, &stop);
            // Remove the flag even on error so a later start can run again.
            app.idea_queues
                .lock()
                .expect("idea queue lock poisoned")
                .remove(&project_id);
            if let Err(e) = result {
                eprintln!("fennec: idea queue {project_id}: {e}");
            }
        });
    }

    send_ideas(app.as_ref(), stream, project_id)
}

/// `POST /api/projects/{id}/ideas/queue/stop`.
///
/// Stops the queue from starting the next idea; a running turn finishes.
pub fn stop_queue(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    if let Some(stop) = app
        .idea_queues
        .lock()
        .expect("idea queue lock poisoned")
        .get(project_id)
    {
        stop.store(true, Ordering::SeqCst);
    }
    send_ideas(app, stream, project_id)
}

fn send_ideas(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    let store = app.store()?;
    let ideas = store.ideas(project_id)?;
    let queue_running = app
        .idea_queues
        .lock()
        .expect("idea queue lock poisoned")
        .contains_key(project_id);

    let mut ideas_json = Vec::with_capacity(ideas.len());
    for idea in &ideas {
        let mut value = json!(idea);
        let session_state = if idea.state == IdeaState::Started {
            match idea.session_id.as_deref() {
                Some(session_id) => store
                    .session(session_id)?
                    .map(|session| session.state(app.turns.is_running(session_id))),
                None => None,
            }
        } else {
            None
        };
        value["sessionState"] = json!(session_state);
        ideas_json.push(value);
    }

    send_json(
        stream,
        200,
        &json!({ "ideas": ideas_json, "queueRunning": queue_running }),
    )
}

fn run_queue(app: &App, project_id: &str, stop: &AtomicBool) -> Result<(), String> {
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }

        let store = app.store()?;
        let Some(project) = store.project(project_id)? else {
            return Ok(());
        };
        let ideas = store.ideas(project_id)?;
        let Some(idea) = next_ready(&ideas) else {
            return Ok(());
        };
        let session_id = start_idea(app, &store, &project, idea)?;
        wait_for_turn(app, &session_id);
    }
}

/// Starts the idea's build with the default workflow, in its session's
/// worktree: from the approved plan when it is ready, from the idea itself
/// otherwise (the workflow then plans first). An idea not framed yet gets its
/// session here. Errors when the session is busy. Returns the session the
/// build runs in.
fn start_idea(
    app: &App,
    store: &SqliteStore,
    project: &Project,
    idea: &Idea,
) -> Result<String, String> {
    if idea.state == IdeaState::Started {
        return Err("idea already started".into());
    }
    let Some(found) = find(
        Path::new(&project.workdir),
        DEFAULT_WORKFLOW_ID,
        &store.preferences()?.enabled_workflows,
    ) else {
        return Err(format!("no workflow {DEFAULT_WORKFLOW_ID}"));
    };
    let session_id = match &idea.session_id {
        Some(id) => id.clone(),
        None => {
            let title: String = idea.text.chars().take(80).collect();
            let id = store.create_session(project, &title)?;
            store.append_idea_event(
                &project.id,
                &IdeaEvent::Framing {
                    idea_id: idea.id.clone(),
                    session_id: id.clone(),
                },
            )?;
            id
        }
    };
    let Some(session) = store.session(&session_id)? else {
        return Err("no such session".into());
    };
    let task = idea.text.clone();
    let job = match (idea.state, &idea.plan) {
        (IdeaState::Ready, Some(plan)) => WorkflowJob::StartFromPlan {
            workflow: found.workflow,
            task,
            plan: plan.clone(),
        },
        _ => WorkflowJob::Start {
            workflow: found.workflow,
            task,
            step_by_step: false,
            attachments: Vec::new(),
        },
    };
    let place = app.open_place(store, &session, project, true)?;
    app.turns.start(&session_id, place, Job::Workflow(job))?;
    store.append_idea_event(
        &project.id,
        &IdeaEvent::Started {
            idea_id: idea.id.clone(),
        },
    )?;
    Ok(session_id)
}

fn wait_for_turn(app: &App, session_id: &str) {
    loop {
        let seen = app.notifier.generation();
        if !app.turns.is_running(session_id) {
            return;
        }
        app.notifier.wait_past(seen, Duration::from_secs(60));
    }
}
