//! Port of openfox `context/instructions.ts`: the instructions a new
//! session's system prompt carries, from the Settings page and the
//! AGENTS.md files from the filesystem root down to the working directory.
//!
//! Not ported: the LANGUAGE setting (the global instructions can say it),
//! project instructions (a project's AGENTS.md covers them), CLAUDE.md
//! (fennec reads only the tool-neutral AGENTS.md), and the injected-files
//! list openfox shows in its UI.

use std::fs;
use std::path::{Path, PathBuf};

const INSTRUCTION_FILENAME: &str = "AGENTS.md";

/// openfox `findInstructionFiles`: root first, so a file closer to the
/// working directory can override its parents.
fn find_instruction_files(workdir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<&Path> = workdir.ancestors().collect();
    dirs.reverse();
    dirs.into_iter()
        .map(|dir| dir.join(INSTRUCTION_FILENAME))
        .filter(|path| path.is_file())
        .collect()
}

/// openfox `loadInstructions`: each file under its path; an unreadable one
/// is skipped.
fn load_instructions(files: &[PathBuf]) -> String {
    let mut sections = Vec::new();
    for path in files {
        if let Ok(content) = fs::read_to_string(path) {
            sections.push(format!("Instructions from: {}\n{content}", path.display()));
        }
    }
    sections.join("\n")
}

/// openfox `getAllInstructions`: the global instructions, then the files;
/// empty when there are none.
pub fn all_instructions(global: Option<&str>, workdir: &Path) -> String {
    let mut sections = Vec::new();
    if let Some(g) = global
        && !g.trim().is_empty()
    {
        sections.push(format!("## GLOBAL INSTRUCTIONS\n\n{g}"));
    }
    let loaded = load_instructions(&find_instruction_files(workdir));
    if !loaded.is_empty() {
        sections.push(format!("## FILE INSTRUCTIONS\n\n{loaded}"));
    }
    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;

    fn root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fennec-instructions-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn under_root(files: Vec<PathBuf>, root: &Path) -> Vec<PathBuf> {
        files
            .into_iter()
            .filter(|path| path.starts_with(root))
            .collect()
    }

    #[test]
    fn no_agents_md_finds_nothing_under_root() {
        let root = root();
        let found = find_instruction_files(&root);
        assert!(under_root(found, &root).is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn finds_agents_md_in_the_workdir() {
        let root = root();
        std::fs::write(root.join(INSTRUCTION_FILENAME), "root").unwrap();
        let found = find_instruction_files(&root);
        let found = under_root(found, &root);
        assert_eq!(found, vec![root.join(INSTRUCTION_FILENAME)]);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn walks_up_from_the_workdir_with_the_root_first() {
        let root = root();
        let workdir = root.join("a").join("b");
        std::fs::create_dir_all(&workdir).unwrap();
        std::fs::write(root.join(INSTRUCTION_FILENAME), "root").unwrap();
        std::fs::write(workdir.join(INSTRUCTION_FILENAME), "child").unwrap();
        let found = find_instruction_files(&workdir);
        let found = under_root(found, &root);
        assert_eq!(
            found,
            vec![
                root.join(INSTRUCTION_FILENAME),
                workdir.join(INSTRUCTION_FILENAME)
            ]
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_instructions_prefixes_each_file_and_skips_missing() {
        let root = root();
        let file = root.join(INSTRUCTION_FILENAME);
        std::fs::write(&file, "hello").unwrap();
        let missing = root.join("missing.md");
        let loaded = load_instructions(&[file.clone(), missing]);
        assert_eq!(
            loaded,
            format!("Instructions from: {}\nhello", file.display())
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn all_instructions_orders_global_before_files() {
        let root = root();
        std::fs::write(root.join(INSTRUCTION_FILENAME), "file").unwrap();
        let all = all_instructions(Some("G"), &root);
        assert!(all.starts_with(
            "## GLOBAL INSTRUCTIONS\n\nG\n\n## FILE INSTRUCTIONS\n\nInstructions from: "
        ));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_blank_global_and_no_file_add_nothing() {
        let root = root();
        let all = all_instructions(Some("  "), &root);
        assert!(!all.contains("GLOBAL INSTRUCTIONS"));
        std::fs::remove_dir_all(root).ok();
    }
}
