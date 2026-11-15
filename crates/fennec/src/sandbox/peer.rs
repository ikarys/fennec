//! Recognises loopback TCP clients that are fennec sandboxed commands, so they are not treated
//! as the local human.

use std::net::SocketAddr;

#[cfg(any(target_os = "linux", test))]
use std::collections::{HashMap, HashSet};

/// True when the TCP client socket of `peer -> server` belongs to a sandbox fennec launched.
#[cfg(target_os = "linux")]
pub fn is_sandboxed(peer: SocketAddr, server: SocketAddr) -> bool {
    let Some(processes) = crate::procfs::process_table() else {
        return false;
    };

    let my_pid = std::process::id();
    let roots = processes
        .iter()
        .filter(|process| process.ppid == my_pid && process.comm == "bwrap")
        .map(|process| process.pid)
        .collect::<Vec<_>>();

    if roots.is_empty() {
        return false;
    }

    let parents = processes
        .iter()
        .map(|process| (process.pid, process.ppid))
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
        .filter_map(crate::procfs::parse_tcp_row)
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
