//! The integration routes (contract: `docs/api.md`): preview the session's
//! branch, integrate it on the human's confirmation, or discard it.
//! `routes.rs` only dispatches here.

use super::App;
use super::http::{Request, error, send_json};
use super::notifier::NotifyingLog;
use super::routes::session_and_project;
use crate::agent::ports::EventLog;
use crate::domain::criteria::{CriterionState, CriterionStatus};
use crate::domain::decision::{Decision, decisions_in_force};
use crate::domain::event::Event;
use crate::domain::integration::{
    Blocker, blockers, commit_message, default_message, unacknowledged,
};
use crate::domain::test_files::new_test_patterns;
use crate::domain::workflow::run;
use crate::integration::{self, ApplyError, Preview};
use crate::protected;
use crate::store::SqliteStore;
use crate::worktree::{self, Worktree};
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

/// Everything the review gate decides on, computed the same way for the
/// preview and for the integration itself (the server never trusts the
/// client's view of it).
struct Gate {
    session_id: String,
    title: String,
    wt: Worktree,
    top: PathBuf,
    preview: Preview,
    criteria: Vec<CriterionState>,
    /// Informational: shown to the human, never a blocker.
    decisions: Vec<Decision>,
    protected_touched: Vec<String>,
    /// The test files under the project, relative to it, not yet protected.
    test_files: Vec<String>,
    blockers: Vec<Blocker>,
}

/// The session's gate; `Err((status, message))` when there is nothing to gate.
fn gate(store: &SqliteStore, app: &App, id: &str) -> Result<Result<Gate, (u16, String)>, String> {
    let Some((session, project)) = session_and_project(store, id)? else {
        return Ok(Err((404, "no such session".into())));
    };
    let Some(wt) = app.place(store, &session, &project)?.worktree else {
        return Ok(Err((409, "the session has no worktree".into())));
    };
    let Some((top, prefix)) = worktree::top(Path::new(&project.workdir)) else {
        return Ok(Err((409, "the project is not in a git repository".into())));
    };
    let preview = match integration::preview(&top, &wt) {
        Ok(preview) => preview,
        Err(e) => return Ok(Err((409, e))),
    };
    // The protection is the human's, from the project's tree, not the
    // worktree's copy the agent worked in.
    let protection = protected::load(Path::new(&project.workdir));
    let protected_touched = integration::protected_touched(&preview.files, &prefix, &protection);
    let in_project: Vec<String> = preview
        .files
        .iter()
        .filter_map(|file| file.strip_prefix(prefix.as_str()).map(String::from))
        .collect();
    let test_files = new_test_patterns(&in_project, &protection);
    let events = store.events(&session.id)?;
    let decisions = decisions_in_force(&events);
    let criteria = run::current_run(&events)
        .map(|state| state.criteria)
        .unwrap_or_default();
    let blockers = blockers(
        preview.commits.len(),
        &criteria,
        &protected_touched,
        preview.origin_branch.as_deref(),
        preview.human_branch.as_deref(),
    );
    let title = session.title.clone();
    Ok(Ok(Gate {
        session_id: session.id,
        title,
        wt,
        top,
        preview,
        criteria,
        decisions,
        protected_touched,
        test_files,
        blockers,
    }))
}

/// The project's directory, where `.agents/protected` lives.
fn gate_workdir(store: &SqliteStore, id: &str) -> Result<String, String> {
    session_and_project(store, id)?
        .map(|(_, project)| project.workdir)
        .ok_or_else(|| "no such session".into())
}

/// `GET /api/sessions/{id}/integration`: what the integration would do and
/// what blocks it.
pub fn get_integration(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let store = app.store()?;
    let gate = match gate(&store, app, id)? {
        Ok(gate) => gate,
        Err((status, message)) => return error(stream, status, &message),
    };
    let files: Vec<Value> = gate
        .preview
        .files
        .iter()
        .map(|path| json!({ "path": path, "protected": gate.protected_touched.contains(path) }))
        .collect();
    let criteria: Vec<Value> = gate.criteria.iter().map(criterion_json).collect();
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
            "testFiles": gate.test_files,
            "criteria": criteria,
            "decisions": gate.decisions,
            "blockers": gate.blockers,
        }),
    )
}

/// `POST /api/sessions/{id}/integrate`: the human's confirmation. The
/// client's `acknowledged` keys only say which blockers the human accepted.
pub fn integrate(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
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
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let store = app.store()?;
    let gate = match gate(&store, app, id)? {
        Ok(gate) => gate,
        Err((status, message)) => return error(stream, status, &message),
    };
    let message = match commit_message(
        body.get("message").and_then(Value::as_str).unwrap_or(""),
        &gate.session_id,
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
        let workdir = gate_workdir(&store, id)?;
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
    send_json(
        stream,
        200,
        &json!({
            "head": head,
            "commits": commits,
            "protectedTests": protected_tests,
            "protectError": protect_error,
        }),
    )
}

/// `POST /api/sessions/{id}/discard`: the human's decision to drop the
/// session's worktree, branch and base ref.
pub fn discard(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let _ = req.json()?;
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let store = app.store()?;
    let gate = match gate(&store, app, id)? {
        Ok(gate) => gate,
        Err((status, message)) => return error(stream, status, &message),
    };
    if let Err(message) = worktree::remove(&gate.top, &gate.wt) {
        return error(stream, 409, &message);
    }
    let commits: Vec<String> = gate.preview.commits.iter().map(|c| c.sha.clone()).collect();
    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        &gate.session_id,
        &Event::WorktreeDiscarded {
            branch: gate.wt.branch.clone(),
            commits: commits.clone(),
        },
    )?;
    send_json(stream, 200, &json!({ "commits": commits }))
}

fn criterion_json(state: &CriterionState) -> Value {
    let provenance = match &state.status {
        CriterionStatus::Pending => None,
        CriterionStatus::Passed { provenance } | CriterionStatus::Failed { provenance } => {
            Some(provenance)
        }
    };
    json!({
        "id": state.criterion.id,
        "description": state.criterion.description,
        "check": state.criterion.check,
        "status": state.status,
        "provenance": provenance,
    })
}
