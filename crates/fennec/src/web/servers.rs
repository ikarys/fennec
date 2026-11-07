//! The project's servers, started and stopped from a session's panel: a
//! background process in the session's workdir, tracked in memory. Output is
//! kept as the last lines, and the URL is read from that output. A human
//! action (trust zone 1), so it runs on the host, not in the sandbox.

use super::App;
use super::http::{Request, error, send_json};
use crate::domain::server::{ServerDef, announced_url, strip_ansi, url_port, validate};
use crate::servers;
use crate::web::http::HttpError;
use crate::worktree;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The lines kept per run: a build log scrolls far, only its end tells why it stopped.
const MAX_LINES: usize = 200;
/// The lines `list` serves: enough to see a URL or an error, small to poll.
const SHOWN_LINES: usize = 40;

/// A server's process, started in its own process group so a stop frees the
/// port by killing the whole tree (`setsid` makes the child the group leader).
pub(super) struct RunningServer {
    pub(super) pid: u32,
    pub(super) port: u16,
    pub(super) output: Arc<Mutex<Output>>,
}

/// Recent server output and exit state.
pub(super) struct Output {
    lines: VecDeque<String>,
    exit: Option<Option<i32>>,
}

/// `{sessionId}/{name}`: one running server per (session, server).
fn key(session_id: &str, name: &str) -> String {
    format!("{session_id}/{name}")
}

/// The project's servers with their running state in this session.
/// `GET /api/sessions/{id}/servers`.
pub fn list(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let Some((_, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let dir = Path::new(&project.workdir);
    let defs = servers::read(dir);
    // Nothing configured: offer the guessed command, which the human confirms
    // once (saving it) rather than typing it.
    let defs: Vec<(ServerDef, bool)> = if defs.is_empty() {
        servers::guess(dir)
            .map(|def| (def, true))
            .into_iter()
            .collect()
    } else {
        defs.into_iter().map(|def| (def, false)).collect()
    };
    let snapshots: Vec<Option<Snapshot>> = {
        let runs = crate::locks::lock(&app.server_runs);
        defs.iter()
            .map(|(def, _)| runs.get(&key(id, def.name.trim())).map(Snapshot::of))
            .collect()
    };
    // Probing ports happens after the lock is released: a slow connect must not
    // hold every other server request.
    let out: Vec<Value> = defs
        .into_iter()
        .zip(snapshots)
        .map(|((def, suggested), snap)| {
            let (state, url, exit_code, output) = match snap {
                None => ("stopped", None, None, Vec::new()),
                Some(Snapshot {
                    exit: Some(code),
                    lines,
                    ..
                }) => ("exited", None, code, lines),
                Some(Snapshot { port, lines, .. }) => {
                    let url =
                        announced_url(&lines).unwrap_or_else(|| format!("http://127.0.0.1:{port}"));
                    let state = if port_is_ready(&url, port) {
                        "ready"
                    } else {
                        "starting"
                    };
                    (state, Some(url), None, lines)
                }
            };
            json!({
                "name": def.name,
                "command": def.command,
                "suggested": suggested,
                "state": state,
                "running": url.is_some(),
                "url": url,
                "exitCode": exit_code,
                "output": output,
            })
        })
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

/// What `list` reads of a run under the lock.
struct Snapshot {
    port: u16,
    lines: Vec<String>,
    exit: Option<Option<i32>>,
}

impl Snapshot {
    fn of(run: &RunningServer) -> Self {
        let out = crate::locks::lock(&run.output);
        let skip = out.lines.len().saturating_sub(SHOWN_LINES);
        Snapshot {
            port: run.port,
            lines: out.lines.iter().skip(skip).cloned().collect(),
            exit: out.exit,
        }
    }
}

/// Saves the project's servers to `.agents/servers.json`.
/// `POST /api/sessions/{id}/servers` with `{servers: [{name, command}]}`.
pub fn save(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
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

/// Starts a server in the session's place.
/// `POST /api/sessions/{id}/servers/start` with `{name}`.
pub fn start(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let Some((session, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    let name = req_name(req)?;
    let defs = servers::read(Path::new(&project.workdir));
    let Some(def) = defs.into_iter().find(|d| d.name.trim() == name) else {
        return error(stream, 404, "no such server");
    };

    let k = key(id, &name);
    {
        let running = crate::locks::lock(&app.server_runs);
        if let Some(run) = running.get(&k)
            && crate::locks::lock(&run.output).exit.is_none()
        {
            return error(stream, 409, "already running");
        }
    }

    let store = app.store()?;
    let place = app.place(&store, &session, &project)?;

    // In the session's own place, so the server serves the agent's work; the
    // server list is still read from the project's directory (the human's
    // configuration).
    if place.worktree.is_some() {
        if let Err(e) = worktree::link_dependencies(&place.workdir) {
            return error(stream, 500, &format!("link the dependencies: {e}"));
        }
        if let Err(e) = worktree::link_env_files(&place.workdir) {
            return error(stream, 500, &format!("link the .env files: {e}"));
        }
    }

    let port = free_port().map_err(|e| format!("pick a port: {e}"))?;
    let shared_target = crate::worktree::shared_target(
        &crate::worktree::root(&app.settings.db_path),
        &place.workdir,
    );
    let target = shared_target.as_deref();

    let mut child = spawn(&def.command, &place.workdir, port, target)
        .map_err(|e| format!("start {name}: {e}"))?;
    let pid = child.id();

    let output = Arc::new(Mutex::new(Output {
        lines: VecDeque::new(),
        exit: None,
    }));

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");

    let out_stdout = output.clone();
    std::thread::spawn(move || read_pipe(stdout, out_stdout));

    let out_stderr = output.clone();
    std::thread::spawn(move || read_pipe(stderr, out_stderr));

    crate::locks::lock(&app.server_runs).insert(
        k.clone(),
        RunningServer {
            pid,
            port,
            output: output.clone(),
        },
    );

    // Record the exit state without removing the entry, so a crash stays
    // visible with its output.
    let runs = app.server_runs.clone();
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|status| status.code());
        let mut runs = crate::locks::lock(&runs);
        if let Some(run) = runs.get_mut(&k)
            && run.pid == pid
        {
            crate::locks::lock(&run.output).exit = Some(code);
        }
    });

    send_json(stream, 202, &json!({ "pid": pid, "port": port }))
}

/// Stops a server in this session. `POST /api/sessions/{id}/servers/stop` with `{name}`.
pub fn stop(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let name = req_name(req)?;
    let k = key(id, &name);

    let pid = {
        let running = crate::locks::lock(&app.server_runs);
        match running.get(&k) {
            None => return error(stream, 404, "not running"),
            Some(run) => {
                if crate::locks::lock(&run.output).exit.is_some() {
                    drop(running);
                    crate::locks::lock(&app.server_runs).remove(&k);
                    return send_json(stream, 200, &json!({}));
                }
                run.pid
            }
        }
    };

    stop_process(pid);
    crate::locks::lock(&app.server_runs).remove(&k);
    send_json(stream, 200, &json!({}))
}

/// The `name` of a start/stop request.
fn req_name(req: &Request) -> Result<String, HttpError> {
    let body = req.json()?;
    let name = body["name"].as_str().unwrap_or("").trim().to_string();
    if name.is_empty() {
        return Err(HttpError::new(400, "name must not be empty"));
    }
    Ok(name)
}

/// A free port on loopback.
// ponytail: the port is free when picked, another process may take it before the server binds; the server then fails to start and shows as stopped.
fn free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// `setsid sh -c <command>` in `workdir` with `PORT`: a new process group, pid == pgid.
fn spawn(
    command: &str,
    workdir: &Path,
    port: u16,
    target: Option<&Path>,
) -> std::io::Result<std::process::Child> {
    let mut cmd = Command::new("setsid");
    cmd.args(["sh", "-c", command])
        .current_dir(workdir)
        .env("PORT", port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(target) = target {
        cmd.env("CARGO_TARGET_DIR", target);
    }

    cmd.spawn()
}

/// Reads a pipe into the output ring buffer, splitting on newlines and CRs.
fn read_pipe<R: std::io::Read + Send + 'static>(reader: R, output: Arc<Mutex<Output>>) {
    let mut buf = BufReader::new(reader);
    let mut chunk = Vec::new();

    loop {
        let n = match buf.read_until(b'\n', &mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };

        for piece in chunk[..n].split(|&b| b == b'\n' || b == b'\r') {
            if piece.is_empty() {
                continue;
            }

            let line = strip_ansi(&String::from_utf8_lossy(piece));
            if line.is_empty() {
                continue;
            }

            let mut out = crate::locks::lock(&output);
            if out.lines.len() >= MAX_LINES {
                out.lines.pop_front();
            }
            out.lines.push_back(line);
        }

        chunk.clear();
    }
}

/// Checks whether the URL's port answers on loopback within 150 ms.
fn port_is_ready(url: &str, fallback: u16) -> bool {
    let port = url_port(url).unwrap_or(fallback);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

/// Kills the server's whole process group; ignores an already-gone process.
fn stop_process(pid: u32) {
    // `--`: procps' kill otherwise reads `-<pid>` as an option and kills nothing.
    let _ = Command::new("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .output();
}
