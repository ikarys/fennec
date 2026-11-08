// ponytail: token matching is a guard rail, not a sandbox: eval, variables ($X -rf), aliases and a script written then run all bypass it. The bwrap sandbox stays the hard boundary.
//! Command patterns and their matching against a command line's segments.

use std::fmt;

use crate::shell::{basename, segments};

/// A human-readable pattern parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError(pub String);

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PatternError {}

/// A parsed command pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPattern {
    /// The pattern as the human typed it, trimmed: stored and shown back.
    text: String,
    program: String,
    words: Vec<String>,
    flags: Vec<String>,
}

impl CommandPattern {
    /// Parses a single-segment command pattern.
    pub fn parse(pattern: &str) -> Result<Self, PatternError> {
        let parsed_segments = segments(pattern);
        if parsed_segments.is_empty() {
            return Err(PatternError("empty pattern".to_string()));
        }
        if parsed_segments.len() > 1 {
            return Err(PatternError(
                "pattern contains several segments".to_string(),
            ));
        }

        let tokens = &parsed_segments[0];
        if tokens.is_empty() {
            return Err(PatternError("empty pattern".to_string()));
        }

        let raw_program = &tokens[0];
        if raw_program.is_empty() {
            return Err(PatternError("empty program".to_string()));
        }
        if raw_program.starts_with('-') {
            return Err(PatternError("program must not start with '-'".to_string()));
        }

        let program = basename(raw_program).to_string();
        let mut words = Vec::new();
        let mut flags = Vec::new();

        for token in &tokens[1..] {
            if token.starts_with('-') {
                flags.extend(flags_of(token));
            } else {
                words.push(token.clone());
            }
        }

        flags.sort();
        flags.dedup();

        Ok(Self {
            text: pattern.trim().to_string(),
            program,
            words,
            flags,
        })
    }

    /// Returns true when this pattern matches one simple command segment.
    pub fn matches(&self, segment: &[String]) -> bool {
        if segment.is_empty() {
            return false;
        }
        if basename(&segment[0]) != self.program {
            return false;
        }

        let (segment_flags, segment_words) = segment_flags_and_words(segment);
        if !self.flags.iter().all(|flag| segment_flags.contains(flag)) {
            return false;
        }

        words_are_in_order_subsequence(&segment_words, &self.words)
    }

    /// True when the segment is exactly this command: same program, same words in the same
    /// order, same flags. An Allow rule matches this way, so it never covers added flags.
    pub fn matches_exactly(&self, segment: &[String]) -> bool {
        let Some(program) = segment.first() else {
            return false;
        };
        let (segment_flags, segment_words) = segment_flags_and_words(segment);
        self.program == basename(program)
            && self.flags == segment_flags
            && self.words == segment_words
    }

    /// Returns a human-readable description of how fennec understood the pattern.
    pub fn describe(&self) -> String {
        let program = &self.program;
        let mut parts = vec![format!("program {program}")];

        if !self.words.is_empty() {
            parts.push(format!("words {}", self.words.join(" ")));
        }
        if !self.flags.is_empty() {
            parts.push(format!("flags {}", self.flags.join(" ")));
        }

        parts.join(", ")
    }

    /// The pattern as typed (trimmed).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the program basename.
    pub fn program(&self) -> &str {
        &self.program
    }
}

fn flags_of(token: &str) -> Vec<String> {
    if token == "--" {
        return Vec::new();
    }

    if let Some(rest) = token.strip_prefix("--") {
        let name = rest.split('=').next().unwrap_or("");
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return vec![name.to_string()];
        }
        return Vec::new();
    }

    if let Some(rest) = token.strip_prefix('-')
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return rest.chars().map(|c| c.to_string()).collect();
    }

    Vec::new()
}

fn segment_flags_and_words(segment: &[String]) -> (Vec<String>, Vec<String>) {
    let mut flags = Vec::new();
    let mut words = Vec::new();
    let mut after_double_dash = false;

    for token in segment.iter().skip(1) {
        if after_double_dash {
            words.push(token.clone());
            continue;
        }

        if token == "--" {
            after_double_dash = true;
            continue;
        }

        if token.starts_with('-') {
            flags.extend(flags_of(token));
        } else {
            words.push(token.clone());
        }
    }

    flags.sort();
    flags.dedup();
    (flags, words)
}

fn words_are_in_order_subsequence(haystack: &[String], needle: &[String]) -> bool {
    let mut haystack_iter = haystack.iter();
    needle
        .iter()
        .all(|wanted| haystack_iter.by_ref().any(|actual| actual == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(text: &str) -> CommandPattern {
        CommandPattern::parse(text).unwrap()
    }

    fn matches_any(command: &str, expected: &CommandPattern) -> bool {
        segments(command)
            .iter()
            .any(|segment| expected.matches(segment))
    }

    #[test]
    fn rm_rf_matches_expected_commands() {
        let p = pattern("rm -rf");

        assert!(matches_any("rm -fr x", &p));
        assert!(matches_any("rm -r -f x", &p));
        assert!(!matches_any("rm -Rf x", &p));
        assert!(matches_any("/bin/rm -rf x", &p));
        assert!(matches_any("sudo rm -rf /", &p));
        assert!(matches_any("sudo -u root rm -rf /", &p));
        assert!(matches_any("a && rm -rf b", &p));
        assert!(matches_any("ls | xargs rm -rf", &p));
        assert!(matches_any("bash -c 'rm -rf b'", &p));
        assert!(matches_any("bash -lc \"cd x && rm -rf b\"", &p));
        assert!(matches_any("find . -name x -exec rm -rf {} \\;", &p));
        assert!(matches_any("echo $(rm -rf b)", &p));
        assert!(matches_any("echo \"$(rm -rf b)\"", &p));
        assert!(matches_any("FOO=1 env BAR=2 nohup rm -rf b", &p));
        assert!(matches_any("timeout 5 rm -rf b", &p));
    }

    #[test]
    fn rm_rf_does_not_match_non_matches() {
        let p = pattern("rm -rf");

        assert!(!matches_any("echo \"rm -rf\"", &p));
        assert!(!matches_any("echo 'rm -rf b'", &p));
        assert!(!matches_any("rm -r x", &p));
        assert!(!matches_any("rmdir -rf x", &p));
        assert!(!matches_any("grep rm -rf", &p));
    }

    #[test]
    fn git_push_force_matches() {
        let p = pattern("git push --force");

        assert!(matches_any("git push origin main --force", &p));
        assert!(matches_any("git -C x push --force", &p));
        assert!(!matches_any("git push origin main", &p));
        assert!(!matches_any("git pull --force", &p));
    }

    #[test]
    fn parse_errors() {
        assert!(CommandPattern::parse("").is_err());
        assert!(CommandPattern::parse("-rf").is_err());
        assert!(CommandPattern::parse("a; b").is_err());
    }

    #[test]
    fn describe_git_push_force() {
        let p = pattern("git push --force");
        assert_eq!(p.describe(), "program git, words push, flags force");
    }
}
