//! The cross-project backlog dashboard (contract: `docs/api.md`): a summary of
//! the tasks done on every project. `routes.rs` only dispatches here.

use std::net::TcpStream;

use serde_json::{Value, json};

use super::App;
use super::http::send_json;
use crate::agent::ports::EventLog;
use crate::domain::event::Event;
use crate::store::SqliteStore;
use backlog::{Idea, IdeaState};

/// `GET /api/backlog`: every project's done tasks, most done first.
pub fn list_backlog(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    send_json(stream, 200, &backlog_view(&store)?)
}

/// The dashboard's payload: one entry per project, each listing its done tasks
/// (most recent first), and the totals across all of them.
fn backlog_view(store: &SqliteStore) -> Result<Value, String> {
    let mut projects: Vec<Value> = Vec::new();
    let mut total_done = 0usize;

    for project in store.projects()? {
        let mut tasks: Vec<Value> = Vec::new();
        for idea in store.ideas(&project.id)? {
            if let Some(status) = done_status(store, &idea)? {
                total_done += 1;
                tasks.push(json!({
                    "id": idea.id,
                    "text": idea.text,
                    "status": status,
                    "updatedAt": idea.updated_at,
                }));
            }
        }
        tasks.sort_by_key(|task| std::cmp::Reverse(task["updatedAt"].as_u64().unwrap_or(0)));
        projects.push(json!({
            "id": project.id,
            "name": project.name,
            "doneCount": tasks.len(),
            "tasks": tasks,
        }));
    }

    projects.sort_by_key(|project| std::cmp::Reverse(project["doneCount"].as_u64().unwrap_or(0)));

    Ok(json!({
        "projects": projects,
        "total": { "done": total_done, "projects": projects.len() },
    }))
}

/// The status of a done task, or `None` when it is not done. A task is done
/// when the human completed it, or started it and then integrated its work —
/// the same two states the project board's `done` group shows.
fn done_status(store: &SqliteStore, idea: &Idea) -> Result<Option<&'static str>, String> {
    match idea.state {
        IdeaState::Completed => Ok(Some("completed")),
        IdeaState::Started => {
            let Some(session_id) = idea.session_id.as_deref() else {
                return Ok(None);
            };
            let integrated = store
                .events(session_id)?
                .iter()
                .any(|event| matches!(event, Event::IntegrationApplied { .. }));
            Ok(integrated.then_some("integrated"))
        }
        _ => Ok(None),
    }
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

    fn integrated_event() -> Event {
        Event::IntegrationApplied {
            branch: "b".into(),
            base: "main".into(),
            commits: Vec::new(),
            head: "h".into(),
            message: String::new(),
            acknowledged: Vec::new(),
            protected_tests: Vec::new(),
        }
    }

    #[test]
    fn backlog_view_lists_done_tasks_per_project() {
        let app = app();
        let store = app.store().unwrap();

        // Project A: one completed, one started+integrated, one started and not
        // integrated (the last one is not done).
        let a = store.project_for("/tmp/backlog-a").unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Added {
                    idea_id: "a1".into(),
                    text: "done explicit".into(),
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

        let sa = store.create_session(&a, "sa").unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Added {
                    idea_id: "a2".into(),
                    text: "integrated".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Framing {
                    idea_id: "a2".into(),
                    session_id: sa.clone(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Started {
                    idea_id: "a2".into(),
                    autonomous: false,
                },
            )
            .unwrap();
        store.append(&sa, &integrated_event()).unwrap();

        let sb = store.create_session(&a, "sb").unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Added {
                    idea_id: "a3".into(),
                    text: "not integrated".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Framing {
                    idea_id: "a3".into(),
                    session_id: sb.clone(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &a.id,
                &BacklogEvent::Started {
                    idea_id: "a3".into(),
                    autonomous: false,
                },
            )
            .unwrap();

        // Project B: one completed.
        let b = store.project_for("/tmp/backlog-b").unwrap();
        store
            .append_idea_event(
                &b.id,
                &BacklogEvent::Added {
                    idea_id: "b1".into(),
                    text: "b done".into(),
                    attachments: Vec::new(),
                },
            )
            .unwrap();
        store
            .append_idea_event(
                &b.id,
                &BacklogEvent::Completed {
                    idea_id: "b1".into(),
                },
            )
            .unwrap();

        let view = backlog_view(&store).unwrap();

        assert_eq!(view["total"]["done"], 3);
        assert_eq!(view["total"]["projects"], 2);

        let projects = view["projects"].as_array().unwrap();
        // Project A holds the most done tasks, so it leads the list.
        assert_eq!(projects[0]["id"], a.id);
        assert_eq!(projects[0]["doneCount"], 2);
        let a_tasks = projects[0]["tasks"].as_array().unwrap();
        let statuses: Vec<&str> = a_tasks
            .iter()
            .map(|task| task["status"].as_str().unwrap())
            .collect();
        assert!(statuses.contains(&"completed"));
        assert!(statuses.contains(&"integrated"));

        assert_eq!(projects[1]["id"], b.id);
        assert_eq!(projects[1]["doneCount"], 1);
    }
}
