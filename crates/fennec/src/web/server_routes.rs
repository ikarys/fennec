//! The server's own settings over HTTP: its port, and a restart to apply it.

use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde::Serialize;

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::server_config as domain;
use crate::server_config;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerView {
    port: u16,
    running_port: u16,
    from_flag: bool,
    path: String,
}

pub fn show(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    match view(app) {
        Ok(view) => send_json(stream, 200, &serde_json::json!(view)),
        Err(message) => error(stream, 500, &message),
    }
}

pub fn set(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let value = match req.json() {
        Ok(value) => value,
        Err(message) => return error(stream, 400, &message),
    };

    let port = match value.get("port").and_then(serde_json::Value::as_u64) {
        Some(port) => match domain::valid_port(port) {
            Ok(port) => port,
            Err(message) => return error(stream, 400, &message),
        },
        _ => return error(stream, 400, "port must be a number"),
    };

    if let Err(message) = server_config::set_port(port) {
        return error(stream, 500, &message);
    }

    match view(app) {
        Ok(view) => send_json(stream, 200, &serde_json::json!(view)),
        Err(message) => error(stream, 500, &message),
    }
}

pub fn restart(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    if !app.turns.running_ids().is_empty() {
        return error(stream, 409, "a turn is running: restart when it ends");
    }

    let binary = match std::env::current_exe() {
        Ok(binary) => binary,
        Err(err) => return error(stream, 500, &format!("current executable: {err}")),
    };
    let binary = strip_deleted_suffix(binary);

    send_json(stream, 202, &serde_json::json!({ "restarting": true }))?;

    thread::spawn(move || {
        // Give the response a moment to leave before the process is replaced.
        thread::sleep(Duration::from_millis(300));
        let message = crate::update::restart(&binary);
        eprintln!("fennec: restart failed: {message}");
    });

    Ok(())
}

fn view(app: &App) -> Result<ServerView, String> {
    let port = server_config::port()?;
    Ok(ServerView {
        port,
        running_port: app.port,
        from_flag: has_port_flag(),
        path: server_config::path().to_string_lossy().into_owned(),
    })
}

fn has_port_flag() -> bool {
    std::env::args().any(|arg| arg == "--port" || arg.starts_with("--port="))
}

fn strip_deleted_suffix(mut path: PathBuf) -> PathBuf {
    if let Some(stripped) = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(" (deleted)"))
        .map(str::to_owned)
    {
        path.set_file_name(stripped);
    }
    path
}
