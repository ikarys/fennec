//! The project's architecture rules on disk.

use std::path::Path;

use crate::domain::architecture::Rules;

/// The project's rules: `.agents/architecture` when present, none otherwise.
pub fn load(workdir: &Path) -> Rules {
    let content = std::fs::read_to_string(workdir.join(".agents").join("architecture"));
    Rules::parse(&content.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::drift;
    use crate::domain::module_map::{self, SourceFile};

    /// fennec's own production sources: test modules, `tests.rs` and
    /// `test_support.rs` cut off, since a test may wire an adapter in.
    fn production_sources(root: &Path) -> Vec<SourceFile> {
        crate::module_map::tracked_files(root)
            .into_iter()
            .filter(|file| {
                !file.path.ends_with("tests.rs")
                    && !file.path.ends_with("test_support.rs")
                    && !file.path.contains("/tests/")
            })
            .map(|file| SourceFile {
                content: file
                    .content
                    .split("#[cfg(test)]")
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                path: file.path,
            })
            .collect()
    }

    #[test]
    fn fennec_keeps_its_own_architecture_rules() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let rules = load(&root);
        let sources = production_sources(&root);

        let line_breaks: Vec<String> = sources
            .iter()
            .flat_map(|file| {
                let lines: Vec<String> = file.content.lines().map(str::to_string).collect();
                rules.check(&file.path, &lines)
            })
            .map(|violation| format!("{}: {}", violation.path, violation.line))
            .collect();
        assert!(
            line_breaks.is_empty(),
            "line rules broken:\n{}",
            line_breaks.join("\n")
        );

        let mut map = module_map::build(&sources);
        let module_breaks = drift::measure(&mut map, &rules, None).violations;
        assert!(
            module_breaks.is_empty(),
            "module rules broken: {:?}",
            module_breaks
                .iter()
                .map(|rule_break| format!("{} -> {}", rule_break.from, rule_break.to))
                .collect::<Vec<_>>()
        );
    }
}
