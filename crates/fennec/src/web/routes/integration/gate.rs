//! The review gate: everything the integration decides on, computed from the
//! session's worktree, the project's rules and the log.

use crate::agent::ports::EventLog;
use crate::architecture;
use crate::domain::criteria::CriterionState;
use crate::domain::decision::{Decision, decisions_in_force};
use crate::domain::integration::{Blocker, DiffChecks, Landing, blockers};
use crate::domain::plan_scope;
use crate::domain::preflight;
use crate::domain::project::{Project, SessionSummary};
use crate::domain::review_map::{self, FileFacts, ReviewSignal};
use crate::domain::security_scan::ScanStatus;
use crate::domain::test_files::new_test_patterns;
use crate::domain::understanding::{self, Understanding};
use crate::domain::workflow::run;
use crate::integration::{self, Preview};
use crate::protected;
use crate::risk;
use crate::sandbox::Sandbox;
use crate::security_scan;
use crate::store::SqliteStore;
use crate::web::App;
use crate::web::http::HttpError;
use crate::worktree::{self, Worktree};
use std::path::{Path, PathBuf};

/// What the integration acts on: the session, its worktree and the branch's preview,
/// without the gate's checks (cheap: no scanner, no blame).
pub(super) struct Target {
    pub(super) session: SessionSummary,
    pub(super) project: Project,
    pub(super) wt: Worktree,
    pub(super) top: PathBuf,
    /// The project's path inside the repository ("" at its root, else ending in '/').
    pub(super) prefix: String,
    pub(super) preview: Preview,
}

/// The session's target; a 404 or 409 when there is nothing to integrate.
pub(super) fn target(store: &SqliteStore, app: &App, id: &str) -> Result<Target, HttpError> {
    let (session, project) = app.require_session(id)?;
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
    Ok(Target {
        session,
        project,
        wt,
        top,
        prefix,
        preview,
    })
}

/// Everything the review gate decides on, computed the same way for the
/// preview and for the integration itself (the server never trusts the
/// client's view of it).
pub(super) struct Gate {
    pub(super) session_id: String,
    pub(super) title: String,
    pub(super) wt: Worktree,
    pub(super) top: PathBuf,
    pub(super) preview: Preview,
    pub(super) criteria: Vec<CriterionState>,
    /// Informational: shown to the human, never a blocker.
    pub(super) decisions: Vec<Decision>,
    pub(super) protected_touched: Vec<String>,
    /// The test files under the project, relative to it, not yet protected.
    pub(super) test_files: Vec<String>,
    pub(super) blockers: Vec<Blocker>,
    /// Which security scanners ran on the branch, and why the others did not.
    pub(super) security_scans: Vec<ScanStatus>,
    /// The comprehension dialogue's state on the branch's current head.
    pub(super) understanding: Understanding,
    /// Review map, top-relative: riskiest first, with deterministic signals.
    pub(super) review: Vec<(String, Vec<ReviewSignal>)>,
    /// Criteria whose check passed before any change (last preflight).
    pub(super) passed_before: Vec<String>,
    /// The session's idea the human may finish here, when it is still started.
    pub(super) idea: Option<(String, String)>,
}

/// The session's gate; a 404 or 409 when there is nothing to gate.
pub(super) fn gate(store: &SqliteStore, app: &App, id: &str) -> Result<Gate, HttpError> {
    let Target {
        session,
        project,
        wt,
        top,
        prefix,
        preview,
    } = target(store, app, id)?;
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
    let changed = integration::changed_files(&top, &wt, &preview.files, &prefix)?;
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
    // a deviation the gate must name. The tests the human validated are part
    // of that contract.
    let plan = run::current_run(&events)
        .as_ref()
        .and_then(run::plan_output)
        .map(plan_scope::plan_files)
        .unwrap_or_default();
    let validated: Vec<String> = crate::domain::test_files::approved(&events)
        .iter()
        .map(|path| path.trim_start_matches("./").to_string())
        .collect();
    let contract = plan_scope::contract(plan, &validated);
    let changed_paths: Vec<String> = changed.iter().map(|(path, _)| path.clone()).collect();
    checks.outside_plan = plan_scope::outside(&contract, &changed_paths);
    checks.validated_tests = validated;
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

/// The idea the integration dialog may finish: the session's idea, only while
/// it is still started.
pub(super) fn finishable_idea(
    project_id: &str,
    ideas: &[backlog::Idea],
    session_id: &str,
) -> Option<(String, String)> {
    backlog::idea_for_session(ideas, session_id)
        .filter(|idea| idea.state == backlog::IdeaState::Started)
        .map(|idea| (project_id.to_string(), idea.id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finishable_idea_is_none_without_ideas() {
        assert_eq!(finishable_idea("project", &[], "session"), None);
    }
}
