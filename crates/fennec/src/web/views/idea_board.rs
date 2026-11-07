//! A project's ideas board, read side (contract: `docs/api.md`): each idea's
//! status from its live session, the queue, the night report. Served by
//! `idea_routes.rs` per project and gathered by `backlog_routes.rs` across them.

use std::cell::OnceCell;
use std::collections::HashSet;
use std::path::Path;

use serde_json::{Value, json};

use crate::domain::criteria::CriterionStatus;
use crate::domain::decision::decisions_in_force;
use crate::domain::event::Event;
use crate::domain::project::{Project, SessionState};
use crate::domain::workflow::run;
use crate::store::SqliteStore;
use crate::web::App;
use crate::web::jobs::idea_queue;
use backlog::{
    Backlog, Idea, IdeaState, Progress, STAGES, Stage, Status, queue_order, status, summarize,
};
use token_usage::{Filter, Ledger};

/// What every idea view of one board reads: fetched once per board.
pub(crate) struct Board<'a> {
    app: &'a App,
    store: &'a SqliteStore,
    backlog: &'a Backlog,
    project: &'a Project,
    queue: Vec<&'a Idea>,
    ledger: Option<Ledger>,
    /// Read from git only when an idea's session is gone (see `integrated_in_git`).
    integrated_sessions: OnceCell<HashSet<String>>,
}

impl<'a> Board<'a> {
    pub(crate) fn new(
        app: &'a App,
        store: &'a SqliteStore,
        backlog: &'a Backlog,
        project: &'a Project,
    ) -> Self {
        Board {
            app,
            store,
            backlog,
            project,
            queue: queue_order(backlog),
            ledger: Ledger::open(&app.settings.usage_path()).ok(),
            integrated_sessions: OnceCell::new(),
        }
    }

    /// The session's work is on the project's checked-out branch: what still proves an
    /// integration once the human deleted the session and its log with it.
    fn integrated_in_git(&self, session_id: &str) -> bool {
        self.integrated_sessions
            .get_or_init(|| crate::git::integrated_sessions(Path::new(&self.project.workdir)))
            .contains(session_id)
    }

    /// The status the human sees for one idea of this board.
    pub(crate) fn status_of(&self, idea: &Idea) -> Result<Status, String> {
        idea_view(self, idea).map(|(status, _)| status)
    }

    /// Every idea's stage, in backlog order.
    pub(crate) fn stages(&self) -> Result<Vec<Stage>, String> {
        self.backlog
            .ideas
            .iter()
            .map(|idea| self.status_of(idea).map(|status| status.stage()))
            .collect()
    }
}

/// The board's payload: `{queue, groups, summary, nightReport}`.
pub(crate) fn board_json(board: &Board) -> Result<Value, String> {
    let (groups, statuses) = groups_json(board)?;
    Ok(json!({
        "queue": queue_json(board)?,
        "groups": groups,
        // The overview's figures: every idea's status, counted once here.
        "summary": summarize(&statuses),
        "nightReport": night_report_json(board)?,
    }))
}

fn queue_json(board: &Board) -> Result<Value, String> {
    let running = idea_queue::queue_running(board.app, &board.project.id);
    let autonomous = running
        && board
            .backlog
            .last_run
            .as_ref()
            .is_some_and(|run| run.autonomous);
    // A halted idea is still in the queue order but waits for the human ("À toi"),
    // and the queue skips it: it is not ready.
    let ready = board
        .queue
        .iter()
        .filter(|idea| idea.halted.is_none())
        .count();
    let next = backlog::next_ready(board.backlog).map(|idea| {
        json!({
            "id": idea.id,
            "text": idea.text,
            "title": backlog::title(&idea.text, idea.plan.as_deref()),
        })
    });
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

/// The ideas grouped by stage, and their statuses (one log read per idea).
fn groups_json(board: &Board) -> Result<(Value, Vec<Status>), String> {
    let mut views: Vec<(Stage, &Idea, Value)> = Vec::new();
    let mut statuses = Vec::new();
    for idea in &board.backlog.ideas {
        let (status, view) = idea_view(board, idea)?;
        statuses.push(status);
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

    Ok((json!(groups), statuses))
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
    let mut report = IdeaReport {
        integrated: false,
        criteria: None,
        decisions: Vec::new(),
        ran_workflow: false,
    };
    let mut running = false;
    let mut session_state = None;

    if matches!(idea.state, IdeaState::Started | IdeaState::Framing)
        && let Some(session_id) = idea.session_id.as_deref()
    {
        running = app.turns.is_running(session_id);
        session = store.session(session_id)?;
        session_state = session.as_ref().map(|s| s.state(running));
        report = app.idea_report(store, session_id)?;
        if session.is_none() {
            report.integrated = board.integrated_in_git(session_id);
        }
    }

    // A framing session where the human launched a workflow went past
    // framing without the queue: show its run, not "resume framing".
    let ran_workflow = report.ran_workflow;
    let progress = if idea.state == IdeaState::Started || ran_workflow {
        progress(session_state, running, report.integrated)
    } else {
        None
    };
    let status = if idea.state == IdeaState::Framing && ran_workflow {
        status(
            &Idea {
                state: IdeaState::Started,
                ..idea.clone()
            },
            progress,
        )
    } else {
        status(idea, progress)
    };
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

    let spans = if idea.state == IdeaState::Started
        && let Some(started_at) = idea.started_at
        && let Some(session_id) = idea.session_id.as_deref()
        && session.is_some()
    {
        Some(run_spans(
            &store.running_changes(session_id, started_at)?,
            running.then(crate::domain::clock::now_ms),
        ))
    } else {
        None
    };

    let duration_ms = spans.as_ref().map(|spans| {
        spans
            .iter()
            .map(|&(start, end)| end.saturating_sub(start))
            .sum::<u64>()
    });

    let tokens = if let (Some(spans), Some(ledger)) = (&spans, &board.ledger) {
        // ponytail: counts the project's calls in the run spans, exact while the
        // queue runs one idea at a time; the spans also exclude the waits for
        // the human and the later integration.
        Some(
            spans
                .iter()
                .filter_map(|&(start, end)| {
                    ledger
                        .summary(&Filter {
                            group_id: Some(project_id.to_string()),
                            since_ms: Some(start as i64),
                            until_ms: Some(end as i64),
                            ..Default::default()
                        })
                        .ok()
                        .map(|summary| {
                            summary.prompt_tokens.saturating_sub(summary.cached_tokens)
                                + summary.completion_tokens
                        })
                })
                .sum::<u64>(),
        )
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
        "title": backlog::title(&idea.text, idea.plan.as_deref()),
        "description": backlog::description(&idea.text, idea.plan.as_deref()),
        "attachments": serde_json::to_value(&idea.attachments).unwrap_or(Value::Array(vec![])),
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
        "updatedAt": idea.updated_at,
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
    // Integration is final: it holds even once the session is gone.
    if integrated {
        return Some(Progress::Integrated);
    }
    let state = state?;
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

/// The spans a session ran, from its `running.changed` flips: the time an idea
/// worked, without the waits for the human or a later integration.
fn run_spans(changes: &[(u64, bool)], now_ms: Option<u64>) -> Vec<(u64, u64)> {
    let mut spans = Vec::new();
    let mut open = None;
    for &(timestamp, is_running) in changes {
        if is_running {
            if open.is_none() {
                open = Some(timestamp);
            }
        } else if let Some(start) = open.take() {
            spans.push((start, timestamp));
        }
    }
    if let Some(start) = open
        && let Some(now) = now_ms
    {
        spans.push((start, now));
    }
    spans
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

#[derive(Debug, Clone)]
pub(crate) struct IdeaReport {
    integrated: bool,
    criteria: Option<CriteriaCounts>,
    decisions: Vec<DecisionView>,
    ran_workflow: bool,
}

pub(crate) fn idea_report(events: &[Event]) -> IdeaReport {
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

    let ran_workflow = events
        .iter()
        .any(|event| matches!(event, Event::WorkflowStarted { .. }));
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
        ran_workflow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmSettings, Settings};
    use crate::domain::preferences::ModelParams;
    use crate::notifier::Notifier;
    use crate::web::turns::Turns;
    use backlog::{BacklogEvent, IdeaAttachment};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn app() -> App {
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
            db_path: PathBuf::from(":memory:"),
        };
        let notifier = Arc::new(Notifier::default());
        App {
            turns: Turns::new(settings.clone(), notifier.clone()),
            settings,
            port: 0,
            listen: std::net::Ipv4Addr::LOCALHOST.into(),
            notifier: notifier.clone(),
            skill_folds: Mutex::new(HashMap::new()),
            idea_reports: Mutex::new(HashMap::new()),
            idea_queues: Mutex::default(),
            worktree_scan: Mutex::default(),
            server_runs: Arc::new(Mutex::new(HashMap::new())),
            plugins: Arc::new(crate::plugins::PluginManager::new(
                crate::tools::Sandbox::Off,
                std::env::temp_dir(),
                std::env::temp_dir(),
                notifier,
            )),
            agents_home: std::env::temp_dir(),
            logins: crate::web::login::Logins::default(),
            update_check: Default::default(),
        }
    }
    #[test]
    fn board_view_lists_idea_attachments() {
        let app = app();
        let store = app.store().unwrap();
        let project = store.project_for("/tmp/attach").unwrap();

        store
            .append_idea_event(
                &project.id,
                &BacklogEvent::Added {
                    idea_id: "i1".into(),
                    text: "t".into(),
                    attachments: vec![IdeaAttachment {
                        filename: "shot.png".into(),
                        mime_type: "image/png".into(),
                        size: 3,
                        data: "data:image/png;base64,AAA=".into(),
                    }],
                },
            )
            .unwrap();

        let backlog = store.backlog(&project.id).unwrap();
        let project = store.project(&project.id).unwrap().unwrap();
        let board = Board {
            app: &app,
            store: &store,
            backlog: &backlog,
            project: &project,
            queue: queue_order(&backlog),
            ledger: None,
            integrated_sessions: OnceCell::new(),
        };

        let (_, value) = idea_view(&board, &backlog.ideas[0]).unwrap();
        let attachments = value["attachments"].as_array().unwrap();
        assert_eq!(attachments[0]["filename"], "shot.png");
        assert_eq!(attachments[0]["data"], "data:image/png;base64,AAA=");
    }

    #[test]
    fn progress_maps_each_session_state() {
        assert!(progress(None, false, false).is_none());
        assert_eq!(progress(None, false, true), Some(Progress::Integrated));
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
        assert!(!report.ran_workflow);
    }

    #[test]
    fn run_spans_sums_closed_spans() {
        let spans = run_spans(&[(10, true), (20, false), (30, true), (40, false)], None);
        assert_eq!(spans, vec![(10, 20), (30, 40)]);
        assert_eq!(
            spans.iter().map(|&(start, end)| end - start).sum::<u64>(),
            20
        );
    }

    #[test]
    fn run_spans_repeated_true_does_not_reopen() {
        let spans = run_spans(&[(10, true), (20, true), (30, false)], None);
        assert_eq!(spans, vec![(10, 30)]);
    }

    #[test]
    fn run_spans_closes_open_span_at_now_when_running() {
        let spans = run_spans(&[(10, true)], Some(50));
        assert_eq!(spans, vec![(10, 50)]);
    }

    #[test]
    fn run_spans_drops_open_span_when_not_running() {
        assert_eq!(run_spans(&[(10, true)], None), Vec::new());
    }

    #[test]
    fn run_spans_ignores_leading_false() {
        let spans = run_spans(&[(5, false), (10, true), (20, false)], None);
        assert_eq!(spans, vec![(10, 20)]);
    }
}
