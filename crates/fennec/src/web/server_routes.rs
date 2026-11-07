//! The server's own settings over HTTP: its port, listen address, password and
//! a restart to apply them.

use crate::web::http::HttpError;
use std::net::{IpAddr, Ipv4Addr, TcpStream};
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
    remote: bool,
    running_remote: bool,
    password_set: bool,
    restart_needed: bool,
}

pub fn show(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    match view(app) {
        Ok(view) => send_json(stream, 200, &serde_json::json!(view)),
        Err(message) => error(stream, 500, &message),
    }
}

pub fn set(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let value = req.json()?;

    let port = match value.get("port").and_then(serde_json::Value::as_u64) {
        Some(port) => match domain::valid_port(port) {
            Ok(port) => port,
            Err(message) => return error(stream, 400, &message),
        },
        _ => return error(stream, 400, "port must be a number"),
    };

    let remote = match value.get("remote").and_then(serde_json::Value::as_bool) {
        Some(remote) => remote,
        _ => return error(stream, 400, "remote must be a boolean"),
    };

    let listen = server_config::listen()?;
    if remote && server_config::password()?.is_none() {
        return error(stream, 400, "set a password before allowing remote access");
    }

    if let Err(message) = server_config::set_port(port) {
        return error(stream, 500, &message);
    }

    // A specific address the human wrote stays as long as it agrees with the switch.
    let new_listen = match (remote, listen.is_loopback()) {
        (true, true) => Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
        (false, false) => Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        _ => None,
    };
    if let Some(Err(message)) = new_listen.map(server_config::set_listen) {
        return error(stream, 500, &message);
    }

    match view(app) {
        Ok(view) => send_json(stream, 200, &serde_json::json!(view)),
        Err(message) => error(stream, 500, &message),
    }
}

pub fn set_password(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let value = req.json()?;

    let password = match value.get("password").and_then(serde_json::Value::as_str) {
        Some(password) => password.to_string(),
        None => return error(stream, 400, "password is required"),
    };

    let hash = match server_config::new_hash(&password) {
        Ok(hash) => hash,
        Err(message) => return error(stream, 400, &message),
    };

    if let Err(message) = server_config::set_password(&hash) {
        return error(stream, 500, &message);
    }

    app.logins.set_password(hash);

    match view(app) {
        Ok(view) => send_json(stream, 200, &serde_json::json!(view)),
        Err(message) => error(stream, 500, &message),
    }
}

pub fn restart(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
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
    let listen = server_config::listen()?;
    let password_set = server_config::password()?.is_some();
    let remote = !listen.is_loopback();
    let running_remote = !app.listen.is_loopback();
    let from_flag = has_port_flag();
    let restart_needed = (port != app.port && !from_flag) || remote != running_remote;

    Ok(ServerView {
        port,
        running_port: app.port,
        from_flag,
        path: server_config::path().to_string_lossy().into_owned(),
        remote,
        running_remote,
        password_set,
        restart_needed,
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
