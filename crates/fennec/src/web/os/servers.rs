//! The project's servers, started and stopped from a session's panel: a
//! background process in the session's workdir, tracked in memory. Output goes
//! to a log file, and live runs are recorded so a restarted fennec can adopt
//! them. A human action (trust zone 1), so it runs on the host, not in the sandbox.

use crate::domain::server::{ServerDef, address, announced_url, url_port, validate};
use crate::listening;
use crate::process_start;
use crate::server_log;
use crate::server_runs;
use crate::servers;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{Request, error, send_json};
use crate::worktree;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::File;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The lines kept per run: a build log scrolls far, only its end tells why it stopped.
const MAX_LINES: usize = 200;
/// The lines `list` serves: enough to see a URL or an error, small to poll.
const SHOWN_LINES: usize = 40;

/// A server's process, started in its own process group so a stop frees the
/// port by killing the whole tree (`setsid` makes the child the group leader).
pub(crate) struct RunningServer {
    pub(crate) pid: u32,
    pub(crate) port: u16,
    pub(crate) started: String,
    pub(crate) log: PathBuf,
    pub(crate) adopted: bool,
    pub(crate) exit: Arc<Mutex<Option<Exit>>>,
}

/// A server's exit state.
#[derive(Clone, Copy)]
pub(crate) struct Exit {
    code: Option<i32>,
    signal: Option<i32>,
}

/// `{sessionId}/{name}`: one running server per (session, server).
fn key(session_id: &str, name: &str) -> String {
    format!("{session_id}/{name}")
}

/// The project's servers with their running state in this session.
/// `GET /api/sessions/{id}/servers`.
pub fn list(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let (_, project) = app.require_session(id)?;
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
            let mut row = run_state(snap);
            row["name"] = json!(def.name);
            row["command"] = json!(def.command);
            row["suggested"] = json!(suggested);
            row
        })
        .collect();
    send_json(stream, 200, &Value::Array(out))
}

/// The run-dependent fields of one server row: state, where it listens, how it ended.
fn run_state(snap: Option<Snapshot>) -> Value {
    let Some(snap) = snap else {
        return json!({
            "state": "stopped", "running": false, "url": null, "port": null, "extraUrls": [],
            "portIgnored": false, "exitCode": null, "exitSignal": null, "output": [],
        });
    };
    if let Some(exit) = snap.exit {
        return json!({
            "state": "exited", "running": false, "url": null, "port": null, "extraUrls": [],
            "portIgnored": false, "exitCode": exit.code, "exitSignal": exit.signal,
            "output": snap.lines,
        });
    }
    let listening = listening::group_ports(snap.pid)
        .unwrap_or_else(|| connect_probe(snap.port, snap.announced.as_deref()));
    let addr = address(snap.port, &listening, snap.announced);
    json!({
        "state": if addr.ready { "ready" } else { "starting" },
        "running": true,
        "url": addr.url,
        "port": snap.port,
        "extraUrls": addr.extra_urls,
        "portIgnored": addr.port_ignored,
        "exitCode": null,
        "exitSignal": null,
        "output": snap.lines,
    })
}

/// What `list` reads of a run under the lock.
struct Snapshot {
    pid: u32,
    port: u16,
    lines: Vec<String>,
    announced: Option<String>,
    exit: Option<Exit>,
}

impl Snapshot {
    fn of(run: &RunningServer) -> Self {
        // Reading a 64 KiB tail under the server-runs lock is fine: it is a
        // local file read, not a network probe.
        let all = server_log::tail(&run.log, MAX_LINES);
        let announced = announced_url(&all);
        let lines = all[all.len().saturating_sub(SHOWN_LINES)..].to_vec();

        let exit = {
            let exit = *crate::locks::lock(&run.exit);
            if exit.is_none()
                && run.adopted
                && process_start::of(run.pid).as_deref() != Some(run.started.as_str())
            {
                // An adopted run can end while nobody is waiting for it.
                Some(Exit {
                    code: None,
                    signal: None,
                })
            } else {
                exit
            }
        };

        Snapshot {
            pid: run.pid,
            port: run.port,
            lines,
            announced,
            exit,
        }
    }
}

/// Saves the project's servers to `.agents/servers.json`.
/// `POST /api/sessions/{id}/servers` with `{servers: [{name, command}]}`.
pub fn save(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let (_, project) = app.require_session(id)?;
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
    let (session, project) = app.require_session(id)?;
    let name = req_name(req)?;
    let defs = servers::read(Path::new(&project.workdir));
    let Some(def) = defs.into_iter().find(|d| d.name.trim() == name) else {
        return error(stream, 404, "no such server");
    };

    let k = key(id, &name);
    {
        let running = crate::locks::lock(&app.server_runs);
        if let Some(run) = running.get(&k)
            && crate::locks::lock(&run.exit).is_none()
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

    let dir = server_runs::dir(Path::new(&app.settings.db_path));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return error(
            stream,
            500,
            &format!("create the server run directory: {e}"),
        );
    }
    let log = server_log::path(&dir, &k);

    let mut child = spawn(&def.command, &place.workdir, port, target, &log)
        .map_err(|e| format!("start {name}: {e}"))?;
    let pid = child.id();
    let started = process_start::of(pid).unwrap_or_default();
    let exit = Arc::new(Mutex::new(None));

    {
        let mut runs = crate::locks::lock(&app.server_runs);
        runs.insert(
            k.clone(),
            RunningServer {
                pid,
                port,
                started,
                log,
                adopted: false,
                exit: exit.clone(),
            },
        );
        persist(&runs, &dir);
    }

    // Record the exit state without removing the entry, so a crash stays
    // visible with its output.
    let runs = app.server_runs.clone();
    std::thread::spawn(move || {
        let status = child.wait().ok();
        let exit = Some(Exit {
            code: status.and_then(|status| status.code()),
            signal: status.and_then(|status| status.signal()),
        });
        let mut runs = crate::locks::lock(&runs);
        if let Some(run) = runs.get_mut(&k)
            && run.pid == pid
        {
            *crate::locks::lock(&run.exit) = exit;
        }
        persist(&runs, &dir);
    });

    send_json(stream, 202, &json!({ "pid": pid, "port": port }))
}

/// Stops a server in this session. `POST /api/sessions/{id}/servers/stop` with `{name}`.
pub fn stop(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let name = req_name(req)?;
    let k = key(id, &name);
    let dir = server_runs::dir(Path::new(&app.settings.db_path));

    let (pid, log, kill) = {
        let running = crate::locks::lock(&app.server_runs);
        match running.get(&k) {
            None => return error(stream, 404, "not running"),
            Some(run) => {
                if crate::locks::lock(&run.exit).is_some() {
                    let log = run.log.clone();
                    drop(running);
                    crate::locks::lock(&app.server_runs).remove(&k);
                    let _ = std::fs::remove_file(&log);
                    persist(&crate::locks::lock(&app.server_runs), &dir);
                    return send_json(stream, 200, &json!({}));
                }

                // An adopted pid may have been recycled: never kill a stranger's group.
                let kill = !run.adopted
                    || process_start::of(run.pid).as_deref() == Some(run.started.as_str());
                (run.pid, run.log.clone(), kill)
            }
        }
    };

    if kill {
        stop_process(pid);
    }
    crate::locks::lock(&app.server_runs).remove(&k);
    let _ = std::fs::remove_file(&log);
    persist(&crate::locks::lock(&app.server_runs), &dir);
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
/// Under systemd the server runs in its own scope, so a fennec restart does not kill it.
fn spawn(
    command: &str,
    workdir: &Path,
    port: u16,
    target: Option<&Path>,
    log: &Path,
) -> std::io::Result<std::process::Child> {
    let file = File::create(log)?;
    let stdout = file.try_clone()?;
    let stderr = file;

    let under_systemd = std::env::var_os("INVOCATION_ID").is_some();
    let program = if under_systemd {
        "systemd-run"
    } else {
        "setsid"
    };
    let args: Vec<&str> = if under_systemd {
        vec![
            "--user",
            "--scope",
            "--quiet",
            "--collect",
            "setsid",
            "sh",
            "-c",
            command,
        ]
    } else {
        vec!["sh", "-c", command]
    };

    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(workdir)
        .env("PORT", port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    if let Some(target) = target {
        cmd.env("CARGO_TARGET_DIR", target);
    }

    cmd.spawn()
}

/// Writes the records for the runs that are still live.
fn persist(runs: &HashMap<String, RunningServer>, dir: &Path) {
    let records: Vec<server_runs::RunRecord> = runs
        .iter()
        .filter(|(_, run)| crate::locks::lock(&run.exit).is_none() && !run.started.is_empty())
        .map(|(key, run)| server_runs::RunRecord {
            key: key.clone(),
            pid: run.pid,
            port: run.port,
            started: run.started.clone(),
        })
        .collect();

    // The panel still works when this fails; only restart survival is lost.
    let _ = server_runs::save(dir, &records);
}

/// The live runs to adopt at startup: records whose process is still the same.
pub(crate) fn adopt(db_path: &Path) -> HashMap<String, RunningServer> {
    let dir = server_runs::dir(db_path);
    let records = server_runs::load(&dir);

    let mut runs = HashMap::new();
    let mut kept = Vec::new();
    for record in records {
        if !record.started.is_empty()
            && process_start::of(record.pid).as_deref() == Some(record.started.as_str())
        {
            runs.insert(
                record.key.clone(),
                RunningServer {
                    pid: record.pid,
                    port: record.port,
                    started: record.started.clone(),
                    log: server_log::path(&dir, &record.key),
                    adopted: true,
                    exit: Arc::new(Mutex::new(None)),
                },
            );
            kept.push(record);
        } else {
            let _ = std::fs::remove_file(server_log::path(&dir, &record.key));
        }
    }

    let _ = server_runs::save(&dir, &kept);
    runs
}

/// Where `group_ports` cannot see the sockets: the one port that accepts a connection.
fn connect_probe(given: u16, announced: Option<&str>) -> Vec<u16> {
    let port = announced.and_then(url_port).unwrap_or(given);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    if TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok() {
        vec![port]
    } else {
        Vec::new()
    }
}

/// Kills the server's whole process group; ignores an already-gone process.
fn stop_process(pid: u32) {
    // `--`: procps' kill otherwise reads `-<pid>` as an option and kills nothing.
    let _ = Command::new("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .output();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temporary directory for a test.
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fennec-servers-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn adopt_keeps_a_live_record_and_drops_a_recycled_pid() {
        let Some(started) = process_start::of(std::process::id()) else {
            return;
        };

        let base = temp_dir();
        let db_path = base.join("db.sqlite");
        let runs_dir = server_runs::dir(&db_path);
        std::fs::create_dir_all(&runs_dir).unwrap();

        let pid = std::process::id();
        let records = vec![
            server_runs::RunRecord {
                key: "s1/live".to_string(),
                pid,
                port: 8080,
                started: started.clone(),
            },
            server_runs::RunRecord {
                key: "s1/dead".to_string(),
                pid,
                port: 8081,
                started: "0".to_string(),
            },
        ];
        server_runs::save(&runs_dir, &records).unwrap();

        let dead_log = server_log::path(&runs_dir, "s1/dead");
        std::fs::write(&dead_log, "x").unwrap();

        let adopted = adopt(&db_path);

        assert!(adopted.contains_key("s1/live"));
        assert!(!adopted.contains_key("s1/dead"));
        assert!(!dead_log.exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
