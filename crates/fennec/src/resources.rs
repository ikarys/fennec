//! fennec's own process reading: the /proc adapter for `domain::resources`.

use crate::domain::resources::ProcessSample;

/// A reading of fennec's own process, from the kernel's own files.
#[cfg(target_os = "linux")]
pub fn sample() -> Option<ProcessSample> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let cpu_ticks = crate::domain::resources::parse_cpu_ticks(&stat)?;
    let rss_bytes = crate::domain::resources::parse_rss_bytes(&status)?;
    Some(ProcessSample {
        cpu_ticks,
        rss_bytes,
        at_ms: crate::domain::clock::now_ms(),
    })
}

/// A reading of fennec's own process, from the kernel's own files.
#[cfg(not(target_os = "linux"))]
pub fn sample() -> Option<ProcessSample> {
    // ponytail: macOS has no /proc; the figures are absent there until a `proc_pidinfo` reading is added.
    None
}
