//! The terminal a session opens from the "open a terminal" button: the
//! command it runs and the errors that say what to do when it cannot.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use super::{spawn, terminal_command};
use crate::domain::lang::Lang;
use crate::domain::preferences::Preferences;

/// A unique dir under the system temp, created.
static DIR_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let counter = DIR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fennec-terminal-test-{}-{counter}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// WSL_DISTRO_NAME and PATH are process-wide; the tests that read them run
/// one at a time, each with the env it needs, the old values restored.
static ENV: Mutex<()> = Mutex::new(());

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    wsl: Option<String>,
    path: Option<String>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.wsl {
            unsafe { std::env::set_var("WSL_DISTRO_NAME", value) };
        }
        if let Some(value) = &self.path {
            unsafe { std::env::set_var("PATH", value) };
        }
    }
}

/// The env the resolution sees: `wsl` for WSL_DISTRO_NAME, `path` as PATH.
fn env(wsl: Option<&str>, path: &Path) -> EnvGuard {
    let lock = ENV.lock().unwrap();
    let old_wsl = std::env::var("WSL_DISTRO_NAME").ok();
    let old_path = std::env::var("PATH").ok();
    unsafe {
        match wsl {
            Some(value) => std::env::set_var("WSL_DISTRO_NAME", value),
            None => std::env::remove_var("WSL_DISTRO_NAME"),
        }
        std::env::set_var("PATH", path);
    }
    EnvGuard {
        _lock: lock,
        wsl: old_wsl,
        path: old_path,
    }
}

/// A PATH dir holding an executable `name`.
fn path_with(name: &str) -> PathBuf {
    let dir = temp_dir().join("bin");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(name);
    std::fs::write(&file, "#! /bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

/// An empty PATH dir: nothing to find.
fn empty_path() -> PathBuf {
    let dir = temp_dir().join("empty");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn terminal_setting_wins_over_the_lookup() {
    let path = empty_path();
    let _env = env(Some("Ubuntu"), &path);
    let dir = temp_dir();
    let cmd = terminal_command(Some("alacritty --title fennec"), &dir);
    assert_eq!(cmd.get_program().to_str().unwrap(), "alacritty");
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, vec!["--title", "fennec"]);
    assert_eq!(cmd.get_current_dir(), Some(dir.as_path()));
}

#[test]
fn a_blank_terminal_setting_falls_through_to_the_lookup() {
    let path = path_with("x-terminal-emulator");
    let _env = env(None, &path);
    let dir = temp_dir();
    let cmd = terminal_command(Some("   "), &dir);
    assert_eq!(
        Path::new(cmd.get_program()).file_name().unwrap(),
        "x-terminal-emulator"
    );
}

#[test]
fn path_lookup_finds_a_known_program() {
    let path = path_with("x-terminal-emulator");
    let _env = env(None, &path);
    let dir = temp_dir();
    let cmd = terminal_command(None, &dir);
    assert_eq!(
        Path::new(cmd.get_program()).file_name().unwrap(),
        "x-terminal-emulator"
    );
    assert_eq!(cmd.get_current_dir(), Some(dir.as_path()));
}

#[test]
fn no_terminal_clear_error() {
    let path = empty_path();
    let _env = env(None, &path);
    let dir = temp_dir();
    let err = spawn(None, &dir, Lang::En).unwrap_err();
    assert!(err.contains("Settings"), "{err}");
    assert!(err.contains("$TERMINAL"), "{err}");
    assert!(!err.contains("No such file or directory"), "{err}");
}

#[test]
fn a_missing_terminal_setting_clear_error() {
    let path = empty_path();
    let _env = env(None, &path);
    let dir = temp_dir();
    let err = spawn(Some("no-such-terminal-xyz"), &dir, Lang::En).unwrap_err();
    assert!(err.contains("no-such-terminal-xyz"), "{err}");
    assert!(err.contains("Settings"), "{err}");
    assert!(err.contains("$TERMINAL"), "{err}");
    assert!(!err.contains("No such file or directory"), "{err}");
}

#[test]
fn missing_folder_clear_error() {
    let path = empty_path();
    let _env = env(None, &path);
    let missing = temp_dir().join("no-such-folder");
    let err = spawn(Some("no-such-terminal-xyz"), &missing, Lang::En).unwrap_err();
    assert!(err.contains("the folder no longer exists"), "{err}");
    assert!(err.contains(missing.to_str().unwrap()), "{err}");
}

#[test]
fn terminal_preference_defaults_to_none_and_round_trips() {
    let old: Preferences = serde_json::from_str(
        r#"{"display":{"reasoningOpen":true,"toolResultsOpen":false,"fontSize":14}}"#,
    )
    .unwrap();
    assert_eq!(old.terminal, None);

    let p = Preferences {
        terminal: Some("alacritty --title fennec".into()),
        ..Preferences::default()
    };
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("\"terminal\""));
    let back: Preferences = serde_json::from_str(&json).unwrap();
    assert_eq!(back.terminal, Some("alacritty --title fennec".into()));
}

#[test]
fn a_terminal_that_will_not_start_names_the_program() {
    let dir = temp_dir();
    let program = dir.join("fake-term");
    std::fs::write(&program, "#!/bin/sh\n").unwrap();
    // Not executable: a spawn failure that is not a missing program.
    let err = spawn(Some(program.to_str().unwrap()), &dir, Lang::En).unwrap_err();
    assert!(err.contains("could not be started"), "{err}");
    assert!(err.contains("fake-term"), "{err}");
}
