//! The integration routes (contract: `docs/api.md`): preview the session's
//! branch, integrate it on the human's confirmation, or discard it.
//! `routes.rs` only dispatches here.

use super::App;
use super::http::{Request, error, send_json};
use super::thread_view::CriterionView;
use crate::agent::ports::EventLog;
use crate::architecture;
use crate::domain::criteria::CriterionState;
use crate::domain::decision::{Decision, decisions_in_force};
use crate::domain::event::Event;
use crate::domain::integration::{
    Blocker, DiffChecks, Landing, blockers, branch_changed, commit_message, conflict_prompt,
    default_message, unacknowledged,
};
use crate::domain::model_choice::models_used;
use crate::domain::plan_scope;
use crate::domain::preflight;
use crate::domain::review_map::{self, FileFacts, ReviewSignal};
use crate::domain::security_scan::ScanStatus;
use crate::domain::test_files::new_test_patterns;
use crate::domain::understanding::{self, Understanding};
use crate::domain::workflow::run;
use crate::integration::{self, ApplyError, Preview};
use crate::notifier::NotifyingLog;
use crate::protected;
use crate::risk;
use crate::security_scan;
use crate::store::SqliteStore;
use crate::tools::Sandbox;
use crate::web::http::HttpError;
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
    /// Which security scanners ran on the branch, and why the others did not.
    security_scans: Vec<ScanStatus>,
    /// The comprehension dialogue's state on the branch's current head.
    understanding: Understanding,
    /// Review map, top-relative: riskiest first, with deterministic signals.
    review: Vec<(String, Vec<ReviewSignal>)>,
    /// Criteria whose check passed before any change (last preflight).
    passed_before: Vec<String>,
    /// The session's idea the human may finish here, when it is still started.
    idea: Option<(String, String)>,
}

/// The session's gate; a 404 or 409 when there is nothing to gate.
fn gate(store: &SqliteStore, app: &App, id: &str) -> Result<Gate, HttpError> {
    let Some((session, project)) = app.session_and_project(id)? else {
        return Err(HttpError::new(404, "no such session"));
    };
    let Some(wt) = app.place(store, &session, &project)?.worktree else {
        return Err(HttpError::new(409, "the session has no worktree"));
    };
    let Some((top, prefix)) = worktree::top(Path::new(&project.workdir)) else {
        return Err(HttpError::new(
            409,
            "the project is not in a git repository",
        ));
    };
    let preview = match integration::preview(&top, &wt) {
        Ok(preview) => preview,
        Err(e) => return Err(HttpError::new(409, e)),
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
    let rules = architecture::load(Path::new(&project.workdir));
    let changed = integration::changed_files(&top, &wt, &preview.files, &prefix);
    let events = store.events(&session.id)?;
    let mut checks = DiffChecks::of(&rules, &changed);
    // The scanners read model-written files: they run in the human's sandbox,
    // read-only, like any command.
    let sandbox = Sandbox::for_store(&store.preferences()?.sandbox, &app.settings.db_path);
    let scan = security_scan::scan(&top, &wt, &preview.files, &sandbox);
    checks.scanning = scan.running();
    checks.security = scan.findings;
    let head = preview.commits.last().map(|commit| commit.sha.as_str());
    let understanding = match head {
        Some(head) => understanding::understanding(&events, head, |dialogue| {
            store.events(dialogue).unwrap_or_default()
        }),
        None => Understanding::None,
    };
    checks.understood = understanding.held();
    // The plan is the contract the human approved: files it does not list are
    // a deviation the gate must name.
    let plan = run::current_run(&events)
        .as_ref()
        .and_then(run::plan_output)
        .map(plan_scope::plan_files)
        .unwrap_or_default();
    let changed_paths: Vec<String> = changed.iter().map(|(path, _)| path.clone()).collect();
    checks.outside_plan = plan_scope::outside(&plan, &changed_paths);
    let hot = risk::hot_files(&top);
    let tested = risk::tested_on_branch(&top, &wt.branch, &preview.files);
    let facts: Vec<FileFacts> = preview
        .files
        .iter()
        .filter_map(|path| {
            let project_path = path.strip_prefix(prefix.as_str())?;
            let (added, removed) = preview.counts.get(path).copied().unwrap_or_default();
            Some(FileFacts {
                path: project_path.to_string(),
                added,
                removed,
                protected: protected_touched.contains(path),
                hot: hot.contains(path),
                tested: tested.contains(path),
            })
        })
        .collect();
    let mut review: Vec<(String, Vec<ReviewSignal>)> = review_map::order(&facts, &checks)
        .into_iter()
        .map(|(path, signals)| (format!("{prefix}{path}"), signals))
        .collect();
    for path in preview
        .files
        .iter()
        .filter(|path| !path.starts_with(prefix.as_str()))
    {
        review.push((path.clone(), Vec::new()));
    }
    let decisions = decisions_in_force(&events);
    let passed_before = preflight::passed_before(&events);
    let criteria = run::current_run(&events)
        .map(|state| state.criteria)
        .unwrap_or_default();
    let blockers = blockers(
        preview.commits.len(),
        &criteria,
        &protected_touched,
        &checks,
        &Landing {
            origin_branch: preview.origin_branch.as_deref(),
            human_branch: preview.human_branch.as_deref(),
            local_changes: &preview.local_changes,
            conflicts: &preview.conflicts,
        },
    );
    let title = session.title.clone();
    let idea = finishable_idea(&project.id, &store.ideas(&project.id)?, &session.id);
    Ok(Gate {
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
        security_scans: scan.scans,
        understanding,
        review,
        passed_before,
        idea,
    })
}

/// The project's directory, where `.agents/protected` lives.
fn gate_workdir(app: &App, id: &str) -> Result<String, String> {
    app.session_and_project(id)?
        .map(|(_, project)| project.workdir)
        .ok_or_else(|| "no such session".into())
}

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
            "blockers": gate.blockers,
            "securityScans": gate.security_scans,
            "understanding": gate.understanding,
            "idea": idea_json(gate.idea.as_ref()),
        }),
    )
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
    let Some((_, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let Some((_, dialogue_project)) = app.session_and_project(dialogue_id)? else {
        return error(stream, 404, "no such dialogue");
    };
    if dialogue_project.id != project.id {
        return error(stream, 400, "the dialogue belongs to another project");
    }
    let store = app.store()?;
    let gate = gate(&store, app, id)?;
    let Some(head) = gate.preview.commits.last().map(|commit| commit.sha.clone()) else {
        return error(stream, 409, "nothing to understand");
    };
    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        &gate.session_id,
        &Event::UnderstandingStarted {
            session_id: dialogue_id.to_string(),
            head: head.clone(),
        },
    )?;
    let events = store.events(&gate.session_id)?;
    let understanding = understanding::understanding(&events, &head, |dialogue| {
        store.events(dialogue).unwrap_or_default()
    });
    send_json(stream, 200, &json!({ "understanding": understanding }))
}

/// `POST /api/sessions/{id}/integration/message`: an English commit subject the
/// model proposes for the integration (indicative; the human edits it), or null.
pub fn suggest_message(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let gate = gate(&store, app, id)?;

    let events = store.events(&gate.session_id)?;
    let request = crate::domain::commit_subject::request_since_integration(&events);
    let commits: Vec<&str> = gate
        .preview
        .commits
        .iter()
        .map(|commit| commit.subject.as_str())
        .collect();
    let files: Vec<(&str, u64, u64)> = gate
        .preview
        .files
        .iter()
        .map(|path| {
            let (added, removed) = gate.preview.counts.get(path).copied().unwrap_or_default();
            (path.as_str(), added, removed)
        })
        .collect();

    match super::commit_subject::suggest(&app.settings, &request, &gate.title, &commits, &files) {
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
    let Some((session, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
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
        let workdir = gate_workdir(app, id)?;
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
    let gate = gate(&store, app, id)?;
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
    // Abandoning the work ends a run still waiting on the human: its pending
    // question would otherwise outlive the work it decided.
    if let Some(finished) = run::finish_on_discard(&store.events(&gate.session_id)?) {
        log.append(&gate.session_id, &finished)?;
    }
    // The idea's work is gone with its worktree: left started, it would wait
    // for a review on the board forever.
    if let Some((project_id, idea_id)) = gate.idea {
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
    if super::workflow_routes::waiting_for(app, id)?.is_some() {
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

    let Some((session, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
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
        super::turns::Job::Input(crate::domain::slash::TurnInput::Prompt(prompt), Vec::new()),
    ) {
        return error(stream, 409, &message);
    }
    send_json(
        stream,
        202,
        &json!({ "conflicts": conflicts, "started": true }),
    )
}

/// The idea the integration dialog may finish: the session's idea, only while
/// it is still started.
fn finishable_idea(
    project_id: &str,
    ideas: &[backlog::Idea],
    session_id: &str,
) -> Option<(String, String)> {
    backlog::idea_for_session(ideas, session_id)
        .filter(|idea| idea.state == backlog::IdeaState::Started)
        .map(|idea| (project_id.to_string(), idea.id.clone()))
}

fn idea_json(idea: Option<&(String, String)>) -> Value {
    idea.map(|(project_id, id)| json!({ "projectId": project_id, "id": id }))
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finishable_idea_is_none_without_ideas() {
        assert_eq!(finishable_idea("project", &[], "session"), None);
    }

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
