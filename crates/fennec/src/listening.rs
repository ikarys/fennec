//! The TCP ports a process group listens on, read from the OS.

#[cfg(target_os = "linux")]
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

/// The TCP ports processes in `pgid` listen on, sorted and deduplicated.
#[cfg(target_os = "linux")]
pub fn group_ports(pgid: u32) -> Option<Vec<u16>> {
    linux_group_ports(pgid)
}

/// The TCP ports processes in `pgid` listen on, sorted and deduplicated.
#[cfg(target_os = "macos")]
pub fn group_ports(pgid: u32) -> Option<Vec<u16>> {
    macos_group_ports(pgid)
}

/// Other operating systems do not expose a portable process-group listener list here.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn group_ports(_pgid: u32) -> Option<Vec<u16>> {
    None
}

#[cfg(target_os = "linux")]
fn linux_group_ports(pgid: u32) -> Option<Vec<u16>> {
    let processes = crate::procfs::process_table()?;
    let pids: Vec<u32> = processes
        .iter()
        .filter(|process| process.pgrp == pgid)
        .map(|process| process.pid)
        .collect();

    if pids.is_empty() {
        return Some(Vec::new());
    }

    let mut inodes: Vec<u64> = Vec::new();
    for pid in &pids {
        inodes.extend(crate::procfs::socket_inodes(*pid));
    }

    if inodes.is_empty() {
        return Some(Vec::new());
    }

    let inodes: HashSet<u64> = inodes.into_iter().collect();
    let mut ports = Vec::new();

    for file in ["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(text) = std::fs::read_to_string(file) {
            for line in text.lines() {
                if let Some((port, inode)) = crate::procfs::parse_listen_row(line)
                    && inodes.contains(&inode)
                {
                    ports.push(port);
                }
            }
        }
    }

    ports.sort_unstable();
    ports.dedup();
    Some(ports)
}

#[cfg(target_os = "macos")]
fn macos_group_ports(pgid: u32) -> Option<Vec<u16>> {
    let out = Command::new("lsof")
        .args([
            "-nP",
            "-a",
            "-g",
            &pgid.to_string(),
            "-iTCP",
            "-sTCP:LISTEN",
            "-Fn",
        ])
        .stdin(Stdio::null())
        .output()
        .ok()?;

    let mut ports = parse_lsof(&String::from_utf8_lossy(&out.stdout));
    ports.sort_unstable();
    ports.dedup();
    Some(ports)
}

/// `lsof -Fn` lines starting with `n` are `host:port`; the port is after the last `:`.
#[cfg(any(target_os = "macos", test))]
fn parse_lsof(out: &str) -> Vec<u16> {
    out.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix('n')?;
            let port = rest.rsplit_once(':')?.1;
            port.parse::<u16>().ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lsof_reads_the_ports_from_n_lines() {
        let out = "n127.0.0.1:5180\nn*:8080\nn[::1]:3000\np123\ntTCP\n";
        assert_eq!(parse_lsof(out), vec![5180, 8080, 3000]);
    }

    #[test]
    fn parse_lsof_ignores_lines_without_a_port() {
        assert_eq!(parse_lsof("n\nnfoo\nn127.0.0.1:\n"), Vec::<u16>::new());
    }
}
