//! Deleting sessions on the human's request (contract: docs/api.md): the log, the child sessions and the worktree with its branch. The usage ledger keeps the calls. The session's rules go with the session.

use crate::chat;
use crate::domain::project::SessionSummary;
use crate::store::SqliteStore;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use crate::worktree;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

/// `POST /api/sessions/delete`: the human's deletion of top-level sessions.
pub fn delete_sessions(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let body = req.json()?;
    let Some(items) = body.get("ids").and_then(Value::as_array) else {
        return error(stream, 400, "ids must be a non-empty list of session ids");
    };
    if items.is_empty() || items.len() > 500 {
        return error(stream, 400, "ids must be a non-empty list of session ids");
    }
    let ids: Option<Vec<String>> = items
        .iter()
        .map(|item| item.as_str().map(String::from))
        .collect();
    let Some(ids) = ids else {
        return error(stream, 400, "ids must be a non-empty list of session ids");
    };
    let store = app.store()?;
    let mut plans: Vec<(String, SessionSummary, Vec<String>)> = Vec::new();
    for id in &ids {
        let Some(session) = store.session(id)? else {
            return error(stream, 404, &format!("no such session: {id}"));
        };
        if session.parent_id.is_some() {
            return error(stream, 400, "only top-level sessions can be deleted");
        }
        let tree = store.session_tree(id)?;
        plans.push((id.clone(), session, tree));
    }
    let mut running: Vec<String> = Vec::new();
    for (id, _, tree) in &plans {
        if tree.iter().any(|sid| app.turns.is_running(sid)) && !running.contains(id) {
            running.push(id.clone());
        }
    }
    if !running.is_empty() {
        return send_json(
            stream,
            409,
            &json!({ "error": "a turn is running", "running": running }),
        );
    }
    let mut deleted: Vec<String> = Vec::new();
    let mut failed: Vec<Value> = Vec::new();
    let rule_store =
        rules::RuleStore::open(&app.settings.rules_path()).map_err(|e| e.to_string())?;
    for (id, session, tree) in plans {
        match remove_tree(app, &store, &rule_store, &session, &tree) {
            Ok(None) => deleted.push(id),
            Ok(Some(e)) => failed.push(json!({ "id": id, "error": e })),
            Err(e) => return Err(e.into()),
        }
    }
    send_json(
        stream,
        200,
        &json!({ "deleted": deleted, "failed": failed }),
    )
}

/// Removes one top-level session and its children: the worktree with its branch, the log, the
/// session rules, the canvas artifacts, the cached skill folds. `Ok(Some(error))` when the worktree could not be
/// removed: nothing was deleted, the human can retry.
pub(crate) fn remove_tree(
    app: &App,
    store: &SqliteStore,
    rule_store: &rules::RuleStore,
    session: &SessionSummary,
    tree: &[String],
) -> Result<Option<String>, String> {
    // A child session shares its root's worktree: removing the root's removes theirs.
    if let Some(project) = store.project(&session.project_id)? {
        if chat::is_chat(&project) {
            // The chat's per-session scratch goes with the session; a pure
            // conversation never created it.
            let scratch = chat::session_dir(&app.settings.db_path, &session.id);
            match std::fs::remove_dir_all(&scratch) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Ok(Some(e.to_string())),
            }
        } else if let Some(wt) = app.place(store, session, &project)?.worktree {
            let Some((top, _)) = worktree::top(Path::new(&project.workdir)) else {
                return Ok(Some("the project is not in a git repository".into()));
            };
            if let Err(e) = worktree::remove(&top, &wt) {
                return Ok(Some(e));
            }
        }
    }
    store.delete_sessions(tree).map_err(|e| e.to_string())?;
    let canvas =
        canvas::CanvasStore::open(&app.settings.canvas_path()).map_err(|e| e.to_string())?;
    for sid in tree {
        rule_store.delete_session(sid).map_err(|e| e.to_string())?;
        canvas.delete_session(sid).map_err(|e| e.to_string())?;
    }
    let mut folds = crate::locks::lock(&app.skill_folds);
    for sid in tree {
        folds.remove(sid);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmSettings, Settings};
    use crate::domain::id::new_id;
    use crate::domain::preferences::ModelParams;
    use crate::notifier::Notifier;
    use crate::web::turns::Turns;
    use serde_json::json;
    use std::collections::HashMap;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Deleting a chat session removes its per-session scratch directory.
    #[test]
    fn delete_session_removes_the_chat_scratch() {
        // Its own store directory: the scratch root is `chat/` next to the
        // database, so a shared `/tmp/chat` would race with other tests.
        let base = std::env::temp_dir().join(format!("fennec-chat-delete-{}", new_id()));
        std::fs::create_dir_all(&base).unwrap();
        let db_path = base.join("fennec.db");
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
            db_path: db_path.clone(),
        };
        let notifier = Arc::new(Notifier::default());
        let app = App {
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
        };
        let store = app.store().unwrap();
        let project = store
            .chat_project(&chat::workdir(&db_path).display().to_string())
            .unwrap();
        let session = store.create_session(&project, "chat").unwrap();

        // The agent wrote a file during the conversation.
        let scratch = chat::session_dir(&db_path, &session);
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join("notes.txt"), "hello").unwrap();
        assert!(scratch.exists());

        let req = Request {
            method: "POST".into(),
            path: "/api/sessions/delete".into(),
            query: String::new(),
            headers: Vec::new(),
            body: json!({ "ids": [session] }).to_string().into_bytes(),
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _client = TcpStream::connect(addr).unwrap();
        let mut server = listener.accept().unwrap().0;
        delete_sessions(&app, &req, &mut server).unwrap();

        assert!(!scratch.exists());
        assert!(store.session(&session).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&base);
    }
}
