//! The cross-project Backlog page: one table row per project with its idea
//! counts and queue controls. It gathers each project's board so the human can
//! see what needs attention and start/stop queues without opening each project.
//! `routes.rs` only dispatches here.

use std::net::TcpStream;

use serde_json::{Value, json};

use super::App;
use super::http::send_json;
use super::idea_board::{Board, board_json};
use crate::store::SqliteStore;

/// `GET /api/backlog`: the cross-project table of every project's board.
pub fn list_backlog(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    send_json(stream, 200, &backlog_view(app, &store)?)
}

/// The table's payload: totals across all projects and one row per project that
/// has ideas in a stage or a running queue.
fn backlog_view(app: &App, store: &SqliteStore) -> Result<Value, String> {
    let mut rows: Vec<Value> = Vec::new();

    let mut total_yours = 0usize;
    let mut total_running = 0usize;
    let mut total_ready = 0usize;
    let mut total_drafts = 0usize;
    let mut total_done = 0usize;
    let mut total_projects = 0usize;

    for project in store.projects()? {
        total_projects += 1;

        let backlog = store.backlog(&project.id)?;
        let board = Board::new(app, store, &backlog, &project);
        let board = board_json(&board)?;

        let mut yours = 0usize;
        let mut running = 0usize;
        let mut drafts = 0usize;
        let mut done = 0usize;

        for group in board["groups"].as_array().into_iter().flatten() {
            let Some(ideas) = group["ideas"].as_array() else {
                continue;
            };
            let count = ideas.len();

            match group["stage"].as_str() {
                Some("yours") => yours += count,
                Some("running") => running += count,
                Some("drafts") => drafts += count,
                Some("done") => done += count,
                _ => {}
            }
        }

        let queue = &board["queue"];
        let ready = queue["ready"].as_u64().unwrap_or(0) as usize;
        let is_running = queue["running"].as_bool().unwrap_or(false);

        total_yours += yours;
        total_running += running;
        total_ready += ready;
        total_drafts += drafts;
        total_done += done;

        // An empty project is noise in the table; keep a row only when it has
        // an idea in a stage or its queue is already running.
        if yours > 0 || running > 0 || ready > 0 || drafts > 0 || done > 0 || is_running {
            rows.push(json!({
                "project": { "id": project.id, "name": project.name },
                "yours": yours,
                "running": running,
                "ready": ready,
                "drafts": drafts,
                "done": done,
                "queue": {
                    "running": is_running,
                    "autonomous": queue["autonomous"].as_bool().unwrap_or(false),
                    "next": queue["next"].clone(),
                },
            }));
        }
    }

    // Running queues need the human first; among stopped rows, the ideas waiting
    // for a decision and the largest ready pile are the most actionable.
    rows.sort_by(|a, b| {
        let a_running = a["queue"]["running"].as_bool().unwrap_or(false);
        let b_running = b["queue"]["running"].as_bool().unwrap_or(false);
        let a_yours = a["yours"].as_u64().unwrap_or(0);
        let b_yours = b["yours"].as_u64().unwrap_or(0);
        let a_ready = a["ready"].as_u64().unwrap_or(0);
        let b_ready = b["ready"].as_u64().unwrap_or(0);
        let a_name = a["project"]["name"].as_str().unwrap_or("");
        let b_name = b["project"]["name"].as_str().unwrap_or("");

        b_running
            .cmp(&a_running)
            .then(b_yours.cmp(&a_yours))
            .then(b_ready.cmp(&a_ready))
            .then(a_name.cmp(b_name))
    });

    Ok(json!({
        "total": {
            "yours": total_yours,
            "running": total_running,
            "ready": total_ready,
            "drafts": total_drafts,
            "done": total_done,
            "projects": total_projects,
        },
        "projects": rows,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmSettings, Settings};
    use crate::domain::preferences::ModelParams;
    use crate::notifier::Notifier;
    use crate::web::turns::Turns;
    use backlog::BacklogEvent;
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
            notifier,
            skill_folds: Mutex::new(HashMap::new()),
            idea_queues: Mutex::default(),
            server_runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[test]
    fn backlog_view_summarizes_boards_across_projects() {
        let app = app();
        let store = app.store().unwrap();

        // Project A: one completed idea and one ready idea.
        let a = store.project_for("/tmp/backlog-a").unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Added {
                    idea_id: "a1".into(),
                    text: "done".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Completed {
                    idea_id: "a1".into(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Added {
                    idea_id: "a2".into(),
                    text: "ready".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();
        // A ready idea is queued only once framed in a session.
        let framing = store.create_session(&a, "framing").unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Framing {
                    idea_id: "a2".into(),
                    session_id: framing,
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Ready {
                    idea_id: "a2".into(),
                    plan: "plan".into(),
                },
            )
            .unwrap();

        // Project B: nothing.
        store.project_for("/tmp/backlog-b").unwrap();

        let view = backlog_view(&app, &store).unwrap();

        assert_eq!(view["total"]["projects"], 2);
        assert_eq!(view["total"]["done"], 1);
        assert_eq!(view["total"]["ready"], 1);

        let projects = view["projects"].as_array().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["project"]["id"], a.id);
        assert_eq!(projects[0]["done"], 1);
        assert_eq!(projects[0]["ready"], 1);
        assert_eq!(projects[0]["queue"]["running"], false);
    }
}
