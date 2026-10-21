//! A task carries a short title and a description that holds the content of
//! the request. The two are distinct, each is editable on its own, and both
//! are shown in the task's details.
//!
//! The crate is binary-only, so these tests reach the product the way a user
//! does: they start the `fennec` binary against an isolated home and talk to
//! it over its local HTTP API.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Server {
    port: u16,
    home: PathBuf,
    child: Child,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Start `fennec` on a free port, with every home pointed at a throwaway
/// folder so the test never touches the person's real data.
fn start() -> Server {
    let port = free_port();
    let home = std::env::temp_dir().join(format!(
        "fennec-lead-title-desc-{}-{port}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let lead_home = home.join("lead");
    let os_home = home.join("os");
    let cfg = home.join("cfg");
    for d in [&lead_home, &os_home, &cfg] {
        std::fs::create_dir_all(d).unwrap();
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_fennec"))
        .env("FENNEC_LEAD_ADDR", format!("127.0.0.1:{port}"))
        .env("FENNEC_LEAD_HOME", &lead_home)
        .env("FENNEC_OS_HOME", &os_home)
        .env("XDG_CONFIG_HOME", &cfg)
        .env("HOME", &home)
        .env("GIT_AUTHOR_NAME", "Fennec Test")
        .env("GIT_AUTHOR_EMAIL", "fennec@test.local")
        .env("GIT_COMMITTER_NAME", "Fennec Test")
        .env("GIT_COMMITTER_EMAIL", "fennec@test.local")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(_) => break,
            Err(_) => {
                if Instant::now() > deadline {
                    let mut out = String::new();
                    let _ = child.stdout.as_mut().unwrap().read_to_string(&mut out);
                    let mut err = String::new();
                    let _ = child.stderr.as_mut().unwrap().read_to_string(&mut err);
                    panic!("fennec did not start on port {port}\nstdout: {out}\nstderr: {err}");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    Server { port, home, child }
}

/// One request to the running server; the body is JSON (or empty for a GET).
fn http(server: &Server, method: &str, path: &str, body: Option<&Value>) -> Value {
    let host = format!("127.0.0.1:{}", server.port);
    let payload = body.map(|v| v.to_string()).unwrap_or_default();
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf).to_string();
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
    if body.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(body).unwrap()
}

/// Register a fresh empty folder as a project, so a task can belong to it.
fn add_project(server: &Server) -> String {
    let dir = server.home.join("proj");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.canonicalize().unwrap().display().to_string();
    let v = http(
        server,
        "POST",
        "/api/projects",
        Some(&json!({"path": path, "place": "here"})),
    );
    assert!(v.get("error").is_none(), "add project failed: {v}");
    path
}

/// Create a backlog task with a title and a description, and return its id.
fn create_task(server: &Server, project: &str, title: &str, description: &str) -> String {
    let v = http(
        server,
        "POST",
        "/api/requests",
        Some(
            &json!({"project": project, "title": title, "description": description, "later": true}),
        ),
    );
    assert!(v.get("error").is_none(), "create task failed: {v}");
    v["id"].as_str().unwrap().to_string()
}

/// The card the view shows for a task, whatever list it sits in.
fn card(server: &Server, id: &str) -> Value {
    let view = http(server, "GET", "/api/view", None);
    for list in ["todo", "running", "history", "later", "archived"] {
        if let Some(c) = view[list]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["id"] == id))
        {
            return c.clone();
        }
    }
    panic!("task {id} is not in the view: {view}");
}

#[test]
fn a_task_carries_a_distinct_title_and_description() {
    let s = start();
    let project = add_project(&s);
    let id = create_task(
        &s,
        &project,
        "Ajouter un filtre",
        "Le filtre doit trier les tâches par date et permettre de les grouper par projet.",
    );

    let c = card(&s, &id);
    // The two fields are distinct: the short label and the content, not one
    // copy of the other.
    assert_eq!(c["title"], "Ajouter un filtre");
    assert_eq!(
        c["description"],
        "Le filtre doit trier les tâches par date et permettre de les grouper par projet."
    );
    assert_ne!(c["title"], c["description"]);
}

#[test]
fn the_description_holds_the_content_of_the_request() {
    let s = start();
    let project = add_project(&s);
    let content =
        "Étape 1 : lire la demande.\nÉtape 2 : proposer une approche.\nÉtape 3 : écrire le test.";
    let id = create_task(&s, &project, "Préparer la refonte", content);

    let c = card(&s, &id);
    // The title stays the short label; the description keeps the whole
    // content of the request, line for line.
    assert_eq!(c["title"], "Préparer la refonte");
    assert_eq!(c["description"], content);
    assert!(c["description"].as_str().unwrap().len() > c["title"].as_str().unwrap().len());
}

#[test]
fn a_task_title_and_description_are_each_editable() {
    let s = start();
    let project = add_project(&s);
    let id = create_task(&s, &project, "Titre A", "Description A");

    // Edit the title alone: the description is left as it was.
    let v = http(
        &s,
        "POST",
        "/api/rewrite",
        Some(&json!({"id": id, "title": "Titre B", "description": "Description A"})),
    );
    assert!(v.get("error").is_none(), "edit title failed: {v}");
    let c = card(&s, &id);
    assert_eq!(c["title"], "Titre B");
    assert_eq!(c["description"], "Description A");

    // Edit the description alone: the title is left as it was.
    let v = http(
        &s,
        "POST",
        "/api/rewrite",
        Some(&json!({"id": id, "title": "Titre B", "description": "Description B"})),
    );
    assert!(v.get("error").is_none(), "edit description failed: {v}");
    let c = card(&s, &id);
    assert_eq!(c["title"], "Titre B");
    assert_eq!(c["description"], "Description B");
}
