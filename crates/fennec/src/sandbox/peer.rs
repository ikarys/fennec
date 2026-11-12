//! Recognises loopback TCP clients that are fennec sandboxed commands, so they are not treated
//! as the local human.

use std::net::SocketAddr;

#[cfg(any(target_os = "linux", test))]
use std::collections::{HashMap, HashSet};
#[cfg(any(target_os = "linux", test))]
use std::net::{Ipv4Addr, Ipv6Addr};

/// True when the TCP client socket of `peer -> server` belongs to a sandbox fennec launched.
#[cfg(target_os = "linux")]
pub fn is_sandboxed(peer: SocketAddr, server: SocketAddr) -> bool {
    let Some(processes) = process_table() else {
        return false;
    };

    let my_pid = std::process::id();
    let roots = processes
        .iter()
        .filter(|(_, comm, ppid)| *ppid == my_pid && comm == "bwrap")
        .map(|(pid, _, _)| *pid)
        .collect::<Vec<_>>();

    if roots.is_empty() {
        return false;
    }

    let parents = processes
        .iter()
        .map(|(pid, _, ppid)| (*pid, *ppid))
        .collect::<Vec<_>>();
    let descendants = descendants(&roots, &parents);

    let Some(inode) = socket_inode(peer, server) else {
        return false;
    };

    fd_has_inode(&descendants, inode)
}

/// Non-Linux targets do not use this procfs check; macOS uses Seatbelt instead.
#[cfg(not(target_os = "linux"))]
pub fn is_sandboxed(_: SocketAddr, _: SocketAddr) -> bool {
    false
}

/// One `/proc/<pid>/stat` row: pid, command name, and parent pid.
#[cfg(any(target_os = "linux", test))]
fn parse_stat(text: &str) -> Option<(u32, String, u32)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }

    let pid = text[..open].trim().parse::<u32>().ok()?;
    let comm = text[open + 1..close].to_string();
    let ppid = text[close + 1..]
        .split_whitespace()
        .nth(1)?
        .parse::<u32>()
        .ok()?;

    Some((pid, comm, ppid))
}

/// One `/proc/net/tcp` or `/proc/net/tcp6` row: local address, remote address, socket inode.
#[cfg(any(target_os = "linux", test))]
fn parse_tcp_row(line: &str) -> Option<(SocketAddr, SocketAddr, u64)> {
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

/// All pids in `roots` and every process that descends from them through `parents`.
#[cfg(any(target_os = "linux", test))]
fn descendants(roots: &[u32], parents: &[(u32, u32)]) -> HashSet<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, ppid) in parents {
        children.entry(*ppid).or_default().push(*pid);
    }

    let mut seen: HashSet<u32> = roots.iter().copied().collect();
    let mut stack = roots.to_vec();

    while let Some(pid) = stack.pop() {
        if let Some(kids) = children.get(&pid) {
            for kid in kids {
                if seen.insert(*kid) {
                    stack.push(*kid);
                }
            }
        }
    }

    seen
}

#[cfg(target_os = "linux")]
fn process_table() -> Option<Vec<(u32, String, u32)>> {
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

/// The same endpoint, an IPv4 address and its IPv4-mapped IPv6 form being one.
#[cfg(target_os = "linux")]
fn same_addr(a: SocketAddr, b: SocketAddr) -> bool {
    a.port() == b.port() && a.ip().to_canonical() == b.ip().to_canonical()
}

#[cfg(target_os = "linux")]
fn socket_inode(peer: SocketAddr, server: SocketAddr) -> Option<u64> {
    // The table is generated on each read and costs with the machine's socket count: read only
    // the server's family (an IPv6 listener lists its IPv4-mapped clients in tcp6).
    let file = if server.is_ipv4() {
        "/proc/net/tcp"
    } else {
        "/proc/net/tcp6"
    };
    std::fs::read_to_string(file)
        .ok()?
        .lines()
        .filter_map(parse_tcp_row)
        .find(|(local, remote, inode)| {
            *inode != 0 && same_addr(*local, peer) && same_addr(*remote, server)
        })
        .map(|(_, _, inode)| inode)
}

#[cfg(target_os = "linux")]
fn fd_has_inode(pids: &HashSet<u32>, inode: u64) -> bool {
    let target = format!("socket:[{inode}]");

    for pid in pids {
        let dir = format!("/proc/{pid}/fd");
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(link) = std::fs::read_link(entry.path()) else {
                continue;
            };

            if link.to_string_lossy() == target {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stat_keeps_a_comm_with_spaces_and_parentheses() {
        let text = "42 (my (cool) app) S 7 8 9";
        assert_eq!(parse_stat(text), Some((42, "my (cool) app".to_string(), 7)));
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
    fn descendants_include_grandchildren_and_exclude_unrelated_pids() {
        let parents = vec![(1, 0), (2, 1), (3, 2), (9, 1), (8, 0)];

        let set = descendants(&[1], &parents);

        assert!(set.contains(&1));
        assert!(set.contains(&2));
        assert!(set.contains(&3));
        assert!(set.contains(&9));
        assert!(!set.contains(&8));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn e2e_a_sandboxed_client_is_recognised_and_the_test_process_is_not() {
        fn bwrap_available() -> bool {
            std::process::Command::new("bwrap")
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        }

        if !bwrap_available() {
            return;
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();

        let self_client = std::net::TcpStream::connect(addr).unwrap();
        let (self_stream, _) = listener.accept().unwrap();
        let self_peer = self_stream.peer_addr().unwrap();
        let self_server = self_stream.local_addr().unwrap();
        assert!(!is_sandboxed(self_peer, self_server));
        drop(self_stream);
        drop(self_client);

        let dir = crate::tools::test_support::temp_dir();

        let sandbox = crate::sandbox::Sandbox::On {
            network: true,
            hidden_dirs: vec![],
            hidden_files: vec![],
            writable: vec![],
            read_only: vec![],
            mounts: vec![],
        };

        let script = format!("bash -c 'exec 3<>/dev/tcp/127.0.0.1/{port}; sleep 5'");
        let mut child = sandbox.command(&script, &dir, &dir).spawn().unwrap();

        // A bwrap that cannot start (AppArmor) must fail the test, not hang it.
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20))
                }
                Err(error) => panic!("the sandboxed client never connected: {error}"),
            }
        };
        let peer = stream.peer_addr().unwrap();
        let server = stream.local_addr().unwrap();
        assert!(is_sandboxed(peer, server));

        child.kill().unwrap();
        child.wait().unwrap();
    }
}
