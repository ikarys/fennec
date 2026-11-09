//! The background service on this machine: its file written, its manager
//! (systemd on Linux, launchd on macOS) called. A file already there is the
//! human's: it is never overwritten.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::domain::service::{
    Control, LAUNCHD_LABEL, SYSTEMD_UNIT, launchctl_args, launchd_plist, mise_latest,
    systemctl_args, systemd_unit,
};
use crate::folders::home;

const NO_SERVICE_FILE: &str = "the service is not installed: run `fennec service install`";

/// Writes the service file if there is none, then starts the service at once and at each login.
/// Returns where the logs are.
pub fn install() -> Result<String, String> {
    let exe = own_binary()?;
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    if cfg!(target_os = "macos") {
        let log = home().join("Library/Logs/fennec.log");
        let agent = launchd_agent();
        let plist = launchd_plist(&exe, &path, &log.display().to_string())?;
        create_dir_for(&log)?;
        write_unless_present(&agent, &plist)?;
        launchctl_control(Control::Start)?;
        Ok(format!("tail -f {}", log.display()))
    } else {
        require_systemd()?;
        write_unless_present(&systemd_unit_file(), &systemd_unit(&exe, &path)?)?;
        run("systemctl", &["--user", "daemon-reload"])?;
        run("systemctl", &["--user", "enable", "--now", SYSTEMD_UNIT])?;
        Ok(format!("journalctl --user -u {SYSTEMD_UNIT} -f"))
    }
}

pub fn control(control: Control) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        if !launchd_agent().exists() {
            return Err(NO_SERVICE_FILE.into());
        }
        launchctl_control(control)
    } else {
        require_systemd()?;
        if !systemd_unit_file().exists() {
            return Err(NO_SERVICE_FILE.into());
        }
        let args = systemctl_args(control);
        run(
            "systemctl",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        )
    }
}

fn launchctl_control(control: Control) -> Result<(), String> {
    let uid = current_uid()?;
    let target = format!("gui/{uid}/{LAUNCHD_LABEL}");
    let loaded = Command::new("launchctl")
        .args(["print", &target])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("could not run launchctl: {e}"))?
        .success();
    let agent = launchd_agent().display().to_string();
    match launchctl_args(control, loaded, &uid, &agent) {
        Some(args) => run(
            "launchctl",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        ),
        None => Ok(()),
    }
}

fn require_systemd() -> Result<(), String> {
    let session = Command::new("systemctl")
        .args(["--user", "show-environment"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match session {
        Ok(status) if status.success() => Ok(()),
        _ => Err("no systemd user session: run `fennec serve` yourself".into()),
    }
}

fn own_binary() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("could not find this binary: {e}"))?;
    let exe = exe
        .canonicalize()
        .map_err(|e| format!("could not resolve {}: {e}", exe.display()))?;
    let exe_text = exe
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{} is not valid UTF-8", exe.display()))?;
    // A version pinned below the latest, or an `installs` folder that is not mise's, stays as is.
    let same_binary = |link: &String| Path::new(link).canonicalize().is_ok_and(|p| p == exe);
    Ok(mise_latest(&exe_text)
        .filter(same_binary)
        .unwrap_or(exe_text))
}

fn systemd_unit_file() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| home().join(".config"), PathBuf::from);
    config.join("systemd/user/fennec.service")
}

fn launchd_agent() -> PathBuf {
    home().join(format!("Library/LaunchAgents/{LAUNCHD_LABEL}.plist"))
}

fn write_unless_present(file: &Path, content: &str) -> Result<(), String> {
    if file.exists() {
        println!("keeping the existing {}", file.display());
        return Ok(());
    }
    create_dir_for(file)?;
    std::fs::write(file, content).map_err(|e| format!("could not write {}: {e}", file.display()))
}

fn create_dir_for(file: &Path) -> Result<(), String> {
    let Some(dir) = file.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))
}

fn current_uid() -> Result<String, String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|e| format!("could not run id: {e}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("could not run {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed: {status}", args.join(" ")))
    }
}
