//! The home page's cross-project summary: running count, recent sessions, and favorite projects.

use crate::chat;
use crate::domain::home::recent_sessions;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::send_json;
use crate::web::routes::session_routes::session_json;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::TcpStream;

pub(crate) fn serve(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    let store = app.store()?;
    let projects = store.projects()?;
    let favorites = store.favorite_projects()?;
    let sessions = store.sessions(None)?;

    let mut project_names: HashMap<String, String> = HashMap::new();
    for p in &projects {
        project_names.insert(p.id.clone(), p.name.clone());
    }

    // One pass gives the global running count and the per-project counts the favorites need.
    let mut running = 0usize;
    let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
    for s in &sessions {
        let is_running = app.turns.is_running(&s.id);
        if is_running {
            running += 1;
        }
        let entry = counts.entry(s.project_id.clone()).or_insert((0, 0));
        entry.0 += 1;
        if is_running {
            entry.1 += 1;
        }
    }

    let recent = recent_sessions(sessions, 8)
        .into_iter()
        .map(|s| {
            let mut v = session_json(app, &s);
            v["projectId"] = json!(s.project_id);
            v["projectName"] = json!(
                project_names
                    .get(&s.project_id)
                    .map(String::as_str)
                    .unwrap_or("")
            );
            v["chat"] = json!(s.project_id == chat::ID);
            v
        })
        .collect::<Vec<Value>>();

    let mut favorite_projects = Vec::new();
    for p in projects.iter() {
        if p.id == chat::ID || !favorites.contains(&p.id) {
            continue;
        }
        let (session_count, running_count) = counts.get(&p.id).copied().unwrap_or((0, 0));
        favorite_projects.push(json!({
            "id": p.id,
            "name": p.name,
            "workdir": p.workdir,
            "sessions": session_count,
            "running": running_count,
        }));
    }

    send_json(
        stream,
        200,
        &json!({
            "running": running,
            "recent": Value::Array(recent),
            "favorites": Value::Array(favorite_projects),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;
    use crate::web::test_support::app;
    use std::io::Read;
    use std::net::TcpListener;

    #[test]
    fn home_serve_lists_recent_and_favorites() {
        let db_path = std::env::temp_dir().join(format!("fennec-routes-test-{}.db", new_id()));
        let app = app(db_path.clone());
        let store = app.store().unwrap();
        let project_a = store
            .project_for(&format!("/tmp/home-a-{}", new_id()))
            .unwrap();
        let project_b = store
            .project_for(&format!("/tmp/home-b-{}", new_id()))
            .unwrap();
        let session_a = store.create_session(&project_a, "a").unwrap();
        let session_b = store.create_session(&project_b, "b").unwrap();
        store.set_favorite(&project_a.id, true).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let mut server = listener.accept().unwrap().0;
        {
            let mut client = client;
            serve(&app, &mut client).unwrap();
        }
        let mut body = String::new();
        server.read_to_string(&mut body).unwrap();
        let json = body
            .rsplit("\r\n\r\n")
            .next()
            .expect("a response body after the headers");
        let home: Value = serde_json::from_str(json).unwrap();

        let recent = home["recent"].as_array().cloned().unwrap();
        assert_eq!(recent.len(), 2);
        let ids: Vec<String> = recent
            .iter()
            .filter_map(|s| s["id"].as_str().map(String::from))
            .collect();
        assert!(ids.contains(&session_a));
        assert!(ids.contains(&session_b));

        let favorites = home["favorites"].as_array().cloned().unwrap();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0]["id"], json!(project_a.id));
        assert_eq!(favorites[0]["sessions"], json!(1));

        let _ = std::fs::remove_file(&db_path);
    }
}
