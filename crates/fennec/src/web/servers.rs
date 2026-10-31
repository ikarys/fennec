//! The project's servers, started and stopped from a session's panel: a
//! background process in the session's workdir, tracked in memory. A human
//! action (trust zone 1), so it runs on the host, not in the sandbox.

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::server::{ServerDef, validate};
use crate::servers;
use serde_json::{Value, json};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;

/// A server's process, started in its own process group so a stop frees the
/// port by killing the whole tree (`setsid` makes the child the group leader).
pub(super) struct RunningServer {
    pub(super) pid: u32,
}

/// `{sessionId}/{name}`: one running server per (session, server).
fn key(session_id: &str, name: &str) -> String {
    format!("{session_id}/{name}")
}

/// The project's servers with their running state in this session.
/// `GET /api/sessions/{id}/servers`.
pub fn list(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some((_, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let defs = servers::read(Path::new(&project.workdir));
    let running = app.server_runs.lock().unwrap();
    let out: Vec<Value> = defs
        .iter()
        .map(|def| {
            json!({
                "name": def.name,
                "command": def.command,
                "running": running.contains_key(&key(id, def.name.trim())),
            })
        })
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

/// Saves the project's servers to `.agents/servers.json`.
/// `POST /api/sessions/{id}/servers` with `{servers: [{name, command}]}`.
pub fn save(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some((_, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let body = req.json()?;
    let Some(items) = body.get("servers").and_then(Value::as_array) else {
        return error(stream, 400, "servers must be an array");
    };
    let defs: Vec<ServerDef> = items
        .iter()
        .map(|item| ServerDef {
            name: item["name"].as_str().unwrap_or("").to_string(),
            command: item["command"].as_str().unwrap_or("").to_string(),
        })
        .collect();
    if let Err(e) = validate(&defs) {
        return error(stream, 400, &e);
    }
    if let Err(e) = servers::write(Path::new(&project.workdir), &defs) {
        return error(stream, 500, &format!("write the servers: {e}"));
    }
    list(app, stream, id)
}

/// Starts a server in the project's directory.
/// `POST /api/sessions/{id}/servers/start` with `{name}`.
pub fn start(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let Some((_, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let name = req_name(req)?;
    let defs = servers::read(Path::new(&project.workdir));
    let Some(def) = defs.into_iter().find(|d| d.name.trim() == name) else {
        return error(stream, 404, "no such server");
    };
    let k = key(id, &name);
    if app.server_runs.lock().unwrap().contains_key(&k) {
        return error(stream, 409, "already running");
    }
    // In the project's directory, not the session's worktree: a worktree is a
    // bare checkout without node_modules/.venv, so a dev server would not start.
    let mut child = spawn(&def.command, Path::new(&project.workdir))
        .map_err(|e| format!("start {name}: {e}"))?;
    let pid = child.id();
    app.server_runs
        .lock()
        .unwrap()
        .insert(k.clone(), RunningServer { pid });
    // Reap in the background; drop the entry when the process exits so a server
    // that crashes on its own stops showing as running.
    let runs = app.server_runs.clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        // Only drop this child's entry: a stop→restart may have stored a new pid
        // under the same key before this one finished dying.
        let mut runs = runs.lock().unwrap();
        if runs.get(&k).map(|s| s.pid) == Some(pid) {
            runs.remove(&k);
        }
    });
    send_json(stream, 202, &json!({ "pid": pid }))
}

/// Stops a server in this session. `POST /api/sessions/{id}/servers/stop` with `{name}`.
pub fn stop(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let name = req_name(req)?;
    let k = key(id, &name);
    let pid = {
        let running = app.server_runs.lock().unwrap();
        match running.get(&k).map(|s| s.pid) {
            Some(pid) => pid,
            None => return error(stream, 404, "not running"),
        }
    };
    stop_process(pid);
    app.server_runs.lock().unwrap().remove(&k);
    send_json(stream, 200, &json!({}))
}

/// The `name` of a start/stop request.
fn req_name(req: &Request) -> Result<String, String> {
    let body = req.json()?;
    let name = body["name"].as_str().unwrap_or("").trim().to_string();
    if name.is_empty() {
        return Err("name must not be empty".into());
    }
    Ok(name)
}

/// `setsid sh -c <command>` in `workdir`: a new process group, pid == pgid.
fn spawn(command: &str, workdir: &Path) -> std::io::Result<std::process::Child> {
    Command::new("setsid")
        .args(["sh", "-c", command])
        .current_dir(workdir)
        .spawn()
}

/// Kills the server's whole process group; ignores an already-gone process.
fn stop_process(pid: u32) {
    let _ = Command::new("kill")
        .args(["-TERM", &format!("-{pid}")])
        .output();
}
