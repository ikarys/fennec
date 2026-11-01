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
            .filter_map(|pattern| normalize_pattern(pattern))
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

                if let Some(pattern) = normalize_pattern(line) {
                    patterns.push(pattern);
                }
            }
        }

        Protection { patterns }
    }

    /// Only these patterns, no defaults: a path rule's matcher (same syntax as `.agents/protected`).
    pub fn only(patterns: &[String]) -> Protection {
        let patterns = patterns
            .iter()
            .filter_map(|pattern| normalize_pattern(pattern))
            .collect();
        Protection { patterns }
    }

    /// These patterns on top: the human's blocking path rules, so the file tools refuse
    /// them and the sandbox mounts them read-only like any protected file.
    pub fn with_patterns(mut self, patterns: &[String]) -> Protection {
        self.patterns.extend(
            patterns
                .iter()
                .filter_map(|pattern| normalize_pattern(pattern)),
        );
        self
    }

    /// `relative`: a path relative to the project root, `/`-separated, no
    /// leading `./`.
    pub fn is_protected(&self, relative: &str) -> bool {
        if relative.is_empty() {
            return false;
        }

        let path_segments = split_segments(relative);
        for pattern in &self.patterns {
            // A directory pattern protects everything under it: match the path
            // itself and every ancestor directory.
            for length in 1..=path_segments.len() {
                if pattern_matches(pattern, &path_segments[..length]) {
                    return true;
                }
            }
        }

        false
    }
}

fn normalize_pattern(pattern: &str) -> Option<String> {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern.starts_with('#') || pattern.starts_with('!') {
        return None;
    }

    // A trailing `/` is dropped: a directory pattern protects everything under
    // it anyway. A leading `/` is kept because it anchors the pattern.
    let pattern = pattern.strip_suffix('/').unwrap_or(pattern);
    if pattern.is_empty() {
        None
    } else {
        Some(pattern.to_string())
    }
}

fn split_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn pattern_matches(pattern: &str, path_segments: &[&str]) -> bool {
    let anchored = pattern.starts_with('/') || pattern.contains('/');
    let body = pattern.strip_prefix('/').unwrap_or(pattern);
    let pattern_segments = split_segments(body);
    if pattern_segments.is_empty() {
        return false;
    }

    let full_segments: Vec<&str> = if anchored {
        pattern_segments
    } else {
        let mut full_segments = Vec::with_capacity(pattern_segments.len() + 1);
        full_segments.push("**");
        full_segments.extend(pattern_segments);
        full_segments
    };

    match_segments(&full_segments, path_segments)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }

    if pattern[0] == "**" {
        // `**` as a whole segment matches zero or more path segments.
        if match_segments(&pattern[1..], path) {
            return true;
        }
        if !path.is_empty() && match_segments(pattern, &path[1..]) {
            return true;
        }
        return false;
    }

    if path.is_empty() {
        return false;
    }

    if segment_matches(pattern[0], path[0]) {
        match_segments(&pattern[1..], &path[1..])
    } else {
        false
    }
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    glob_match(&pattern, &text)
}

fn glob_match(pattern: &[char], text: &[char]) -> bool {
    let mut pattern_index = 0;
    let mut text_index = 0;
    let mut star_pattern_index = None;
    let mut star_text_index = 0;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star_pattern_index = Some(pattern_index);
            star_text_index = text_index;
            pattern_index += 1;
        } else if let Some(star) = star_pattern_index {
            pattern_index = star + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
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
