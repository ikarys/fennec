//! Adapter for fetching and installing Fennec updates.

pub const REPO_URL: &str = "https://github.com/ikarys/fennec";

/// The changelog release-please keeps on main.
pub const CHANGELOG_URL: &str = "https://github.com/ikarys/fennec/blob/main/CHANGELOG.md";

const INSTALL_SCRIPT: &str = include_str!("../../../install.sh");

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::domain::release;

/// The local build's number (`<commits>.<sha>[-dirty]`), set by `just install`;
/// `None` for a release or CI build.
pub const BUILD: Option<&str> = option_env!("FENNEC_BUILD");

#[derive(Clone)]
pub struct Check {
    pub current: &'static str,
    pub latest: String,
    pub newer: bool,
}

pub fn check() -> Result<Check, String> {
    let latest = latest_tag()?;
    let current = env!("CARGO_PKG_VERSION");
    let newer = release::is_newer(current, &latest);

    Ok(Check {
        current,
        latest,
        newer,
    })
}

pub fn latest_tag() -> Result<String, String> {
    let url = format!("{REPO_URL}/releases/latest");

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .into();

    let response = agent
        .head(url)
        .call()
        .map_err(|e| format!("could not reach {REPO_URL}: {e}"))?;

    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok());

    let tag = location
        .and_then(release::tag_from_latest_url)
        .map(str::to_owned);

    tag.ok_or_else(|| format!("no published release found at {REPO_URL}/releases/latest"))
}

pub fn install(tag: &str, keep_service: bool) -> Result<PathBuf, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("could not find current executable: {e}"))?;
    let exe = std::fs::canonicalize(exe)
        .map_err(|e| format!("could not canonicalize current executable: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "current executable has no parent directory".to_string())?
        .to_path_buf();

    // Compute the installed path before running the script: after the swap,
    // /proc/self/exe points to a deleted inode.
    let binary = dir.join("fennec");

    let mut command = std::process::Command::new("sh");
    command
        .arg("-s")
        .env("FENNEC_VERSION", tag)
        .env("FENNEC_INSTALL_DIR", &dir)
        .env("FENNEC_REPO_URL", REPO_URL)
        .stdin(Stdio::piped());

    if keep_service {
        command.env("FENNEC_SERVICE", "keep");
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("could not run install script: {e}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "install script stdin disappeared".to_string())?;

    if let Err(e) = stdin.write_all(INSTALL_SCRIPT.as_bytes()) {
        drop(stdin);
        let _ = child.wait();
        return Err(format!("could not write install script: {e}"));
    }

    // Close stdin before waiting so the script sees EOF.
    drop(stdin);

    let status = child
        .wait()
        .map_err(|e| format!("install script wait failed: {e}"))?;

    if !status.success() {
        return Err(format!("install script failed with status {status:?}"));
    }

    Ok(binary)
}

pub fn restart(binary: &Path) -> String {
    // exec keeps the same PID, so a systemd service keeps tracking this unit.
    // The listener socket is CLOEXEC, so exec closes it and the new process
    // can bind the port again.
    let e = std::process::Command::new(binary)
        .args(std::env::args_os().skip(1))
        .exec();
    format!("could not restart on {}: {e}", binary.display())
}

pub fn release_url(tag: &str) -> String {
    format!("{REPO_URL}/releases/tag/{tag}")
}
