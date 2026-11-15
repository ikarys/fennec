//! Linux procfs readers shared by the sandbox peer check and the server port probe.

#[cfg(any(target_os = "linux", test))]
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

/// One `/proc/<pid>/stat` row: pid, command name, parent pid, and process group.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, PartialEq, Eq)]
pub struct ProcStat {
    pub pid: u32,
    pub comm: String,
    pub ppid: u32,
    pub pgrp: u32,
}

/// One `/proc/<pid>/stat` row: pid, command name, parent pid, and process group.
#[cfg(any(target_os = "linux", test))]
pub fn parse_stat(text: &str) -> Option<ProcStat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }

    let pid = text[..open].trim().parse::<u32>().ok()?;
    let comm = text[open + 1..close].to_string();
    // After the command name: state, ppid, pgrp.
    let mut fields = text[close + 1..].split_whitespace().skip(1);
    let ppid = fields.next()?.parse::<u32>().ok()?;
    let pgrp = fields.next()?.parse::<u32>().ok()?;

    Some(ProcStat {
        pid,
        comm,
        ppid,
        pgrp,
    })
}

/// The process start time (field 22) of a `/proc/<pid>/stat` row.
#[cfg(any(target_os = "linux", test))]
pub fn parse_start_time(text: &str) -> Option<u64> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }

    // After the command name: state, ppid, pgrp, ...; starttime is the 20th field.
    text[close + 1..]
        .split_whitespace()
        .nth(19)
        .and_then(|field| field.parse::<u64>().ok())
}

/// One `/proc/net/tcp` or `/proc/net/tcp6` row: local address, remote address, socket inode.
#[cfg(any(target_os = "linux", test))]
pub fn parse_tcp_row(line: &str) -> Option<(SocketAddr, SocketAddr, u64)> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    let local = parse_addr(parts.get(1)?)?;
    let remote = parse_addr(parts.get(2)?)?;
    let inode = parts.get(9)?.parse::<u64>().ok()?;

    Some((local, remote, inode))
}

/// A `host:port` address as printed by `/proc/net/tcp` and `/proc/net/tcp6`.
#[cfg(any(target_os = "linux", test))]
fn parse_addr(text: &str) -> Option<SocketAddr> {
    let (host, port) = text.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;

    if host.len() == 8 {
        let value = u32::from_str_radix(host, 16).ok()?;
        let bytes = value.to_le_bytes();
        Some(SocketAddr::new(Ipv4Addr::from(bytes).into(), port))
    } else if host.len() == 32 {
        let mut octets = [0u8; 16];
        for i in 0..4 {
            let word = u32::from_str_radix(&host[i * 8..i * 8 + 8], 16).ok()?;
            octets[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        Some(SocketAddr::new(Ipv6Addr::from(octets).into(), port))
    } else {
        None
    }
}

/// A `/proc/net/tcp` or `/proc/net/tcp6` row in LISTEN state: local port and socket inode.
#[cfg(any(target_os = "linux", test))]
pub fn parse_listen_row(line: &str) -> Option<(u16, u64)> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    let Some(&"0A") = parts.get(3) else {
        return None;
    };

    let local = parse_addr(parts.get(1)?)?;
    let inode = parts.get(9)?.parse::<u64>().ok()?;
    if inode == 0 {
        return None;
    }

    Some((local.port(), inode))
}

/// Every readable process's `/proc/<pid>/stat` row.
#[cfg(target_os = "linux")]
pub fn process_table() -> Option<Vec<ProcStat>> {
    let mut rows = Vec::new();
    let entries = std::fs::read_dir("/proc").ok()?;

    // A process may exit mid-scan: skip it, never give up (that would fail open).
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }

        let Ok(text) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some(row) = parse_stat(&text) {
            rows.push(row);
        }
    }

    Some(rows)
}

/// The inodes of the `socket:[N]` links in `/proc/<pid>/fd`; empty when unreadable.
#[cfg(target_os = "linux")]
pub fn socket_inodes(pid: u32) -> Vec<u64> {
    let dir = format!("/proc/{pid}/fd");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut inodes = Vec::new();
    for entry in entries.flatten() {
        let Ok(link) = std::fs::read_link(entry.path()) else {
            continue;
        };

        let link = link.to_string_lossy();
        if let Some(inode) = link
            .strip_prefix("socket:[")
            .and_then(|rest| rest.strip_suffix(']'))
            .and_then(|digits| digits.parse::<u64>().ok())
        {
            inodes.push(inode);
        }
    }

    inodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stat_keeps_a_comm_with_spaces_and_parentheses() {
        let text = "42 (my (cool) app) S 7 8 9";
        assert_eq!(
            parse_stat(text),
            Some(ProcStat {
                pid: 42,
                comm: "my (cool) app".to_string(),
                ppid: 7,
                pgrp: 8,
            })
        );
    }

    #[test]
    fn parse_start_time_reads_field_22_from_a_stat_line() {
        let text =
            "42 (my (cool) app) S 7 8 9 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 12345 1000 10";
        assert_eq!(parse_start_time(text), Some(12345));
    }

    #[test]
    fn parse_tcp_row_parses_an_ipv4_row() {
        let line = "  1: 0100007F:A1B2 0100007F:1E61 01 00000000:00000000 00:00000000 00000000  1000        0 123456 1 0000000000000000 20 4 30 10 -1";
        let local = "127.0.0.1:41394".parse::<SocketAddr>().unwrap();
        let remote = "127.0.0.1:7777".parse::<SocketAddr>().unwrap();

        assert_eq!(parse_tcp_row(line), Some((local, remote, 123456)));
    }

    #[test]
    fn parse_tcp_row_parses_an_ipv6_loopback_row() {
        let line = "  1: 00000000000000000000000001000000:A1B2 00000000000000000000000001000000:1E61 01 00000000:00000000 00:00000000 00000000  1000        0 654321 1 0000000000000000 20 4 30 10 -1";
        let local = "[::1]:41394".parse::<SocketAddr>().unwrap();
        let remote = "[::1]:7777".parse::<SocketAddr>().unwrap();

        assert_eq!(parse_tcp_row(line), Some((local, remote, 654321)));
    }

    #[test]
    fn parse_listen_row_accepts_an_ipv4_listen_row() {
        let line = "  1: 0100007F:A1B2 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 123456 1 0000000000000000 20 4 30 10 -1";
        assert_eq!(parse_listen_row(line), Some((41394, 123456)));
    }

    #[test]
    fn parse_listen_row_accepts_an_ipv6_listen_row() {
        let line = "  1: 00000000000000000000000001000000:A1B2 00000000000000000000000001000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 654321 1 0000000000000000 20 4 30 10 -1";
        assert_eq!(parse_listen_row(line), Some((41394, 654321)));
    }

    #[test]
    fn parse_listen_row_rejects_an_established_row() {
        let line = "  1: 0100007F:A1B2 0100007F:1E61 01 00000000:00000000 00:00000000 00000000  1000        0 123456 1 0000000000000000 20 4 30 10 -1";
        assert_eq!(parse_listen_row(line), None);
    }

    #[test]
    fn parse_listen_row_rejects_the_header_line() {
        let line = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode";
        assert_eq!(parse_listen_row(line), None);
    }
}
