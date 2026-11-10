//! Opens the human's own terminal in a session's folder (the worktree when it
//! has one, the project otherwise) or a project's folder. The human's action
//! (trust zone 1), so it runs outside the sandbox; the path comes from the
//! store, never the request.

use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{error, require_project, send_json};
use serde_json::json;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;

/// `POST /api/sessions/{id}/terminal`.
pub(crate) fn open(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let (session, project) = app.require_session(id)?;
    let place = app.place(&store, &session, &project)?;
    match spawn(&place.workdir) {
        Ok(()) => send_json(stream, 202, &json!({ "path": place.workdir })),
        Err(e) => error(stream, 500, &format!("could not open a terminal: {e}")),
    }
}

/// `POST /api/projects/{id}/terminal`.
pub(crate) fn open_project(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let project = require_project(&store, id)?;
    match spawn(Path::new(&project.workdir)) {
        Ok(()) => send_json(stream, 202, &json!({ "path": project.workdir })),
        Err(e) => error(stream, 500, &format!("could not open a terminal: {e}")),
    }
}

fn spawn(dir: &Path) -> std::io::Result<()> {
    let mut child = terminal_command(dir).spawn()?;
    // Reaped in the background so a closed terminal leaves no zombie.
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Windows Terminal under WSL, Terminal.app on macOS, else `$TERMINAL` or
/// the Debian alternative, started in `dir`.
fn terminal_command(dir: &Path) -> Command {
    if let Some(distro) = std::env::var("WSL_DISTRO_NAME")
        .ok()
        .filter(|d| !d.is_empty())
    {
        let mut cmd = Command::new("wt.exe");
        cmd.args(["wsl.exe", "-d", &distro, "--cd"]).arg(dir);
        return cmd;
    }
    if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.args(["-a", "Terminal"]).arg(dir);
        return cmd;
    }
    let program = std::env::var("TERMINAL").unwrap_or_else(|_| "x-terminal-emulator".into());
    let mut cmd = Command::new(program);
    cmd.current_dir(dir);
    cmd
}
