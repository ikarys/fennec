//! The terminal a session opens from the "open a terminal" button: the
//! command it runs and the errors that say what to do when it cannot.

use std::path::{Path, PathBuf};

use super::{TerminalEnv, spawn_in, terminal_command};
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

/// The lookup's environment: `wsl` as WSL_DISTRO_NAME, no $TERMINAL, `path` as PATH.
fn env(wsl: Option<&str>, path: &Path) -> TerminalEnv {
    TerminalEnv {
        wsl_distro: wsl.map(str::to_string),
        terminal: None,
        path: Some(path.as_os_str().to_owned()),
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
    let env = env(Some("Ubuntu"), &path);
    let dir = temp_dir();
    let cmd = terminal_command(Some("alacritty --title fennec"), &dir, &env).unwrap();
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
    let env = env(None, &path);
    let dir = temp_dir();
    let cmd = terminal_command(Some("   "), &dir, &env).unwrap();
    assert_eq!(
        Path::new(cmd.get_program()).file_name().unwrap(),
        "x-terminal-emulator"
    );
}

#[test]
fn path_lookup_finds_a_known_program() {
    let path = path_with("x-terminal-emulator");
    let env = env(None, &path);
    let dir = temp_dir();
    let cmd = terminal_command(None, &dir, &env).unwrap();
    assert_eq!(
        Path::new(cmd.get_program()).file_name().unwrap(),
        "x-terminal-emulator"
    );
    assert_eq!(cmd.get_current_dir(), Some(dir.as_path()));
}

#[test]
fn no_terminal_clear_error() {
    let path = empty_path();
    let env = env(None, &path);
    let dir = temp_dir();
    let err = spawn_in(None, &dir, Lang::En, &env).unwrap_err();
    assert!(err.contains("Settings"), "{err}");
    assert!(err.contains("$TERMINAL"), "{err}");
    assert!(!err.contains("No such file or directory"), "{err}");
}

#[test]
fn a_missing_terminal_setting_clear_error() {
    let path = empty_path();
    let env = env(None, &path);
    let dir = temp_dir();
    let err = spawn_in(Some("no-such-terminal-xyz"), &dir, Lang::En, &env).unwrap_err();
    assert!(err.contains("no-such-terminal-xyz"), "{err}");
    assert!(err.contains("Settings"), "{err}");
    assert!(err.contains("$TERMINAL"), "{err}");
    assert!(!err.contains("No such file or directory"), "{err}");
}

#[test]
fn missing_folder_clear_error() {
    let path = empty_path();
    let env = env(None, &path);
    let missing = temp_dir().join("no-such-folder");
    let err = spawn_in(Some("no-such-terminal-xyz"), &missing, Lang::En, &env).unwrap_err();
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
    let env = env(None, &empty_path());
    // Not executable: a spawn failure that is not a missing program.
    let err = spawn_in(Some(program.to_str().unwrap()), &dir, Lang::En, &env).unwrap_err();
    assert!(err.contains("could not be started"), "{err}");
    assert!(err.contains("fake-term"), "{err}");
}
