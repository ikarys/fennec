//! The project's ADR files on disk (`docs/adr/`).

use std::fs;
use std::path::Path;

use crate::domain::adr::{self, ADR_DIR, Adr};

/// The project's ADRs, sorted by number and then file name.
pub fn load(workdir: &Path) -> Vec<Adr> {
    let Ok(entries) = fs::read_dir(workdir.join(ADR_DIR)) else {
        return Vec::new();
    };
    let mut adrs: Vec<Adr> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let content = fs::read_to_string(entry.path()).ok()?;
            adr::parse(&file, &content)
        })
        .collect();
    adrs.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.file.cmp(&b.file)));
    adrs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("fennec-adr-{}-{name}", std::process::id()))
    }

    #[test]
    fn load_missing_dir_is_empty() {
        let workdir = temp_dir("load_missing_dir_is_empty");
        assert!(load(&workdir).is_empty());
        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn load_reads_adrs_sorted() {
        let workdir = temp_dir("load_reads_adrs_sorted");
        let dir = workdir.join(ADR_DIR);
        fs::create_dir_all(&dir).unwrap();
        for (file, content) in [
            ("0002-b.md", "# b"),
            ("0001-a.md", "# a"),
            ("README.md", "# not one"),
        ] {
            fs::write(dir.join(file), content).unwrap();
        }

        let numbers: Vec<u32> = load(&workdir).into_iter().map(|adr| adr.number).collect();
        assert_eq!(numbers, vec![1, 2]);

        let _ = std::fs::remove_dir_all(&workdir);
    }
}
