//! The on-disk record of live server runs, so a restarted fennec can adopt
//! the servers it started before.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A live server run as recorded in `runs.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub key: String,
    pub pid: u32,
    pub port: u16,
    pub started: String,
}

/// The directory that stores `runs.json` and the server log files.
pub fn dir(db_path: &Path) -> PathBuf {
    let base = db_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("servers")
}

/// The recorded live runs; a missing or invalid file means no runs.
pub fn load(dir: &Path) -> Vec<RunRecord> {
    let Ok(text) = std::fs::read_to_string(dir.join("runs.json")) else {
        return Vec::new();
    };

    serde_json::from_str(&text).unwrap_or_default()
}

/// Atomically writes the recorded live runs to `runs.json`.
pub fn save(dir: &Path, records: &[RunRecord]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;

    let text = serde_json::to_string(records).map_err(std::io::Error::other)?;
    let tmp = dir.join("runs.json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dir.join("runs.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temporary directory for a test.
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fennec-server-runs-test-{}-{}",
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
    fn save_and_load_round_trip_records() {
        let dir = temp_dir();
        let records = vec![RunRecord {
            key: "s1/web".to_string(),
            pid: 123,
            port: 8080,
            started: "42".to_string(),
        }];

        save(&dir, &records).unwrap();
        assert_eq!(load(&dir), records);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_empty_for_a_missing_file() {
        let dir = temp_dir();
        assert!(load(&dir).is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_empty_for_a_garbage_file() {
        let dir = temp_dir();
        std::fs::write(dir.join("runs.json"), "not json").unwrap();
        assert!(load(&dir).is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
