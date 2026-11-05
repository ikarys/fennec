//! The project's ADR files on disk (`docs/adr/`).

use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
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

/// Writes one ADR file, creating `docs/adr` if needed. Refuses to overwrite an
/// existing file; returns the project-relative path.
pub fn write(workdir: &Path, file_name: &str, content: &str) -> std::io::Result<String> {
    let dir = workdir.join(ADR_DIR);
    // A symlink planted by a command would make the human's click write elsewhere.
    for part in [workdir.join("docs"), dir.clone()] {
        if fs::symlink_metadata(&part).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(std::io::Error::other(format!(
                "{} is a symlink: not written",
                part.display()
            )));
        }
    }
    fs::create_dir_all(&dir)?;
    let path = dir.join(file_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(content.as_bytes())?;
    Ok(format!("{ADR_DIR}/{file_name}"))
}

/// The next ADR number: the maximum existing number plus one.
pub fn next_number(workdir: &Path) -> u32 {
    load(workdir)
        .into_iter()
        .map(|adr| adr.number)
        .max()
        .unwrap_or(0)
        + 1
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

    #[test]
    fn write_creates_the_adr_dir_and_refuses_to_overwrite() {
        let workdir = temp_dir("write_creates_the_adr_dir_and_refuses_to_overwrite");
        let relative = write(&workdir, "0001-a.md", "# a").unwrap();
        assert_eq!(relative, format!("{ADR_DIR}/0001-a.md"));
        assert!(write(&workdir, "0001-a.md", "# b").is_err());

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn next_number_is_the_max_existing_number_plus_one() {
        let workdir = temp_dir("next_number_is_the_max_existing_number_plus_one");
        assert_eq!(next_number(&workdir), 1);

        let dir = workdir.join(ADR_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("0002-b.md"), "# b").unwrap();
        fs::write(dir.join("0001-a.md"), "# a").unwrap();
        assert_eq!(next_number(&workdir), 3);

        let _ = std::fs::remove_dir_all(&workdir);
    }
}
