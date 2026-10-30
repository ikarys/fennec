//! The project page's ideas backlog (contract: `docs/api.md`): every write is
//! the human's click. `routes.rs` only dispatches here.

use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use super::App;
use super::http::{Request, error, send_json};
use super::idea_queue;
use crate::agent::ports::EventLog;
use crate::domain::criteria::CriterionStatus;
use crate::domain::decision::decisions_in_force;
use crate::domain::event::Event;
use crate::domain::id::new_id;
use crate::domain::project::{Project, SessionState};
use crate::domain::workflow::run;
use crate::store::SqliteStore;
use backlog::{
    Backlog, BacklogEvent, Idea, IdeaState, Progress, STAGES, Stage, Status, queue_order, status,
    summarize,
};
use token_usage::{Filter, Ledger};

/// An idea is a note, not a document: the plan holds the detail.
const MAX_IDEA_CHARS: usize = 4_000;

/// `GET /api/projects/{id}/ideas`.
pub fn list_ideas(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    send_board(app, stream, project_id)
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
        &BacklogEvent::Added {
            idea_id: idea_id.clone(),
            text,
        },
    )?;
    if !session_id.is_empty() {
        store.append_idea_event(
            project_id,
            &BacklogEvent::Framing {
                idea_id: idea_id.clone(),
                session_id,
            },
        )?;
        store.append_idea_event(project_id, &BacklogEvent::Ready { idea_id, plan })?;
    }
    send_board(app, stream, project_id)
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
        .backlog(project_id)?
        .ideas
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
            BacklogEvent::Framing {
                idea_id,
                session_id,
            }
        }
        "ready" => {
            let plan = field("plan");
            if plan.is_empty() {
                return error(stream, 400, "empty plan");
            }
            BacklogEvent::Ready { idea_id, plan }
        }
        "drop" => BacklogEvent::Dropped { idea_id },
        _ => return error(stream, 404, "not found"),
    };
    store.append_idea_event(project_id, &event)?;
    send_board(app, stream, project_id)
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
    let backlog = store.backlog(project_id)?;
    let Some(idea) = backlog.ideas.iter().find(|idea| idea.id == idea_id) else {
        return error(stream, 404, "no such idea");
    };
    if let Err(e) = idea_queue::start_idea(app, &store, &project, idea, false) {
        return error(stream, 409, &e);
    }
    send_board(app, stream, project_id)
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
    idea_queue::start_queue(app, project_id, autonomous);
    send_board(app.as_ref(), stream, project_id)
}

/// `POST /api/projects/{id}/ideas/queue/stop`.
///
/// Stops the queue from starting the next idea; a running turn finishes.
pub fn stop_queue(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    idea_queue::stop_queue(app, project_id);
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/reorder` `{ids: [string]}`.
pub fn reorder(
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
    let Some(ids_value) = body["ids"].as_array() else {
        return error(stream, 400, "ids must be a list of idea ids");
    };
    let mut ids = Vec::with_capacity(ids_value.len());
    for value in ids_value {
        let Some(id) = value.as_str() else {
            return error(stream, 400, "ids must be a list of idea ids");
        };
        ids.push(id.to_string());
    }

    let backlog = store.backlog(project_id)?;
    let queued: std::collections::HashSet<&str> = queue_order(&backlog)
        .iter()
        .map(|idea| idea.id.as_str())
        .collect();
    if ids.iter().any(|id| !queued.contains(id.as_str())) {
        return error(stream, 400, "unknown idea in ids");
    }

    store.append_idea_event(project_id, &BacklogEvent::Reordered { ids })?;
    send_board(app, stream, project_id)
}

/// `POST /api/projects/{id}/ideas/report/read`.
pub fn report_read(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    let store = app.store()?;
    if store.project(project_id)?.is_none() {
        return error(stream, 404, "no such project");
    }
    store.append_idea_event(project_id, &BacklogEvent::ReportRead {})?;
    send_board(app, stream, project_id)
}

/// What every idea view of one board reads: fetched once per board.
struct Board<'a> {
    app: &'a App,
    store: &'a SqliteStore,
    backlog: &'a Backlog,
    project: &'a Project,
    queue: Vec<&'a Idea>,
    ledger: Option<Ledger>,
}

fn send_board(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    let store = app.store()?;
    let Some(project) = store.project(project_id)? else {
        return error(stream, 404, "no such project");
    };
    let backlog = store.backlog(project_id)?;
    let board = Board {
        app,
        store: &store,
        backlog: &backlog,
        project: &project,
        queue: queue_order(&backlog),
        ledger: Ledger::open(&app.settings.usage_path()).ok(),
    };
    let queue = queue_json(&board)?;
    let groups = groups_json(&board)?;
    let night_report = night_report_json(&board)?;

    send_json(
        stream,
        200,
        &json!({
            "queue": queue,
            "groups": groups,
            "nightReport": night_report,
        }),
    )
}

fn queue_json(board: &Board) -> Result<Value, String> {
    let running = idea_queue::queue_running(board.app, &board.project.id);
    let autonomous = running
        && board
            .backlog
            .last_run
            .as_ref()
            .is_some_and(|run| run.autonomous);
    let ready = board.queue.len();
    let next =
        backlog::next_ready(board.backlog).map(|idea| json!({ "id": idea.id, "text": idea.text }));
    let autonomy = board.store.preferences()?.autonomy;

    Ok(json!({
        "running": running,
        "autonomous": autonomous,
        "ready": ready,
        "next": next,
        "caps": {
            "maxMinutes": autonomy.max_minutes_per_idea,
            "maxTokens": autonomy.max_tokens_per_idea,
        },
    }))
}

fn groups_json(board: &Board) -> Result<Value, String> {
    let mut views: Vec<(Stage, &Idea, Value)> = Vec::new();
    for idea in &board.backlog.ideas {
        let (status, view) = idea_view(board, idea)?;
        views.push((status.stage(), idea, view));
    }
    let queue_place = |idea: &Idea| {
        board
            .queue
            .iter()
            .position(|queued| queued.id == idea.id)
            .unwrap_or(usize::MAX)
    };

    let mut groups = Vec::new();
    for stage in STAGES {
        let mut entries: Vec<(&Idea, Value)> = views
            .iter()
            .filter(|(s, _, _)| *s == stage)
            .map(|(_, idea, view)| (*idea, view.clone()))
            .collect();
        // The drafts group stays even empty: the UI puts the add input in it.
        if entries.is_empty() && stage != Stage::Drafts {
            continue;
        }
        if stage == Stage::Queue {
            entries.sort_by_key(|(idea, _)| queue_place(idea));
        } else {
            entries.sort_by_key(|(idea, _)| std::cmp::Reverse(idea.updated_at));
        }
        let ideas = entries
            .into_iter()
            .map(|(_, view)| view)
            .collect::<Vec<Value>>();
        groups.push(json!({ "stage": stage, "ideas": ideas }));
    }

    Ok(json!(groups))
}

fn night_report_json(board: &Board) -> Result<Value, String> {
    let Some(run) = &board.backlog.last_run else {
        return Ok(Value::Null);
    };
    if run.is_open() || run.read || run.idea_ids.is_empty() {
        return Ok(Value::Null);
    }

    let mut ideas = Vec::new();
    let mut statuses = Vec::new();

    for idea_id in &run.idea_ids {
        let Some(idea) = board.backlog.ideas.iter().find(|idea| &idea.id == idea_id) else {
            continue;
        };
        let (status, view) = idea_view(board, idea)?;
        statuses.push(status);
        ideas.push(view);
    }

    if ideas.is_empty() {
        return Ok(Value::Null);
    }

    Ok(json!({
        "startedAt": run.started_at,
        "finishedAt": run.finished_at,
        "reason": run.reason,
        "autonomous": run.autonomous,
        "summary": summarize(&statuses),
        "ideas": ideas,
    }))
}

fn idea_view(board: &Board, idea: &Idea) -> Result<(Status, Value), String> {
    let (app, store, project_id) = (board.app, board.store, board.project.id.as_str());
    let mut session = None;
    let mut events = None;
    let mut running = false;
    let mut session_state = None;

    if idea.state == IdeaState::Started
        && let Some(session_id) = idea.session_id.as_deref()
    {
        running = app.turns.is_running(session_id);
        session = store.session(session_id)?;
        session_state = session.as_ref().map(|s| s.state(running));
        // ponytail: reads each started idea's whole log on every board fetch
        // (the UI polls while a run goes on); a cache keyed by the log's max
        // seq, as `skill_folds` does, if it shows up.
        events = Some(store.events(session_id)?);
    }

    let report = events.as_deref().map(idea_report).unwrap_or(IdeaReport {
        integrated: false,
        criteria: None,
        decisions: Vec::new(),
    });
    let progress = progress(session_state, running, report.integrated);
    let status = status(idea, progress);
    let stage = status.stage();
    let action = status.action();

    let position = if status == Status::Queued {
        board
            .queue
            .iter()
            .position(|queued| queued.id == idea.id)
            .map(|index| index + 1)
    } else {
        None
    };

    let duration_ms = if idea.state == IdeaState::Started
        && let Some(started_at) = idea.started_at
        && let Some(session) = &session
    {
        let end = if running {
            crate::domain::clock::now_ms()
        } else {
            session.updated_at
        };
        Some(end.saturating_sub(started_at))
    } else {
        None
    };

    let tokens = if idea.state == IdeaState::Started
        && let Some(started_at) = idea.started_at
        && let Some(session) = &session
        && let Some(ledger) = &board.ledger
    {
        // ponytail: counts the project's calls in that window, exact while the
        // queue runs one idea at a time.
        let until_ms = if running {
            None
        } else {
            Some(session.updated_at as i64)
        };
        ledger
            .summary(&Filter {
                group_id: Some(project_id.to_string()),
                since_ms: Some(started_at as i64),
                until_ms,
                ..Default::default()
            })
            .ok()
            .map(|summary| {
                summary.prompt_tokens.saturating_sub(summary.cached_tokens)
                    + summary.completion_tokens
            })
    } else {
        None
    };

    let criteria = if idea.state == IdeaState::Started {
        Some(report.criteria.unwrap_or_default())
    } else {
        report.criteria
    };
    let decisions = report.decisions;

    let (commits, files) = if matches!(status, Status::Review | Status::Halted | Status::Failed)
        && let Some(session_id) = idea.session_id.as_deref()
    {
        crate::git::branch_stats(Path::new(&board.project.workdir), session_id)
            .map(|stats| (Some(stats.commits), Some(stats.files)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    let value = json!({
        "id": idea.id,
        "text": idea.text,
        "status": status,
        "stage": stage,
        "action": action,
        "menu": status.menu(),
        "sessionId": idea.session_id,
        "plan": idea.plan,
        "halted": idea.halted,
        "autonomous": idea.autonomous,
        "position": position,
        "createdAt": idea.created_at,
        "startedAt": idea.started_at,
        "durationMs": duration_ms,
        "tokens": tokens,
        "criteria": criteria,
        "decisions": decisions,
        "commits": commits,
        "files": files,
    });

    Ok((status, value))
}

fn progress(state: Option<SessionState>, running: bool, integrated: bool) -> Option<Progress> {
    let state = state?;
    if integrated {
        return Some(Progress::Integrated);
    }
    if running {
        return Some(Progress::Running);
    }
    match state {
        SessionState::WaitingForUser => Some(Progress::WaitingForUser),
        SessionState::Complete | SessionState::Idle => Some(Progress::Finished),
        SessionState::Stopped | SessionState::Error | SessionState::Truncated => {
            Some(Progress::Failed)
        }
        SessionState::Running => Some(Progress::Running),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CriteriaCounts {
    passed: usize,
    failed: usize,
    pending: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionView {
    title: String,
    choice: String,
}

struct IdeaReport {
    integrated: bool,
    criteria: Option<CriteriaCounts>,
    decisions: Vec<DecisionView>,
}

fn idea_report(events: &[Event]) -> IdeaReport {
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

    let decisions = decisions
        .into_iter()
        .map(|decision| DecisionView {
            title: decision.title,
            choice: decision.choice,
        })
        .collect();

    // The human's click is the end of an idea's road: the morning list shows it done.
    let integrated = events
        .iter()
        .any(|event| matches!(event, Event::IntegrationApplied { .. }));

    IdeaReport {
        integrated,
        criteria: if criteria.is_empty() {
            None
        } else {
            Some(CriteriaCounts {
                passed,
                failed,
                pending,
            })
        },
        decisions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_maps_each_session_state() {
        assert!(progress(None, false, false).is_none());
        assert!(matches!(
            progress(Some(SessionState::Idle), false, true),
            Some(Progress::Integrated)
        ));
        assert!(matches!(
            progress(Some(SessionState::Idle), true, false),
            Some(Progress::Running)
        ));
        assert!(matches!(
            progress(Some(SessionState::WaitingForUser), false, false),
            Some(Progress::WaitingForUser)
        ));
        assert!(matches!(
            progress(Some(SessionState::Complete), false, false),
            Some(Progress::Finished)
        ));
        assert!(matches!(
            progress(Some(SessionState::Idle), false, false),
            Some(Progress::Finished)
        ));
        assert!(matches!(
            progress(Some(SessionState::Stopped), false, false),
            Some(Progress::Failed)
        ));
        assert!(matches!(
            progress(Some(SessionState::Error), false, false),
            Some(Progress::Failed)
        ));
        assert!(matches!(
            progress(Some(SessionState::Truncated), false, false),
            Some(Progress::Failed)
        ));
    }

    #[test]
    fn idea_report_empty_log_has_no_criteria_and_no_decisions() {
        let report = idea_report(&[]);

        assert!(report.criteria.is_none());
        assert!(report.decisions.is_empty());
        assert!(!report.integrated);
    }
}
