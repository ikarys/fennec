//! Opens the human's own terminal in a session's folder (the worktree when it
//! has one, the project otherwise) or a project's folder. The human's action
//! (trust zone 1), so it runs outside the sandbox; the path comes from the
//! store, never the request.

use crate::domain::lang::Lang;
use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{error, require_project, send_json};
use serde_json::json;
use std::io;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `POST /api/sessions/{id}/terminal`.
pub(crate) fn open(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let (session, project) = app.require_session(id)?;
    let preferences = store.preferences()?;
    let lang = preferences.display.language;
    let place = app.place(&store, &session, &project)?;
    match spawn(preferences.terminal.as_deref(), &place.workdir, lang) {
        Ok(()) => send_json(stream, 202, &json!({ "path": place.workdir })),
        Err(e) => error(stream, 500, &format!("could not open a terminal: {e}")),
    }
}

/// `POST /api/projects/{id}/terminal`.
pub(crate) fn open_project(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    let project = require_project(&store, id)?;
    let preferences = store.preferences()?;
    let lang = preferences.display.language;
    match spawn(
        preferences.terminal.as_deref(),
        Path::new(&project.workdir),
        lang,
    ) {
        Ok(()) => send_json(stream, 202, &json!({ "path": project.workdir })),
        Err(e) => error(stream, 500, &format!("could not open a terminal: {e}")),
    }
}

/// Starts the terminal in `dir`, reaping it in the background so a closed
/// terminal leaves no zombie. A gone folder and a missing terminal are two
/// different errors, neither the raw spawn error.
pub(super) fn spawn(setting: Option<&str>, dir: &Path, lang: Lang) -> Result<(), String> {
    if !dir.exists() {
        return Err(lang
            .pick(
                &format!("the folder no longer exists: {}", dir.display()),
                &format!("le dossier n'existe plus : {}", dir.display()),
            )
            .to_string());
    }
    let mut cmd = terminal_command(setting, dir);
    let program = PathBuf::from(cmd.get_program());
    let mut child = cmd.spawn().map_err(|err| {
        match err.kind() {
            // The setting names a program that is not on PATH, or no emulator was found.
            io::ErrorKind::NotFound => lang.pick(
                "no terminal emulator found: set one on the Settings page (Terminal command) or in $TERMINAL",
                "aucun émulateur de terminal trouvé : définissez-en un sur la page Paramètres (commande de terminal) ou dans $TERMINAL",
            )
            .to_string(),
            // The program exists but will not start (not executable, …).
            _ => lang.pick(
                &format!(
                    "the terminal command could not be started ({}): {err}",
                    program.display()
                ),
                &format!(
                    "la commande de terminal n'a pas pu démarrer ({}) : {err}",
                    program.display()
                ),
            )
            .to_string(),
        }
    })?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// The command that opens a terminal in `dir`: a non-empty `terminal` setting
/// wins (first token the program, the rest its args); without one, the
/// WSL/macOS default, and on Linux the first of the known emulators found on
/// PATH ($TERMINAL first).
pub(super) fn terminal_command(setting: Option<&str>, dir: &Path) -> Command {
    if let Some(line) = setting.map(str::trim).filter(|line| !line.is_empty()) {
        let mut tokens = line.split_whitespace();
        let program = tokens.next().expect("a non-empty line has a first token");
        let mut cmd = Command::new(program);
        cmd.args(tokens);
        cmd.current_dir(dir);
        return cmd;
    }
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
    let program = std::env::var("TERMINAL")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| KNOWN_EMULATORS.iter().find_map(|name| on_path(name)))
        .unwrap_or_else(|| PathBuf::from(KNOWN_EMULATORS[0]));
    let mut cmd = Command::new(program);
    cmd.current_dir(dir);
    cmd
}

/// Terminal emulators fennec looks for on PATH (Linux, after $TERMINAL).
const KNOWN_EMULATORS: &[&str] = &[
    "x-terminal-emulator",
    "sensible-terminal",
    "gnome-terminal",
    "konsole",
    "xfce4-terminal",
    "alacritty",
    "kitty",
    "st",
    "xterm",
];

/// The full path of `name` when it is a file on PATH.
fn on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|entry| entry.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_that_will_not_start_names_the_program() {
        let dir =
            std::env::temp_dir().join(format!("fennec-terminal-eacces-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join("fake-term");
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        // Not executable: a spawn failure that is not a missing program.
        let err = spawn(Some(program.to_str().unwrap()), &dir, Lang::En).unwrap_err();
        assert!(err.contains("could not be started"), "{err}");
        assert!(err.contains("fake-term"), "{err}");
    }
}
