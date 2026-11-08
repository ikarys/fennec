//! Opens the human's own file manager in a session's project folder: the
//! human's repository, not the worktree. The human's action (trust zone 1),
//! so it runs outside the sandbox; the path comes from the store, never the
//! request.

use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{error, send_json};
use serde_json::json;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;

/// `POST /api/sessions/{id}/files`.
pub(crate) fn open(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let (_session, project) = app.require_session(id)?;
    let dir = Path::new(&project.workdir);
    match spawn(dir) {
        Ok(()) => send_json(stream, 202, &json!({ "path": &project.workdir })),
        Err(e) => error(
            stream,
            500,
            &format!("could not open the file manager: {e}"),
        ),
    }
}

fn spawn(dir: &Path) -> std::io::Result<()> {
    let mut child = file_manager_command(dir).spawn()?;
    // Reaped in the background so a closed file manager leaves no zombie.
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Windows Explorer under WSL, Finder via `open` on macOS, else `xdg-open`,
/// started in `dir`.
fn file_manager_command(dir: &Path) -> Command {
    if std::env::var_os("WSL_DISTRO_NAME").is_some_and(|d| !d.is_empty()) {
        // Explorer resolves `.` against the WSL cwd, yielding a \wsl$ path.
        let mut cmd = Command::new("explorer.exe");
        cmd.arg(".").current_dir(dir);
        return cmd;
    }
    if cfg!(target_os = "macos") {
        // `open` asks Finder to display the directory.
        let mut cmd = Command::new("open");
        cmd.arg(dir);
        return cmd;
    }
    // On Linux, `xdg-open` is the desktop-environment-agnostic opener.
    let mut cmd = Command::new("xdg-open");
    cmd.arg(dir);
    cmd
}
