//! A server run's output file: the log a restarted fennec tails to show the
//! server's last lines.

use crate::domain::server::strip_ansi;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The bytes read from the end of a log when tailing it.
const TAIL_BYTES: u64 = 64 * 1024;

/// The log file for a run key, sanitized so a hostile key cannot leave `dir`.
pub fn path(dir: &Path, key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = if safe.starts_with('.') {
        format!("_{safe}")
    } else {
        safe
    };
    dir.join(format!("{safe}.log"))
}

/// The last `max_lines` non-empty lines of a log file, ANSI-stripped.
pub fn tail(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(len) = file.seek(SeekFrom::End(0)) else {
        return Vec::new();
    };
    if len == 0 {
        return Vec::new();
    }

    let started_mid = len > TAIL_BYTES;
    let start = len.saturating_sub(TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }

    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&buf).to_string();
    let mut lines = Vec::new();
    for (index, piece) in text.split(['\n', '\r']).enumerate() {
        // A tail that started mid-file begins in the middle of a line.
        if started_mid && index == 0 {
            continue;
        }

        let line = strip_ansi(piece);
        if line.is_empty() {
            continue;
        }

        lines.push(line);
    }

    let skip = lines.len().saturating_sub(max_lines);
    lines.drain(..skip);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temporary directory for a test.
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fennec-server-log-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn path_sanitizes_a_server_key() {
        let dir = Path::new("/tmp");
        assert_eq!(path(dir, "s1/web"), dir.join("s1_web.log"));
    }

    #[test]
    fn path_keeps_a_traversal_key_inside_dir() {
        let dir = Path::new("/tmp");
        let p = path(dir, "../../etc/x");
        assert_eq!(p.parent(), Some(dir));
    }

    #[test]
    fn tail_reads_cr_lines_strips_ansi_and_keeps_the_last_n() {
        let dir = temp_dir();
        let path = dir.join("out.log");
        std::fs::write(
            &path,
            "\x1b[32mready\x1b[0m\rprogress\r\x1b[1merror\x1b[0m\r",
        )
        .unwrap();

        assert_eq!(tail(&path, 2), vec!["progress", "error"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_returns_empty_for_a_missing_file() {
        let dir = temp_dir();
        assert!(tail(&dir.join("nope.log"), 10).is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tail_keeps_only_complete_lines_in_a_large_file() {
        let dir = temp_dir();
        let path = dir.join("big.log");
        let long = "a".repeat(70_000);
        std::fs::write(&path, format!("{long}\ncomplete1\ncomplete2\n")).unwrap();

        assert_eq!(tail(&path, 10), vec!["complete1", "complete2"]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
