//! The project's servers, stored in `.agents/servers.json` next to the
//! agents, skills and workflows: a list of `{name, command}`.

use std::fs;
use std::path::Path;

use crate::domain::server::ServerDef;

const FILE: &str = "servers.json";

/// The project's servers, empty when `.agents/servers.json` is absent or unreadable.
pub fn read(workdir: &Path) -> Vec<ServerDef> {
    let path = workdir.join(".agents").join(FILE);
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

/// Writes the project's servers, creating `.agents` when it is missing.
pub fn write(workdir: &Path, defs: &[ServerDef]) -> Result<(), String> {
    let dir = workdir.join(".agents");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(defs).map_err(|e| e.to_string())?;
    fs::write(dir.join(FILE), content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fennec-servers-test-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_absent_file_reads_as_empty() {
        let dir = temp_dir();
        assert!(read(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = temp_dir();
        let defs = vec![
            ServerDef {
                name: "web".into(),
                command: "npm run dev".into(),
            },
            ServerDef {
                name: "api".into(),
                command: "cargo run".into(),
            },
        ];
        write(&dir, &defs).unwrap();
        assert_eq!(read(&dir), defs);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_malformed_file_reads_as_empty() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join(".agents")).unwrap();
        fs::write(dir.join(".agents").join(FILE), "{not json").unwrap();
        assert!(read(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
