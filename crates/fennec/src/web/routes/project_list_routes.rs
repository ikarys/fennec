//! The projects the home lists and opens, their favorites, and the commands
//! and agents a project offers to the composer.

use crate::agents;
use crate::chat;
use crate::domain::agent::resolve_mode as agent_resolve_mode;
use crate::domain::project::Project;
use crate::domain::slash::commands;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use crate::web::os::editor;
use crate::web::routes::folder_routes::store_dir;
use crate::web::routes::session_routes::session_json;
use crate::{folders, git, skills};
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;

pub(crate) fn list_projects(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    let store = app.store()?;
    let favorites = store.favorite_projects()?;
    let recent = store.recent_project_ids()?;
    let mut out = Vec::new();
    for p in store.projects()? {
        // The log says a workflow ended; git says whether its work still waits.
        let pending = git::sessions_with_unintegrated_work(Path::new(&p.workdir));
        // Each session's agent, so the project page can tell an analysis apart.
        let list = agents::discover(Path::new(&p.workdir));
        let sessions: Vec<Value> = store
            .sessions(Some(&p.id))?
            .iter()
            .map(|s| {
                let mut v = session_json(app, s);
                v["awaitingIntegration"] = json!(s.awaiting_integration && pending.contains(&s.id));
                let mode = store.last_mode(&s.id).ok().flatten().and_then(|m| {
                    agent_resolve_mode(&list, Some(m.as_str())).map(|a| a.id.clone())
                });
                v["mode"] = json!(mode);
                v
            })
            .collect();
        let mut v = project_json(&p, store_dir(app));
        v["favorite"] = json!(favorites.contains(&p.id));
        v["recentRank"] = json!(recent.iter().position(|id| *id == p.id));
        v["branch"] = json!(git::current_branch(Path::new(&p.workdir)));
        v["remote"] = json!(
            git::remote_url(Path::new(&p.workdir))
                .as_deref()
                .and_then(git::web_url)
        );
        v["editorUrl"] = json!(editor::vscode_link(
            &p.workdir,
            std::env::var("WSL_DISTRO_NAME").ok().as_deref(),
        ));
        v["sessions"] = Value::Array(sessions);
        out.push(v);
    }
    send_json(stream, 200, &Value::Array(out))
}

pub(crate) fn set_favorite(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    let store = app.store()?;
    let body = req.json()?;
    let favorite = match body["favorite"].as_bool() {
        Some(favorite) => favorite,
        None => return error(stream, 400, "favorite must be a bool"),
    };
    if !store.set_favorite(id, favorite)? {
        return error(stream, 404, "no such project");
    }
    send_json(stream, 200, &json!({ "ok": true }))
}

pub(crate) fn open_project(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
) -> Result<(), HttpError> {
    let body = req.json()?;
    let dir = Path::new(body["workdir"].as_str().unwrap_or(""));
    let Some(dir) = dir.canonicalize().ok().filter(|d| d.is_dir()) else {
        return error(stream, 400, "not a directory");
    };
    if folders::inside_store(&dir, store_dir(app)) {
        return error(
            stream,
            400,
            "fennec's own store cannot be opened as a project",
        );
    }
    let p = app.store()?.project_for(&dir.display().to_string())?;
    send_json(stream, 200, &project_json(&p, store_dir(app)))
}

pub(crate) fn list_commands(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let list = commands(&skills::discover(Path::new(&project.workdir)));
    let out: Vec<Value> = list
        .into_iter()
        .map(|command| json!({ "name": command.name, "description": command.description, "kind": command.kind.wire() }))
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

pub(crate) fn list_agents(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let Some(project) = store.project(id)? else {
        return error(stream, 404, "no such project");
    };
    let mut list = agents::discover(Path::new(&project.workdir));
    // The menu follows a project's order, so phase comes first.
    list.sort_by_key(|agent| (agent.phase.is_none(), agent.phase));
    let out: Vec<Value> = list
        .iter()
        .map(|agent| {
            json!({
                "id": agent.id,
                "name": agent.name,
                "description": agent.description,
                "color": agent.color,
                "subagent": agent.subagent,
                "composer": agent.composer && !agent.subagent,
                "phase": agent.phase,
                "workflow": agent.workflow,
                "origin": agent.origin,
            })
        })
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

pub(crate) fn project_json(p: &Project, store_dir: &Path) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "workdir": p.workdir,
        "chat": chat::is_chat(p),
        "sessionWorktree": folders::inside_store(Path::new(&p.workdir), store_dir),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::EventLog;
    use crate::domain::event::Event;
    use crate::domain::id::new_id;
    use crate::web::test_support::app;
    use std::io::Read;
    use std::net::TcpListener;

    #[test]
    fn project_json_marks_only_the_chat() {
        let chat = Project {
            id: chat::ID.into(),
            name: chat::NAME.into(),
            workdir: "/scratch".into(),
        };
        let other = Project {
            id: "other".into(),
            name: "o".into(),
            workdir: "/w".into(),
        };
        let store_dir = Path::new("/nowhere");
        let chat_json = project_json(&chat, store_dir);
        let other_json = project_json(&other, store_dir);
        assert_eq!(chat_json["chat"], true);
        assert_eq!(other_json["chat"], false);
        assert_eq!(chat_json["sessionWorktree"], false);
    }

    #[test]
    fn list_projects_includes_session_mode() {
        let db_path = std::env::temp_dir().join(format!("fennec-routes-test-{}.db", new_id()));
        let app = app(db_path.clone());
        let store = app.store().unwrap();
        let project = store.project_for("/tmp/analyze-mode").unwrap();
        let session = store.create_session(&project, "analyze").unwrap();
        store
            .append(
                &session,
                &Event::ModeChanged {
                    mode: "analyze".into(),
                    auto: false,
                    reason: None,
                },
            )
            .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let mut server = listener.accept().unwrap().0;
        {
            let mut client = client;
            list_projects(&app, &mut client).unwrap();
        }
        let mut body = String::new();
        server.read_to_string(&mut body).unwrap();
        let json = body
            .rsplit("\r\n\r\n")
            .next()
            .expect("a response body after the headers");
        let projects: Value = serde_json::from_str(json).unwrap();
        let mode = projects[0]["sessions"]
            .as_array()
            .and_then(|s| s.iter().find(|s| s["id"] == session))
            .and_then(|s| s["mode"].as_str());
        assert_eq!(mode, Some("analyze"));
        let _ = std::fs::remove_file(&db_path);
    }
}
