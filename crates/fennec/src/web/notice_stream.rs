//! The human's attention, server-wide: a snapshot of the sessions that need
//! their eye (a question, a workflow waiting on a choice, work ready to
//! integrate, an error), so the UI can show it wherever the human is and fire
//! a system notification. Child sessions are a workflow's inner steps and stay
//! silent.

use super::App;
use crate::agent::ports::EventLog;
use crate::domain::attention::{self, Attention};
use crate::domain::budget::BudgetStatus;
use crate::domain::lang::Lang;
use crate::domain::project::SessionSummary;
use crate::git;
use crate::store::SqliteStore;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant};
use token_usage::display::usd;

/// A comment line now and then, so a closed browser tab is noticed.
const PING: Duration = Duration::from_secs(15);

/// The budgets are recomputed at most this often on a wake.
const BUDGET_REFRESH: Duration = Duration::from_secs(30);

/// The attention set and the last log seq read per session.
type AttentionSet = (HashMap<String, Attention>, HashMap<String, u64>);

pub fn serve(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    // What needs the human right now. Sent on connect, then re-sent only when
    // it changes; a running turn is recomputed when it stops, dropped when it
    // starts.
    let (mut attention, mut last_seq) = current_attention(app, &store)?;
    let mut budgets = budget_notices(app, &store)?;
    let mut last_budgets = Instant::now();
    // Plugin blocks move the snapshot too, on their own version, so a tab
    // sees new plugin data without a session log moving.
    let mut plugins_version = app.plugins.version();
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n",
        )
        .map_err(|e| e.to_string())?;
    if !write_frame(stream, &snapshot(&attention, &budgets, plugins_version)) {
        return Ok(()); // the browser went away
    }
    let mut last_write = Instant::now();
    loop {
        let seen = app.notifier.generation();
        let mut changed = refresh(app, &store, &mut attention, &mut last_seq)?;
        // ponytail: an alert shows up to 30 s late; upgrade: recompute when a turn ends.
        if last_budgets.elapsed() >= BUDGET_REFRESH {
            let fresh = budget_notices(app, &store)?;
            if fresh != budgets {
                budgets = fresh;
                changed = true;
            }
            last_budgets = Instant::now();
        }
        if app.plugins.version() != plugins_version {
            plugins_version = app.plugins.version();
            changed = true;
        }
        if changed {
            if !write_frame(stream, &snapshot(&attention, &budgets, plugins_version)) {
                return Ok(());
            }
            last_write = Instant::now();
        } else if last_write.elapsed() >= PING {
            if stream.write_all(b": ping\n\n").is_err() {
                return Ok(()); // the browser went away
            }
            last_write = Instant::now();
        }
        app.notifier.wait_past(seen, PING);
    }
}

/// The attention set over every top-level session, plus the last log seq read
/// per session, so `refresh` recomputes only what actually moved.
fn current_attention(app: &App, store: &SqliteStore) -> Result<AttentionSet, String> {
    let running = app.turns.running_ids();
    let mut set = HashMap::new();
    let mut seqs = HashMap::new();
    let mut pending = HashMap::new();
    for mut session in store.sessions(None)? {
        if running.contains(&session.id) {
            continue;
        }
        let seq = store.last_seq(&session.id)?;
        seqs.insert(session.id.clone(), seq);
        let events = store.events(&session.id)?;
        confirm_pending_work(store, &mut pending, &mut session)?;
        if let Some(item) = attention::attention(&session, &events, false) {
            let item = if item.kind == attention::AttentionKind::AwaitingIntegration {
                mark_conflict(app, store, &session, item)?
            } else {
                item
            };
            set.insert(session.id.clone(), item);
        }
    }
    Ok((set, seqs))
}

/// Recompute attention for the sessions whose log moved since the last wake —
/// a turn ending, an integration, a discard all land here — drop what is now
/// running, prune what was deleted, and report whether the set moved.
fn refresh(
    app: &App,
    store: &SqliteStore,
    attention: &mut HashMap<String, Attention>,
    last_seq: &mut HashMap<String, u64>,
) -> Result<bool, String> {
    let running = app.turns.running_ids();
    // shortcut: the full SESSION_SELECT (per-session subqueries) runs on every
    // wake; sub-ms for a human's handful of sessions. Cache the list in App and
    // query only the moved sessions if it ever weighs.
    let mut sessions = store.sessions(None)?;
    let mut changed = false;
    let mut pending = HashMap::new();
    for session in &mut sessions {
        if running.contains(&session.id) {
            // A running turn needs no attention: drop it if it lingers.
            if attention.remove(&session.id).is_some() {
                changed = true;
            }
            continue;
        }
        let seq = store.last_seq(&session.id)?;
        if last_seq.get(&session.id) == Some(&seq) {
            continue; // unchanged since the last wake
        }
        last_seq.insert(session.id.clone(), seq);
        let events = store.events(&session.id)?;
        confirm_pending_work(store, &mut pending, session)?;
        let mut next = attention::attention(session, &events, false);
        if let Some(item) = &next
            && item.kind == attention::AttentionKind::AwaitingIntegration
        {
            next = Some(mark_conflict(app, store, session, item.clone())?);
        }
        if attention.get(&session.id).cloned() != next {
            changed = true;
            match next {
                Some(item) => {
                    attention.insert(session.id.clone(), item);
                }
                None => {
                    attention.remove(&session.id);
                }
            }
        }
    }
    // A session deleted since the last wake keeps no attention and no seq.
    // `last_seq` is the superset: every attention entry has a seq entry, so
    // iterating it purges both, keeping `last_seq` bounded to live sessions.
    for id in last_seq.keys().cloned().collect::<Vec<_>>() {
        if !sessions.iter().any(|s| s.id == id) {
            last_seq.remove(&id);
            if attention.remove(&id).is_some() {
                changed = true;
            }
        }
    }
    Ok(changed)
}

/// Finished work awaits integration only while its branch still holds commits
/// past its base, as on the home cards (`list_projects`): work integrated or
/// dropped by hand (a branch deleted, reset onto its base) no longer needs the
/// human. One git call per project per pass, and only for finished work.
fn confirm_pending_work(
    store: &SqliteStore,
    pending: &mut HashMap<String, HashSet<String>>,
    session: &mut SessionSummary,
) -> Result<(), String> {
    if !session.awaiting_integration {
        return Ok(());
    }
    if !pending.contains_key(&session.project_id) {
        let found = match store.project(&session.project_id)? {
            Some(p) => git::sessions_with_unintegrated_work(Path::new(&p.workdir)),
            None => HashSet::new(),
        };
        pending.insert(session.project_id.clone(), found);
    }
    session.awaiting_integration = pending[&session.project_id].contains(&session.id);
    Ok(())
}

/// Finished work whose branch conflicts with the human's branch is a `Conflict`
/// attention, not an `AwaitingIntegration` one: the human cannot integrate it
/// until the agent resolves the markers.
fn mark_conflict(
    app: &App,
    store: &SqliteStore,
    session: &SessionSummary,
    item: Attention,
) -> Result<Attention, String> {
    // ponytail: recomputed only when the session's log moves; a commit on the
    // human's branch alone does not refresh it until the next page load.
    let Some(project) = store.project(&session.project_id)? else {
        return Ok(item);
    };
    let Some(wt) = app.place(store, session, &project)?.worktree else {
        return Ok(item);
    };
    let Some((top, _)) = crate::worktree::top(Path::new(&project.workdir)) else {
        return Ok(item);
    };
    let conflicts = crate::integration::conflicts(&top, &wt);
    if conflicts.is_empty() {
        Ok(item)
    } else {
        Ok(Attention {
            kind: attention::AttentionKind::Conflict,
            detail: conflicts.join(", "),
            ..item
        })
    }
}

/// The projects whose monthly budget is at alert or exceeded, sorted by id.
fn budget_notices(app: &App, store: &SqliteStore) -> Result<Vec<Value>, String> {
    let lang = Lang::from_code(&store.preferences()?.display.language);
    let mut notices = Vec::new();
    for project in store.projects()? {
        let Some(budget) = crate::budget::of_project(&app.settings, store, &project.id)? else {
            continue;
        };
        let status = match budget.status {
            BudgetStatus::Alert => "alert",
            BudgetStatus::Exceeded => "exceeded",
            BudgetStatus::Under => continue,
        };
        let detail = crate::domain::budget::alert(
            lang,
            &usd(budget.spent, lang.locale()),
            &usd(budget.limit, lang.locale()),
        );
        notices.push(json!({
            "projectId": project.id,
            "project": project.name,
            "status": status,
            "detail": detail,
        }));
    }
    notices.sort_by(|a, b| {
        a["projectId"]
            .as_str()
            .unwrap_or("")
            .cmp(b["projectId"].as_str().unwrap_or(""))
    });
    Ok(notices)
}

/// The snapshot as a `data:` frame, ordered by session so it is stable.
/// `plugins_version` moves on every accepted plugin block.update, so a tab
/// refreshes on plugin data without a session log moving.
fn snapshot(set: &HashMap<String, Attention>, budgets: &[Value], plugins_version: u64) -> String {
    let mut items: Vec<&Attention> = set.values().collect();
    items.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    let payload: Vec<Value> = items
        .iter()
        .map(|a| serde_json::to_value(a).unwrap())
        .collect();
    format!(
        "data: {}\n\n",
        json!({ "items": payload, "budgets": budgets, "pluginsVersion": plugins_version })
    )
}

fn write_frame(stream: &mut TcpStream, frame: &str) -> bool {
    stream.write_all(frame.as_bytes()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::EventLog;
    use crate::config::{LlmSettings, Settings};
    use crate::domain::event::{AskType, Event, Role, ToolCall};
    use crate::domain::preferences::ModelParams;
    use crate::domain::workflow::run::RunOutcome;
    use crate::notifier::Notifier;
    use crate::web::turns::Turns;
    use serde_json::json;
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
                notifier.clone(),
            )),
            agents_home: std::env::temp_dir(),
        }
    }

    #[test]
    fn the_snapshot_lists_what_needs_the_human() {
        let app = app();
        let store = app.store().unwrap();
        let project = store.project_for("/tmp/notice-test").unwrap();
        // A session that asked a question: needs the human.
        let asking = store.create_session(&project, "asking").unwrap();
        store
            .append(
                &asking,
                &Event::ToolCall {
                    message_id: "m".into(),
                    tool_call: ToolCall {
                        id: "c1".into(),
                        name: "ask_user".into(),
                        arguments: json!({}),
                        parse_error: None,
                        raw_arguments: None,
                    },
                },
            )
            .unwrap();
        store
            .append(
                &asking,
                &Event::ChatAskUser {
                    call_id: "c1".into(),
                    question: "Which color?".into(),
                    ask_type: AskType::Text,
                    options: None,
                },
            )
            .unwrap();
        // A clean session: nothing to look at.
        let clean = store.create_session(&project, "clean").unwrap();
        store
            .append(
                &clean,
                &Event::MessageStart {
                    message_id: "m".into(),
                    role: Role::User,
                    content: Some("hi".into()),
                    is_system_generated: false,
                    metadata: None,
                    attachments: Vec::new(),
                },
            )
            .unwrap();

        let (set, _) = current_attention(&app, &store).unwrap();
        assert_eq!(
            set.len(),
            1,
            "only the asking session needs the human: {set:?}"
        );
        let item = &set[&asking];
        assert_eq!(item.kind, attention::AttentionKind::Question);
        assert_eq!(item.detail, "Which color?");
        assert!(!set.contains_key(&clean));
    }

    fn git_in(repo: &std::path::Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["-c", "user.name=t", "-c", "user.email=t@e"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A session in a fresh repository whose workflow finished `done`; its
    /// `fennec/<session>` branch holds a commit past its base when `work`, or
    /// sits on its base otherwise (the work was integrated or dropped by hand).
    fn finished_session(store: &SqliteStore, work: bool) -> String {
        let repo =
            std::env::temp_dir().join(format!("fennec-notices-{}", crate::domain::id::new_id()));
        std::fs::create_dir_all(&repo).unwrap();
        git_in(&repo, &["init", "-q"]);
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
        let project = store.project_for(repo.to_str().unwrap()).unwrap();
        let session = store.create_session(&project, "finished").unwrap();
        let base = git_in(&repo, &["rev-parse", "HEAD"]);
        git_in(
            &repo,
            &["update-ref", &format!("refs/fennec/base/{session}"), &base],
        );
        if work {
            git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "work"]);
        }
        let head = git_in(&repo, &["rev-parse", "HEAD"]);
        git_in(
            &repo,
            &["update-ref", &format!("refs/heads/fennec/{session}"), &head],
        );
        store
            .append(
                &session,
                &Event::WorkflowFinished {
                    run_id: "r".into(),
                    outcome: RunOutcome::Done,
                    reason: None,
                    stats: None,
                },
            )
            .unwrap();
        session
    }

    #[test]
    fn work_already_on_its_base_needs_no_integration() {
        let app = app();
        let store = app.store().unwrap();
        let session = finished_session(&store, false);
        let (attention, _) = current_attention(&app, &store).unwrap();
        assert!(
            !attention.contains_key(&session),
            "nothing left to integrate: {attention:?}"
        );
    }

    #[test]
    fn an_integration_clears_the_item() {
        let app = app();
        let store = app.store().unwrap();
        let session = finished_session(&store, true);

        let (mut attention, mut last_seq) = current_attention(&app, &store).unwrap();
        assert!(
            attention.contains_key(&session),
            "finished work waits to integrate: {attention:?}"
        );

        // The human integrates: no turn runs, only a log append — the item must
        // leave the snapshot on the next wake.
        store
            .append(
                &session,
                &Event::IntegrationApplied {
                    branch: "b".into(),
                    base: "main".into(),
                    commits: vec!["c1".into()],
                    head: "c1".into(),
                    message: "m".into(),
                    acknowledged: vec![],
                    protected_tests: vec![],
                },
            )
            .unwrap();
        let changed = refresh(&app, &store, &mut attention, &mut last_seq).unwrap();
        assert!(changed, "the integration moved the attention set");
        assert!(
            !attention.contains_key(&session),
            "integrated work no longer needs the human: {attention:?}"
        );
    }

    #[test]
    fn the_snapshot_is_a_stable_data_frame() {
        let set = HashMap::from([(
            "s".to_string(),
            Attention {
                session_id: "s".into(),
                project_id: "p".into(),
                title: "t".into(),
                kind: attention::AttentionKind::Error,
                detail: "boom".into(),
            },
        )]);
        let frame = snapshot(&set, &[], 7);
        assert!(frame.starts_with("data: "));
        let body = frame.trim_start_matches("data: ").trim_end();
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["items"][0]["sessionId"], "s");
        assert_eq!(value["items"][0]["kind"], "error");
        assert_eq!(value["pluginsVersion"], 7);
    }
}
