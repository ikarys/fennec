//! Web adapter: the local server behind the UI. std only, one thread per
//! connection, bound to 127.0.0.1.

mod assets;
mod children;
mod event_stream;
mod guard;
mod http;
mod notifier;
mod preview;
mod routes;
mod turns;
mod workflow_routes;

use crate::agent::ports::EventLog;
use crate::config::Settings;
use crate::domain::session_skills::{SessionSkills, fold_skills};
use crate::domain::skill::Skill;
use crate::store::SqliteStore;
use notifier::Notifier;
use serde_json::json;
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use turns::Turns;

/// What every request handler shares.
struct App {
    settings: Settings,
    port: u16,
    turns: Turns,
    notifier: Arc<Notifier>,
    /// The skills fold per session, keyed by the log's max seq: the log is
    /// append-only, so an unchanged max seq means an unchanged fold. The fold
    /// is O(log) and the UI refetches it on every relevant event; caching keeps
    /// the steady state to a max-seq lookup.
    skill_folds: Mutex<HashMap<String, (u64, SessionSkills)>>,
}

impl App {
    /// A connection per request: SQLite connections are not shared across
    /// threads.
    fn store(&self) -> Result<SqliteStore, String> {
        Ok(SqliteStore::open(&self.settings.db_path)?)
    }

    /// The session's skills fold, cached against the log's max seq (see
    /// `skill_folds`): the O(log) re-read runs only when a new event has
    /// followed. `skills` is the project's registry — discovery I/O stays with
    /// the caller, so a cache hit re-reads neither the log nor the skills dirs.
    fn folded_skills(
        &self,
        store: &SqliteStore,
        id: &str,
        skills: &[Skill],
    ) -> Result<SessionSkills, String> {
        let last = store.last_seq(id)?;
        let mut cache = self.skill_folds.lock().unwrap();
        if cache.get(id).is_some_and(|(seq, _)| *seq == last) {
            return Ok(cache[id].1.clone());
        }
        let folded = fold_skills(&store.events(id)?, skills);
        cache.insert(id.to_string(), (last, folded.clone()));
        Ok(folded)
    }
}

pub fn serve(port: u16, settings: Settings) -> Result<(), String> {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("127.0.0.1:{port}: {e}"))?;
    eprintln!(
        "fennec · {} · http://127.0.0.1:{port}",
        settings.llm.defaults.model
    );
    let notifier = Arc::new(Notifier::default());
    let app = Arc::new(App {
        turns: Turns::new(settings.clone(), notifier.clone()),
        settings,
        port,
        notifier,
        skill_folds: Mutex::new(HashMap::new()),
    });
    // Fail now rather than on the first request.
    app.store()?;
    for stream in listener.incoming().flatten() {
        let app = app.clone();
        std::thread::spawn(move || {
            let mut stream = stream;
            if let Err(e) = handle(&app, &mut stream) {
                let _ = http::send_json(&mut stream, 400, &json!({ "error": e }));
            }
        });
    }
    Ok(())
}

fn handle(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let req = http::read_request(stream)?;
    if let Err(refusal) = guard::check(&req, app.port) {
        return http::error(stream, refusal.status, refusal.message);
    }
    routes::handle(app, &req, stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmSettings, Settings};
    use crate::domain::event::{Event, ToolCall};
    use crate::domain::preferences::ModelParams;
    use crate::domain::skill::Skill;
    use crate::store::SqliteStore;
    use serde_json::json;
    use std::path::PathBuf;

    /// A minimal `App`; `folded_skills` takes the store, so its settings are
    /// unused here.
    fn app() -> App {
        let settings = Settings {
            llm: LlmSettings {
                base_url: "http://localhost".into(),
                api_key: None,
                defaults: ModelParams {
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
        }
    }

    fn skill(id: &str) -> Skill {
        Skill {
            id: id.into(),
            description: format!("{id} description"),
            prompt: "prompt".into(),
            directory: PathBuf::from(id),
        }
    }

    fn load_call(id: &str, skill_id: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: "load_skill".into(),
                arguments: json!({"skillId": skill_id}),
                parse_error: None,
            },
        }
    }

    fn load_result(id: &str) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: id.into(),
            result: crate::domain::event::ToolResult::success("ok".into(), false),
        }
    }

    #[test]
    fn folded_skills_caches_by_watermark_and_invalidates_on_a_new_event() {
        let app = app();
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "s").unwrap();
        let skills = vec![skill("alpha"), skill("beta")];

        store.append(&sid, &load_call("c1", "alpha")).unwrap();
        store.append(&sid, &load_result("c1")).unwrap();

        let first = app.folded_skills(&store, &sid, &skills).unwrap();
        assert_eq!(first.loaded, vec!["alpha"]);

        // The log is unchanged, so the cache answers with the same fold.
        assert_eq!(app.folded_skills(&store, &sid, &skills).unwrap(), first);

        // A new event advances the watermark and the fold picks it up.
        store.append(&sid, &load_call("c2", "beta")).unwrap();
        store.append(&sid, &load_result("c2")).unwrap();
        let third = app.folded_skills(&store, &sid, &skills).unwrap();
        assert_eq!(third.loaded, vec!["alpha", "beta"]);
    }
}
