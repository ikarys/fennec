//! The project page's ideas backlog (contract: `docs/api.md`): every write is
//! the human's click. `routes.rs` only dispatches here.

use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::App;
use super::http::{Request, error, send_json};
use super::turns::Job;
use crate::agent::ports::EventLog;
use crate::agent::workflow::WorkflowJob;
use crate::domain::criteria::CriterionStatus;
use crate::domain::decision::decisions_in_force;
use crate::domain::event::Event;
use crate::domain::id::new_id;
use crate::domain::idea::{Idea, IdeaEvent, IdeaState, next_ready};
use crate::domain::project::Project;
use crate::domain::workflow::run;
use crate::store::SqliteStore;
use crate::workflows::find;
use token_usage::{Filter, Ledger};

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

/// `POST /api/projects/{id}/ideas` `{text, sessionId?, plan?}`: with a
/// session and its plan, a plan written outside the backlog joins it, ready.
pub fn add_idea(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), String> {
    let store = app.store()?;
    if store.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    let body = req.json()?;
    let field = |name: &str| body[name].as_str().unwrap_or("").trim().to_string();
    let text = field("text");
    if text.is_empty() {
        return error(stream, 400, "empty idea");
    }
    if text.chars().count() > MAX_IDEA_CHARS {
        return error(stream, 400, "idea too long");
    }
    let (session_id, plan) = (field("sessionId"), field("plan"));
    if !session_id.is_empty() {
        match store.session(&session_id)? {
            Some(session) if session.project_id == project_id => {}
            _ => return error(stream, 404, "no such session"),
        }
        if plan.is_empty() {
            return error(stream, 400, "empty plan");
        }
    }
    let idea_id = new_id();
    store.append_idea_event(
        project_id,
        &IdeaEvent::Added {
            idea_id: idea_id.clone(),
            text,
        },
    )?;
    if !session_id.is_empty() {
        store.append_idea_event(
            project_id,
            &IdeaEvent::Framing {
                idea_id: idea_id.clone(),
                session_id,
            },
        )?;
        store.append_idea_event(project_id, &IdeaEvent::Ready { idea_id, plan })?;
    }
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
    if let Err(e) = start_idea(app, &store, &project, idea, false) {
        return error(stream, 409, &e);
    }
    send_ideas(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/queue/start`.
///
/// Starts the project's idea queue if it is not already running.
pub fn start_queue(
    app: &Arc<App>,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    let autonomous = req.json()?["autonomous"].as_bool().unwrap_or(false);

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
            let result = run_queue(app.as_ref(), &project_id, &stop, autonomous);
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
        let mut session_state = None;
        let mut report = None;
        if idea.state == IdeaState::Started
            && let Some(session_id) = idea.session_id.as_deref()
        {
            session_state = store
                .session(session_id)?
                .map(|session| session.state(app.turns.is_running(session_id)));
            // ponytail: reads each started idea's whole log on every board
            // fetch (the UI polls while a run goes on); a cache keyed by the
            // log's max seq, as `skill_folds` does, if it shows up.
            if session_state.is_some() {
                let events = store.events(session_id)?;
                report = Some(idea_report(&events));
            }
        }
        value["sessionState"] = json!(session_state);
        value["report"] = json!(report);
        ideas_json.push(value);
    }

    send_json(
        stream,
        200,
        &json!({ "ideas": ideas_json, "queueRunning": queue_running }),
    )
}

fn run_queue(
    app: &App,
    project_id: &str,
    stop: &AtomicBool,
    autonomous: bool,
) -> Result<(), String> {
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

        let session_id = match start_idea(app, &store, &project, idea, autonomous) {
            Ok(session_id) => session_id,
            // Night mode records why the idea halted and keeps going.
            Err(e) if autonomous => {
                store.append_idea_event(
                    project_id,
                    &IdeaEvent::Halted {
                        idea_id: idea.id.clone(),
                        reason: e,
                    },
                )?;
                continue;
            }
            Err(e) => return Err(e),
        };

        if autonomous {
            watch_idea(app, &store, &project.id, &idea.id, &session_id)?;
        } else {
            wait_for_turn(app, &session_id);
        }
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
    autonomous: bool,
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
    app.turns
        .start_with(&session_id, place, Job::Workflow(job), autonomous)?;
    store.append_idea_event(
        &project.id,
        &IdeaEvent::Started {
            idea_id: idea.id.clone(),
        },
    )?;
    Ok(session_id)
}

fn watch_idea(
    app: &App,
    store: &SqliteStore,
    project_id: &str,
    idea_id: &str,
    session_id: &str,
) -> Result<(), String> {
    let autonomy = store.preferences()?.autonomy;
    let max_minutes = autonomy.max_minutes_per_idea;
    let max_tokens = autonomy.max_tokens_per_idea;

    let started = Instant::now();
    let since_ms = crate::domain::clock::now_ms() as i64;
    let ledger = if max_tokens > 0 {
        Some(Ledger::open(&app.settings.usage_path()).map_err(|e| e.to_string())?)
    } else {
        None
    };

    let mut halt_reason: Option<String> = None;
    let mut stopped = false;
    let mut last_token_check: Option<Instant> = None;

    loop {
        let seen = app.notifier.generation();
        if !app.turns.is_running(session_id) {
            break;
        }

        if halt_reason.is_none() {
            if max_minutes > 0 && started.elapsed() >= Duration::from_secs(max_minutes as u64 * 60)
            {
                halt_reason = Some(format!("stopped after {max_minutes} minutes (cap)"));
            } else if max_tokens > 0
                && last_token_check
                    .is_none_or(|checked| checked.elapsed() >= Duration::from_secs(30))
            {
                // ponytail: the ledger is summed per project since the idea started, so other
                // work in the same project meanwhile counts too; a per-session filter
                // (session and its child sessions) is the upgrade.
                if let Some(ledger) = &ledger {
                    let summary = ledger
                        .summary(&Filter {
                            group_id: Some(project_id.to_string()),
                            since_ms: Some(since_ms),
                            ..Default::default()
                        })
                        .map_err(|e| e.to_string())?;
                    let spent = summary.prompt_tokens.saturating_sub(summary.cached_tokens)
                        + summary.completion_tokens;
                    if spent > max_tokens {
                        halt_reason =
                            Some(format!("stopped after {spent} tokens (cap {max_tokens})"));
                    }
                }
                last_token_check = Some(Instant::now());
            }
        }

        if !stopped && halt_reason.is_some() {
            app.turns.stop(session_id);
            stopped = true;
        }

        app.notifier.wait_past(seen, Duration::from_secs(30));
    }

    if let Some(reason) = halt_reason {
        store.append_idea_event(
            project_id,
            &IdeaEvent::Halted {
                idea_id: idea_id.to_string(),
                reason,
            },
        )?;
    }
    Ok(())
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

fn idea_report(events: &[Event]) -> Value {
    let criteria = run::current_run(events)
        .map(|state| state.criteria)
        .unwrap_or_default();
    let decisions = decisions_in_force(events);

    let mut passed = 0;
    let mut failed = 0;
    let mut pending = 0;
    for criterion in &criteria {
        match &criterion.status {
            CriterionStatus::Pending => pending += 1,
            CriterionStatus::Passed { .. } => passed += 1,
            CriterionStatus::Failed { .. } => failed += 1,
        }
    }

    let decisions_json = decisions
        .iter()
        .map(|decision| json!({ "title": decision.title, "choice": decision.choice }))
        .collect::<Vec<Value>>();

    json!({
        "criteria": {
            "passed": passed,
            "failed": failed,
            "pending": pending,
        },
        "decisions": decisions_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idea_report_empty_log_has_zero_counts() {
        let report = idea_report(&[]);

        assert_eq!(report["criteria"]["passed"], 0);
        assert_eq!(report["criteria"]["failed"], 0);
        assert_eq!(report["criteria"]["pending"], 0);
        assert_eq!(report["decisions"], json!([]));
    }
}
