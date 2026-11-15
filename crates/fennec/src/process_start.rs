//! Process identity tokens: a start-time string that tells whether a pid is
//! still the same process, so a stop never kills a recycled pid.

/// The start-time token of the process that holds `pid` now, if it can be read.
#[cfg(target_os = "linux")]
pub fn of(pid: u32) -> Option<String> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    crate::procfs::parse_start_time(&text).map(|start| start.to_string())
}

/// The start-time token of the process that holds `pid` now, if it can be read.
#[cfg(target_os = "macos")]
pub fn of(pid: u32) -> Option<String> {
    let out = std::process::Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

/// Other operating systems do not expose a portable process start time here.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn of(_pid: u32) -> Option<String> {
    None
}
