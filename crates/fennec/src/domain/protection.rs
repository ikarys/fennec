//! fennec's own protected files (openfox has no protected files): what the
//! human protects from the model, the guardian of fennec's definition, first
//! part. Patterns are gitignore-like.

/// Lockfiles: protected by default in the human's directory, writable in a
/// session worktree so an agent can add a dependency; the integration gate
/// still shows them because it loads the project's protection.
pub const LOCKFILES: [&str; 4] = [
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
];

/// Always protected in the human's own directory, whatever the project lists:
/// fennec's own definitions (agents, skills, workflows, this list), the
/// instructions, the lockfiles (a new dependency is the human's decision; in
/// a session worktree they are writable and the integration gate still shows
/// them), the CI and the release configuration, the git hooks (they run
/// outside the sandbox on the human's next push), and the accepted ADRs
/// (`docs/adr`: an agent proposes a decision, it never rewrites one).
pub const DEFAULT_PATTERNS: [&str; 13] = [
    ".agents",
    "AGENTS.md",
    "CLAUDE.md",
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    ".github",
    ".gitlab-ci.yml",
    "release-please-config.json",
    ".release-please-manifest.json",
    ".githooks",
    "docs/adr",
];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Protection {
    patterns: Vec<String>,
}

impl Protection {
    /// The defaults plus the lines of the project's `.agents/protected`
    /// (`content`; None when absent).
    pub fn new(content: Option<&str>) -> Protection {
        Self::build(&DEFAULT_PATTERNS, content)
    }

    /// The defaults without the lockfiles, plus the lines of the project's
    /// `.agents/protected` (`content`; None when absent). Used in a session
    /// worktree, where a new dependency may change a lockfile; the integration
    /// gate still shows it because it loads the project's protection.
    pub fn for_worktree(content: Option<&str>) -> Protection {
        let defaults: Vec<&str> = DEFAULT_PATTERNS
            .iter()
            .filter(|pattern| !LOCKFILES.contains(pattern))
            .copied()
            .collect();
        Self::build(&defaults, content)
    }

    fn build(defaults: &[&str], content: Option<&str>) -> Protection {
        let mut patterns = defaults
            .iter()
            .filter_map(|pattern| rules::path::normalize(pattern))
            .collect::<Vec<_>>();

        if let Some(content) = content {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                // No negation: the human removes a default by turning protection
                // off, not by negating it. There is no such switch yet, so
                // defaults always apply.
                if line.starts_with('!') {
                    continue;
                }

                if let Some(pattern) = rules::path::normalize(line) {
                    patterns.push(pattern);
                }
            }
        }

        Protection { patterns }
    }

    /// Only these patterns, no defaults.
    #[cfg(test)]
    pub fn only(patterns: &[String]) -> Protection {
        let patterns = patterns
            .iter()
            .filter_map(|pattern| rules::path::normalize(pattern))
            .collect();
        Protection { patterns }
    }

    /// These patterns on top: the human's blocking path rules, so the file tools refuse
    /// them and the sandbox mounts them read-only like any protected file.
    pub fn with_patterns(mut self, patterns: &[String]) -> Protection {
        self.patterns.extend(
            patterns
                .iter()
                .filter_map(|pattern| rules::path::normalize(pattern)),
        );
        self
    }

    /// `relative`: a path relative to the project root, `/`-separated, no
    /// leading `./`.
    pub fn is_protected(&self, relative: &str) -> bool {
        if relative.is_empty() {
            return false;
        }

        self.patterns
            .iter()
            .any(|pattern| rules::path::covers(pattern, relative))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_uses_only_the_given_patterns() {
        let protection = Protection::only(&["secrets/".to_string()]);
        assert!(protection.is_protected("secrets/a.txt"));
        assert!(!protection.is_protected("src/a.rs"));
    }

    #[test]
    fn with_patterns_adds_patterns_to_existing_protection() {
        let protection =
            Protection::only(&["base/".to_string()]).with_patterns(&["extra/".to_string()]);
        assert!(protection.is_protected("base/a.txt"));
        assert!(protection.is_protected("extra/a.txt"));
        assert!(!protection.is_protected("other/a.txt"));
    }

    use super::*;

    #[test]
    fn defaults_protect_fennec_definitions_lockfiles_and_ci() {
        let protection = Protection::new(None);

        for protected in [
            ".agents/protected",
            ".agents/agents/builder.agent.md",
            "web/package-lock.json",
            "Cargo.lock",
            "crates/x/Cargo.lock",
            ".github/workflows/ci.yml",
            ".githooks/pre-push",
            "release-please-config.json",
            "AGENTS.md",
            "sub/AGENTS.md",
        ] {
            assert!(protection.is_protected(protected), "{protected}");
        }

        for not_protected in ["src/main.rs", "Cargo.toml", "agents.rs"] {
            assert!(!protection.is_protected(not_protected), "{not_protected}");
        }
    }

    #[test]
    fn comments_blank_lines_and_negation_are_ignored() {
        let protection = Protection::new(Some("# project protected files\n\n!Cargo.lock\n   \n"));

        assert!(protection.is_protected("Cargo.lock"));
        assert!(!protection.is_protected("anything"));
        // Nothing was added to the defaults.
        assert_eq!(protection, Protection::new(None));
    }

    #[test]
    fn anchored_pattern_matches_from_the_root_only() {
        let protection = Protection::new(Some("/docs/adr\n"));

        assert!(protection.is_protected("docs/adr"));
        assert!(protection.is_protected("docs/adr/0001.md"));
        assert!(!protection.is_protected("x/docs/adr"));
    }

    #[test]
    fn star_matches_within_one_segment_only() {
        let protection = Protection::new(Some("src/*.rs\n"));

        assert!(protection.is_protected("src/a.rs"));
        assert!(!protection.is_protected("src/x/a.rs"));
    }

    #[test]
    fn double_star_matches_zero_or_more_segments() {
        let protection = Protection::new(Some("src/**/*.rs\n"));

        assert!(protection.is_protected("src/a.rs"));
        assert!(protection.is_protected("src/x/a.rs"));
    }

    #[test]
    fn question_mark_matches_one_character() {
        let protection = Protection::new(Some("src/a?.rs\n"));

        assert!(protection.is_protected("src/a1.rs"));
        assert!(!protection.is_protected("src/a12.rs"));
    }

    #[test]
    fn trailing_slash_is_dropped() {
        let protection = Protection::new(Some("docs/\n"));

        assert!(protection.is_protected("docs"));
        assert!(protection.is_protected("docs/adr"));
        assert!(protection.is_protected("sub/docs"));
        assert!(protection.is_protected("sub/docs/adr"));
    }

    #[test]
    fn negation_line_does_not_remove_a_default() {
        let protection = Protection::new(Some("!Cargo.lock\n"));

        assert!(protection.is_protected("Cargo.lock"));
    }

    #[test]
    fn worktree_allows_lockfiles_but_keeps_other_defaults() {
        let protection = Protection::for_worktree(None);

        assert!(!protection.is_protected("Cargo.lock"));
        assert!(!protection.is_protected("web/package-lock.json"));
        assert!(!protection.is_protected("yarn.lock"));
        assert!(!protection.is_protected("pnpm-lock.yaml"));

        for protected in [".agents/x", "AGENTS.md", ".github/workflows/ci.yml"] {
            assert!(protection.is_protected(protected), "{protected}");
        }
    }

    #[test]
    fn worktree_keeps_a_lockfile_listed_by_the_project() {
        let protection = Protection::for_worktree(Some("Cargo.lock\n"));

        assert!(protection.is_protected("Cargo.lock"));
    }
}
