//! The integration routes (contract: `docs/devs/api.md`): preview the session's
//! branch, integrate it on the human's confirmation, or discard it.
//! `routes.rs` only dispatches here.

use crate::agent::ports::EventLog;
use crate::domain::event::Event;
use crate::domain::integration::{
    Blocker, branch_changed, commit_message, conflict_prompt, default_message, unacknowledged,
};
use crate::domain::model_choice::models_used;
use crate::domain::review_map;
use crate::domain::understanding;
use crate::domain::workflow::run;
use crate::integration::{self, ApplyError};
use crate::notifier::NotifyingLog;
use crate::protected;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use crate::web::views::thread_view::CriterionView;
use crate::worktree;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

mod gate;
use gate::{gate, target};

/// `GET /api/sessions/{id}/integration`: what the integration would do and
/// what blocks it.
pub fn get_integration(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let gate = gate(&store, app, id)?;
    let files: Vec<Value> = gate
        .review
        .iter()
        .map(|(path, signals)| {
            let (added, removed) = gate.preview.counts.get(path).copied().unwrap_or_default();
            json!({
                "path": path,
                "protected": gate.protected_touched.contains(path),
                "carried": gate.preview.carried_changes.contains(path),
                "added": added,
                "removed": removed,
                "signals": signals,
            })
        })
        .collect();
    let criteria: Vec<CriterionView> = gate
        .criteria
        .iter()
        .map(|state| CriterionView::new(state, &gate.passed_before))
        .collect();
    send_json(
        stream,
        200,
        &json!({
            "branch": gate.wt.branch,
            "base": gate.wt.base,
            "originBranch": gate.preview.origin_branch,
            "humanBranch": gate.preview.human_branch,
            "defaultMessage": default_message(&gate.title),
            "commits": gate.preview.commits,
            "files": files,
            "commonSignals": review_map::common(&gate.review),
            "testFiles": gate.test_files,
            "criteria": criteria,
            "decisions": gate.decisions,
            "blockers": blockers_json(&gate.blockers),
            "securityScans": gate.security_scans,
            "understanding": gate.understanding,
            "idea": idea_json(gate.idea.as_ref()),
        }),
    )
}

/// The blockers as the dialog shows them: each one says whether the human may accept it.
fn blockers_json(blockers: &[Blocker]) -> Vec<Value> {
    blockers
        .iter()
        .map(|blocker| {
            let mut value = json!(blocker);
            value["acknowledgeable"] = json!(blocker.acknowledgeable());
            value
        })
        .collect()
}

/// `POST /api/sessions/{id}/understanding`: link the comprehension dialogue the
/// UI opened to the reviewed session's branch head.
pub fn start_understanding(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let body = req.json()?;
    let Some(dialogue_id) = body
        .get("dialogueId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    else {
        return error(stream, 400, "dialogueId is required");
    };
    let (_, project) = app.require_session(id)?;
    let Some((_, dialogue_project)) = app.session_and_project(dialogue_id)? else {
        return error(stream, 404, "no such dialogue");
    };
    if dialogue_project.id != project.id {
        return error(stream, 400, "the dialogue belongs to another project");
    }
    let store = app.store()?;
    let target = target(&store, app, id)?;
    let Some(head) = target
        .preview
        .commits
        .last()
        .map(|commit| commit.sha.clone())
    else {
        return error(stream, 409, "nothing to understand");
    };
    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        &target.session.id,
        &Event::UnderstandingStarted {
            session_id: dialogue_id.to_string(),
            head: head.clone(),
        },
    )?;
    let events = store.events(&target.session.id)?;
    let understanding = understanding::understanding(&events, &head, |dialogue| {
        store.events(dialogue).unwrap_or_default()
    });
    send_json(stream, 200, &json!({ "understanding": understanding }))
}

/// `POST /api/sessions/{id}/integration/message`: an English commit subject the
/// model proposes for the integration (indicative; the human edits it), or null.
pub fn suggest_message(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let target = target(&store, app, id)?;

    let events = store.events(&target.session.id)?;
    let task = crate::domain::conversation::first_request(&events).unwrap_or_default();
    let request = crate::domain::commit_subject::request_since_integration(&events);
    let commits: Vec<&str> = target
        .preview
        .commits
        .iter()
        .map(|commit| commit.subject.as_str())
        .collect();
    let files: Vec<(&str, u64, u64)> = target
        .preview
        .files
        .iter()
        .map(|path| {
            let (added, removed) = target.preview.counts.get(path).copied().unwrap_or_default();
            (path.as_str(), added, removed)
        })
        .collect();

    match crate::web::jobs::commit_subject::suggest(
        &app.settings,
        &task,
        &request,
        &target.session.title,
        &commits,
        &files,
    ) {
        Ok(message) => send_json(stream, 200, &json!({ "message": message })),
        Err(e) => {
            eprintln!("fennec: session {id}: commit subject: {e}");
            send_json(stream, 200, &json!({ "message": null }))
        }
    }
}

/// The human's repository as it is now, read for every session (with or
/// without a worktree).
pub fn get_repo(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let (session, project) = app.require_session(id)?;
    let Some((top, _)) = worktree::top(Path::new(&project.workdir)) else {
        return error(stream, 409, "the project is not in a git repository");
    };
    let branch = crate::git::current_branch(&top);
    let origin = app
        .place(&store, &session, &project)?
        .worktree
        .and_then(|wt| worktree::origin_branch(&top, &wt));
    send_json(
        stream,
        200,
        &json!({
            "dir": top.to_string_lossy(),
            "branch": branch,
            "originBranch": origin,
            "branchChanged": branch_changed(origin.as_deref(), branch.as_deref()),
            "dirty": crate::git::dirty_count(&top),
        }),
    )
}

/// `POST /api/sessions/{id}/integrate`: the human's confirmation. The
/// client's `acknowledged` keys only say which blockers the human accepted.
pub fn integrate(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let body = req.json()?;
    let acknowledged: Vec<String> = body
        .get("acknowledged")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let protect_tests = body
        .get("protectTests")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let remove_worktree = body
        .get("removeWorktree")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let store = app.store()?;
    let gate = gate(&store, app, id)?;
    // The human may keep their commits free of model trailers (Settings).
    let mut models = Vec::new();
    if store.preferences()?.integration.model_trailers {
        for session in store.session_tree(&gate.session_id)? {
            for model in models_used(&store.events(&session)?) {
                if !models.contains(&model) {
                    models.push(model);
                }
            }
        }
    }
    let message = match commit_message(
        body.get("message").and_then(Value::as_str).unwrap_or(""),
        &gate.session_id,
        &models,
    ) {
        Ok(message) => message,
        Err(e) => return error(stream, 400, &e),
    };
    let missing = unacknowledged(&gate.blockers, &acknowledged);
    if !missing.is_empty() {
        return send_json(
            stream,
            409,
            &json!({ "error": "unacknowledged blockers", "blockers": missing }),
        );
    }

    let head = match integration::apply(&gate.top, &gate.wt, &message) {
        Ok(head) => head,
        Err(e @ ApplyError::Conflict { .. }) => {
            return send_json(stream, 409, &json!({ "error": "conflict", "conflict": e }));
        }
        Err(ApplyError::Git { message }) => return error(stream, 409, &message),
    };
    let commits: Vec<String> = gate.preview.commits.iter().map(|c| c.sha.clone()).collect();
    // The integration is done: a failure here is reported, never undone.
    let (protected_tests, protect_error) = if protect_tests {
        // The project's directory, where `.agents/protected` lives.
        let (_, project) = app.require_session(id)?;
        let workdir = project.workdir;
        match protected::protect_tests(Path::new(&workdir), &gate.test_files) {
            Ok(added) => (added, None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        }
    } else {
        (Vec::new(), None)
    };
    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        &gate.session_id,
        &Event::IntegrationApplied {
            branch: gate.wt.branch.clone(),
            base: gate.wt.base.clone(),
            commits: commits.clone(),
            head: head.clone(),
            message,
            acknowledged,
            protected_tests: protected_tests.clone(),
        },
    )?;
    // Same rule as above: the integration stands, a cleanup failure is reported.
    let (worktree_removed, remove_error) = if remove_worktree {
        match worktree::remove(&gate.top, &gate.wt) {
            Ok(()) => {
                // No commits to record: the base ref moved, `integration.applied` has them.
                log.append(
                    &gate.session_id,
                    &Event::WorktreeDiscarded {
                        branch: gate.wt.branch.clone(),
                        commits: Vec::new(),
                    },
                )?;
                (true, None)
            }
            Err(e) => (false, Some(e)),
        }
    } else {
        (false, None)
    };
    send_json(
        stream,
        200,
        &json!({
            "head": head,
            "commits": commits,
            "protectedTests": protected_tests,
            "protectError": protect_error,
            "worktreeRemoved": worktree_removed,
            "removeError": remove_error,
        }),
    )
}

/// `POST /api/sessions/{id}/discard`: the human's decision to drop the
/// session's worktree, branch and base ref.
pub fn discard(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let _ = req.json()?;
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let store = app.store()?;
    let target = target(&store, app, id)?;
    let idea = gate::finishable_idea(
        &target.project.id,
        &store.ideas(&target.project.id)?,
        &target.session.id,
    );
    if let Err(message) = worktree::remove(&target.top, &target.wt) {
        return error(stream, 409, &message);
    }
    let commits: Vec<String> = target
        .preview
        .commits
        .iter()
        .map(|c| c.sha.clone())
        .collect();
    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        &target.session.id,
        &Event::WorktreeDiscarded {
            branch: target.wt.branch.clone(),
            commits: commits.clone(),
        },
    )?;
    // Abandoning the work ends a run still waiting on the human: its pending
    // question would otherwise outlive the work it decided.
    if let Some(finished) = run::finish_on_discard(&store.events(&target.session.id)?) {
        log.append(&target.session.id, &finished)?;
    }
    // The idea's work is gone with its worktree: left started, it would wait
    // for a review on the board forever.
    if let Some((project_id, idea_id)) = idea {
        store.append_idea_event(&project_id, &backlog::BacklogEvent::Dropped { idea_id })?;
    }
    send_json(stream, 200, &json!({ "commits": commits }))
}

/// `POST /api/sessions/{id}/resolve-conflict`: bring the branch up to date
/// with the human's HEAD and, when conflicts remain, start a turn that
/// resolves them.
pub fn resolve_conflict(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let _ = req.json()?;
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    if crate::web::routes::workflow_routes::waiting_for(app, id)?.is_some() {
        return error(stream, 409, "a workflow is waiting in this session");
    }
    let store = app.store()?;
    let gate = gate(&store, app, id)?;
    if let Err(message) = worktree::commit(&gate.wt, &format!("fennec: agent work in session {id}"))
    {
        return error(stream, 409, &message);
    }
    let mut conflicts = match integration::update_branch(&gate.top, &gate.wt) {
        Ok(conflicts) => conflicts,
        Err(message) => return error(stream, 409, &message),
    };
    // Markers an earlier resolution left: git no longer sees them, the gate does.
    for blocker in &gate.blockers {
        if let Blocker::Conflict { files } = blocker {
            for file in files {
                if !conflicts.contains(file) {
                    conflicts.push(file.clone());
                }
            }
        }
    }
    if conflicts.is_empty() {
        return send_json(stream, 200, &json!({ "conflicts": [], "started": false }));
    }

    let (session, project) = app.require_session(id)?;
    let agents = crate::agents::discover(Path::new(&project.workdir));
    let events = store.events(id)?;
    if let Ok(Some(Event::ModeChanged { mode, .. })) =
        crate::domain::agent::switch_to(&agents, &events, "builder")
    {
        let log = NotifyingLog {
            log: &store,
            notifier: &app.notifier,
        };
        log.append(
            id,
            &Event::ModeChanged {
                mode,
                auto: true,
                reason: Some("conflict".into()),
            },
        )?;
    }

    let human_branch = gate
        .preview
        .human_branch
        .clone()
        .unwrap_or_else(|| "HEAD".into());
    let prompt = conflict_prompt(&human_branch, &conflicts);
    let place = app.place(&store, &session, &project)?;
    if let Err(message) = app.turns.start(
        id,
        place,
        crate::web::turns::Job::Input(crate::domain::slash::TurnInput::Prompt(prompt), Vec::new()),
    ) {
        return error(stream, 409, &message);
    }
    send_json(
        stream,
        202,
        &json!({ "conflicts": conflicts, "started": true }),
    )
}

fn idea_json(idea: Option<&(String, String)>) -> Value {
    idea.map(|(project_id, id)| json!({ "projectId": project_id, "id": id }))
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idea_json_is_null_without_idea() {
        assert_eq!(idea_json(None), Value::Null);
    }

    #[test]
    fn idea_json_carries_project_and_idea_ids() {
        let idea = ("project".to_string(), "idea".to_string());
        assert_eq!(
            idea_json(Some(&idea)),
            json!({ "projectId": "project", "id": "idea" })
        );
    }
}
