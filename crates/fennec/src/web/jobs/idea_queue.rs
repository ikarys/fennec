//! The ideas queue: one background thread per project that starts ready ideas
//! in turn. The HTTP layer only asks it to start/stop and asks whether it runs.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::agent::ports::EventLog;
use crate::agent::workflow::WorkflowJob;
use crate::domain::project::Project;
use crate::domain::workflow::run;
use crate::store::SqliteStore;
use crate::web::App;
use crate::web::turns::Job;
use crate::workflows::find;
use backlog::{Backlog, BacklogEvent, Caps, Idea, IdeaState, next_ready};
use token_usage::{Filter, Ledger};

/// The workflow an idea is built with: the plan card's "Construire" default.
const DEFAULT_WORKFLOW_ID: &str = "plan-build-verify";

pub(crate) fn start_queue(app: &Arc<App>, project_id: &str, autonomous: bool) {
    spawn_queue(app, project_id, autonomous, Vec::new());
}

/// `resume`: the ideas of a run a restart cut, whose interrupted workflow the
/// queue resumes before it starts the next ready idea.
fn spawn_queue(app: &Arc<App>, project_id: &str, autonomous: bool, resume: Vec<String>) {
    let stop = {
        let mut queues = crate::locks::lock(&app.idea_queues);
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
            let result = run_queue(app.as_ref(), &project_id, &stop, autonomous, &resume);
            // Remove the flag even on error so a later start can run again.
            crate::locks::lock(&app.idea_queues).remove(&project_id);
            if let Err(e) = result {
                eprintln!("fennec: idea queue {project_id}: {e}");
            }
        });
    }
}

pub(crate) fn stop_queue(app: &App, project_id: &str) {
    if let Some(stop) = crate::locks::lock(&app.idea_queues).get(project_id) {
        stop.store(true, Ordering::SeqCst);
    }
}

pub(crate) fn queue_running(app: &App, project_id: &str) -> bool {
    crate::locks::lock(&app.idea_queues).contains_key(project_id)
}

pub(crate) fn start_idea(
    app: &App,
    store: &SqliteStore,
    project: &Project,
    idea: &Idea,
    autonomous: bool,
    restart: bool,
) -> Result<String, String> {
    if idea.state == IdeaState::Started && !restart {
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
                &BacklogEvent::Framing {
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
    let place = app.open_place(store, &session, project, true)?;
    // The idea's pinned files become what the agent consumes: images for the
    // model, text inlined, binaries written into the worktree.
    let (attachments, addendum) = super::idea_assets::prepare(&idea.attachments, &place.workdir);
    let task = if addendum.is_empty() {
        idea.text.clone()
    } else {
        format!("{}{}", idea.text, addendum)
    };
    let job = if let Some(plan) = &idea.plan
        && (idea.state == IdeaState::Ready || (restart && idea.state == IdeaState::Started))
    {
        // A restart reuses the approved plan; a fresh ready idea does too.
        WorkflowJob::StartFromPlan {
            workflow: found.workflow,
            task,
            plan: plan.clone(),
            attachments,
        }
    } else {
        WorkflowJob::Start {
            workflow: found.workflow,
            task,
            step_by_step: false,
            attachments,
        }
    };
    app.turns
        .start_with(&session_id, place, Job::Workflow(job), autonomous)?;
    store.append_idea_event(
        &project.id,
        &BacklogEvent::Started {
            idea_id: idea.id.clone(),
            autonomous,
        },
    )?;
    Ok(session_id)
}

fn run_queue(
    app: &App,
    project_id: &str,
    stop: &AtomicBool,
    autonomous: bool,
    resume: &[String],
) -> Result<(), String> {
    let store = app.store()?;
    store.append_idea_event(project_id, &BacklogEvent::QueueStarted { autonomous })?;

    match run_queue_loop(app, project_id, stop, autonomous, resume) {
        Ok(reason) => {
            store.append_idea_event(
                project_id,
                &BacklogEvent::QueueFinished {
                    reason: reason.into(),
                },
            )?;
            Ok(())
        }
        Err(e) => finish_with_error(&store, project_id, e),
    }
}

fn run_queue_loop(
    app: &App,
    project_id: &str,
    stop: &AtomicBool,
    autonomous: bool,
    resume: &[String],
) -> Result<&'static str, String> {
    let mut to_resume = resume.to_vec();
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok("stopped by the human");
        }

        let store = app.store()?;
        let Some(project) = store.project(project_id)? else {
            return Ok("project gone");
        };
        let backlog = store.backlog(project_id)?;

        // Each cut idea is resumed once: a resume that fails again does not loop.
        if let Some(idea_id) = to_resume.pop() {
            if let Some(session_id) = resume_idea(app, &store, &project, &backlog, &idea_id)? {
                if autonomous {
                    watch_idea(app, &store, &project.id, &idea_id, &session_id)?;
                } else {
                    wait_for_turn(app, &session_id);
                }
            }
            continue;
        }

        let Some(idea) = next_ready(&backlog) else {
            return Ok("no idea ready");
        };

        let session_id = match start_idea(app, &store, &project, idea, autonomous, false) {
            Ok(session_id) => session_id,
            // Night mode records why the idea halted and keeps going.
            Err(e) if autonomous => {
                store.append_idea_event(
                    project_id,
                    &BacklogEvent::Halted {
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

fn finish_with_error(store: &SqliteStore, project_id: &str, e: String) -> Result<(), String> {
    // The queue's own error wins over a failure to record it.
    let _ = store.append_idea_event(
        project_id,
        &BacklogEvent::QueueFinished {
            reason: format!("error: {e}"),
        },
    );
    Err(e)
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
            let mut tokens = None;
            if max_tokens > 0
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
                    tokens = Some(spent);
                }
                last_token_check = Some(Instant::now());
            }

            if let Some(hit) = backlog::cap_reached(
                started.elapsed().as_millis() as u64,
                tokens,
                Caps {
                    max_minutes,
                    max_tokens,
                },
            ) {
                halt_reason = Some(hit.to_string());
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
            &BacklogEvent::Halted {
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

/// Resumes the idea's workflow when a restart cut it mid-node: the idea is
/// started, its session idle, its run neither finished nor paused, a node open.
/// Returns the session resumed, `None` when there is nothing to resume.
fn resume_idea(
    app: &App,
    store: &SqliteStore,
    project: &Project,
    backlog: &Backlog,
    idea_id: &str,
) -> Result<Option<String>, String> {
    let Some(idea) = backlog.ideas.iter().find(|idea| idea.id == idea_id) else {
        return Ok(None);
    };
    let Some(session_id) = idea.session_id.as_deref() else {
        return Ok(None);
    };
    if idea.state != IdeaState::Started || app.turns.is_running(session_id) {
        return Ok(None);
    }
    let cut = run::current_run(&store.events(session_id)?).is_some_and(|state| {
        state.finished.is_none() && state.paused.is_none() && !state.open.is_empty()
    });
    let Some(session) = store.session(session_id)?.filter(|_| cut) else {
        return Ok(None);
    };
    let place = app.place(store, &session, project)?;
    let job = Job::Workflow(WorkflowJob::Resume);
    app.turns
        .start_with(session_id, place, job, idea.autonomous)?;
    Ok(Some(session_id.to_string()))
}

/// A queue lives in memory: a restart cuts it. The cut run is closed, so the
/// report says so, then the queue starts again in the same mode and first
/// resumes the workflows the restart interrupted.
pub(crate) fn resume_interrupted_runs(app: &Arc<App>) -> Result<usize, String> {
    let store = app.store()?;
    let open = store.open_queue_runs().map_err(|e| e.to_string())?;
    for project_id in &open {
        let Some(run) = store.backlog(project_id)?.last_run else {
            continue;
        };
        store
            .append_idea_event(
                project_id,
                &BacklogEvent::QueueFinished {
                    reason: "server restarted".into(),
                },
            )
            .map_err(|e| e.to_string())?;
        spawn_queue(app, project_id, run.autonomous, run.idea_ids);
    }
    Ok(open.len())
}
