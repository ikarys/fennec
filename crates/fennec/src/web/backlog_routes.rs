//! The cross-project backlog dashboard (contract: `docs/api.md`): where the
//! human's tasks stand across every project. It gathers each project's board
//! and shows what waits for the human, what runs, the queues, then what got
//! done. `routes.rs` only dispatches here.

use std::net::TcpStream;

use serde_json::{Value, json};

use super::App;
use super::http::send_json;
use super::idea_board::{Board, board_json};
use crate::domain::project::Project;
use crate::store::SqliteStore;

/// How many done ideas the overview keeps: enough to show recent activity
/// without turning the cross-project page into a project's full history.
const RECENT_DONE: usize = 15;

/// `GET /api/backlog`: the cross-project view of every project's board.
pub fn list_backlog(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    send_json(stream, 200, &backlog_view(app, &store)?)
}

/// The dashboard's payload: the human's waiting ideas, running ideas, active
/// queues, recent done ideas, and the totals across all projects.
fn backlog_view(app: &App, store: &SqliteStore) -> Result<Value, String> {
    let mut yours: Vec<Value> = Vec::new();
    let mut running: Vec<Value> = Vec::new();
    let mut queues: Vec<Value> = Vec::new();
    let mut done: Vec<Value> = Vec::new();

    let mut total_yours = 0usize;
    let mut total_running = 0usize;
    let mut total_ready = 0usize;
    let mut total_drafts = 0usize;
    let mut total_done = 0usize;
    let mut total_projects = 0usize;
    let mut caps = json!({ "maxMinutes": 0, "maxTokens": 0 });

    for project in store.projects()? {
        total_projects += 1;

        let backlog = store.backlog(&project.id)?;
        let board = Board::new(app, store, &backlog, &project);
        let board = board_json(&board)?;

        // The caps are global preferences, so any project's board provides them.
        caps = board["queue"]["caps"].clone();

        for group in board["groups"].as_array().into_iter().flatten() {
            let Some(ideas) = group["ideas"].as_array() else {
                continue;
            };
            let count = ideas.len();

            match group["stage"].as_str() {
                Some("yours") => {
                    total_yours += count;
                    yours.extend(ideas.iter().map(|idea| summary(idea, &project)));
                }
                Some("running") => {
                    total_running += count;
                    running.extend(ideas.iter().map(|idea| summary(idea, &project)));
                }
                Some("drafts") => {
                    total_drafts += count;
                }
                Some("done") => {
                    total_done += count;
                    done.extend(ideas.iter().map(|idea| summary(idea, &project)));
                }
                _ => {}
            }
        }

        let queue = &board["queue"];
        let ready = queue["ready"].as_u64().unwrap_or(0) as usize;
        total_ready += ready;

        let is_running = queue["running"].as_bool().unwrap_or(false);
        if is_running || ready > 0 {
            queues.push(json!({
                "project": { "id": project.id, "name": project.name },
                "running": is_running,
                "autonomous": queue["autonomous"].as_bool().unwrap_or(false),
                "ready": ready,
                "next": queue["next"].clone(),
            }));
        }
    }

    sort_by_updated_at_desc(&mut yours);
    sort_by_updated_at_desc(&mut running);
    sort_by_updated_at_desc(&mut done);
    done.truncate(RECENT_DONE);

    // Running queues are the most urgent; among stopped queues, the largest
    // ready pile is the most relevant.
    queues.sort_by(|a, b| {
        let a_running = a["running"].as_bool().unwrap_or(false);
        let b_running = b["running"].as_bool().unwrap_or(false);
        let a_ready = a["ready"].as_u64().unwrap_or(0);
        let b_ready = b["ready"].as_u64().unwrap_or(0);
        b_running.cmp(&a_running).then(b_ready.cmp(&a_ready))
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
        "caps": caps,
        "yours": yours,
        "running": running,
        "queues": queues,
        "done": done,
    }))
}

/// The cross-project idea summary: the board's idea view without its heavy
/// base64 payloads, plus the project it belongs to.
fn summary(idea: &Value, project: &Project) -> Value {
    let mut summary = idea.clone();
    if let Some(object) = summary.as_object_mut() {
        object.remove("attachments");
        object.remove("plan");
        object.insert(
            "project".to_string(),
            json!({ "id": project.id, "name": project.name }),
        );
    }
    summary
}

fn sort_by_updated_at_desc(ideas: &mut [Value]) {
    ideas.sort_by_key(|idea| std::cmp::Reverse(idea["updatedAt"].as_u64().unwrap_or(0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmSettings, Settings};
    use crate::domain::preferences::ModelParams;
    use crate::web::notifier::Notifier;
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

        let done = view["done"].as_array().unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0]["id"], "a1");
        assert_eq!(done[0]["project"]["name"], a.name);
        assert!(done[0].get("attachments").is_none());

        let queues = view["queues"].as_array().unwrap();
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0]["project"]["id"], a.id);
        assert_eq!(queues[0]["ready"], 1);
    }
}
