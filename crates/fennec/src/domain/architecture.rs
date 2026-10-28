//! Architecture rules from `.agents/architecture`: the human declares what a
//! path may not contain. Fennec checks only the lines an integration adds,
//! so debt already in the file never blocks the check. Rules match text, not
//! syntax (`use crate::{web}` or a `super::` path slips past them), so the
//! check is a tripwire shown to the human, not a proof.

/// One `<path prefix> forbids <text>` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub prefix: String,
    pub forbidden: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rules {
    rules: Vec<Rule>,
}

impl Rules {
    /// Parse the content of `.agents/architecture`.
    ///
    /// Blank lines and `#` comments are ignored. A line is a rule only when it
    /// contains the literal ` forbids ` separator; everything before it is the
    /// path prefix, everything after it is the forbidden text. Lines that do
    /// not match the shape are skipped so a malformed file never panics.
    pub fn parse(content: &str) -> Rules {
        let mut rules = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((prefix, forbidden)) = line.split_once(" forbids ") {
                let prefix = prefix.trim();
                let forbidden = forbidden.trim();
                if prefix.is_empty() || forbidden.is_empty() {
                    continue;
                }

                rules.push(Rule {
                    prefix: prefix.to_string(),
                    forbidden: forbidden.to_string(),
                });
            }
        }

        Rules { rules }
    }

    /// True when the human wrote no usable rule.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Check the added lines of `path` against every rule.
    ///
    /// A rule applies when `path` starts with its prefix. An added line is
    /// checked after trimming and ignored when it is a comment (`//` or `#`,
    /// but not Rust attributes `#[` and `#![`). Violations are emitted in
    /// added-line order, then rule order, so the output is deterministic for
    /// the same input.
    pub fn check(&self, path: &str, added: &[String]) -> Vec<Violation> {
        let mut violations = Vec::new();

        for line in added {
            let line = line.trim();
            if line.starts_with("//")
                || (line.starts_with('#') && !line.starts_with("#[") && !line.starts_with("#!["))
            {
                continue;
            }

            for rule in &self.rules {
                if path.starts_with(&rule.prefix) && line.contains(&rule.forbidden) {
                    violations.push(Violation {
                        path: path.to_string(),
                        line: line.to_string(),
                        prefix: rule.prefix.clone(),
                        forbidden: rule.forbidden.clone(),
                    });
                }
            }
        }

        violations
    }
}

/// A violation of an architecture rule.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    pub path: String,
    pub line: String,
    pub prefix: String,
    pub forbidden: String,
}

/// Extract the added lines from a unified diff, which may contain several
/// files.
///
/// Lines before the first `@@` of a file are headers; a `diff --git ` line
/// starts a new file's header again. Inside a hunk, a line starting with `+`
/// is an added line with that one `+` removed, so `++i;` yields `+i;`.
pub fn added_lines(diff: &str) -> Vec<String> {
    let mut added = Vec::new();
    let mut in_hunk = false;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            in_hunk = false;
            continue;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if in_hunk && line.starts_with('+') {
            added.push(line[1..].to_string());
        }
    }

    added
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skips_comments_and_garbage() {
        let rules = Rules::parse(
            "# a comment
            blank below

            not a rule
            src/ forbids std::fs
            forbids no_prefix
            no_separator
            src/ forbids
        ",
        );

        assert_eq!(rules.rules.len(), 1);
        assert_eq!(rules.rules[0].prefix, "src/");
        assert_eq!(rules.rules[0].forbidden, "std::fs");
    }

    #[test]
    fn prefix_matching_uses_path_start() {
        let rules = Rules::parse("crates/fennec/src/domain/ forbids std::fs");
        let added = vec![String::from("let _ = std::fs;")];

        let violations = rules.check("crates/fennec/src/domain/architecture.rs", &added);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].prefix, "crates/fennec/src/domain/");

        let outside = rules.check("crates/fennec/src/domain2/architecture.rs", &added);
        assert!(outside.is_empty());
    }

    #[test]
    fn comment_added_lines_are_ignored() {
        let rules = Rules::parse("src/ forbids std::fs");
        let added = vec![
            String::from("// std::fs"),
            String::from("   # std::fs"),
            String::from("std::fs"),
        ];

        let violations = rules.check("src/a.rs", &added);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, "std::fs");
    }

    #[test]
    fn rust_attributes_are_not_skipped_as_comments() {
        let rules = Rules::parse("src/ forbids crate::web");
        let added = vec![String::from("#[allow(unused)] use crate::web::x;")];

        let violations = rules.check("src/a.rs", &added);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, "#[allow(unused)] use crate::web::x;");
    }

    #[test]
    fn added_lines_ignores_headers_context_and_removed_lines() {
        let diff = "+++ b/src/a.rs
@@ -1,2 +1,3 @@
 context
+added one
-removed
+
++kept
";
        assert_eq!(added_lines(diff), vec!["added one", "", "+kept"]);
    }

    #[test]
    fn added_lines_handles_several_files_in_one_diff() {
        let diff = "diff --git a/x b/x
--- a/x
+++ b/x
@@ -1 +1 @@
+one
diff --git a/y b/y
--- a/y
+++ b/y
@@ -1 +1 @@
+two
";
        assert_eq!(added_lines(diff), vec!["one", "two"]);
    }

    #[test]
    fn empty_rules_report_nothing() {
        let rules = Rules::parse(
            "# only a comment
",
        );
        assert!(rules.is_empty());
        assert!(
            rules
                .check("src/a.rs", &[String::from("anything")])
                .is_empty()
        );
    }

    #[test]
    fn several_rules_report_each_match() {
        let rules = Rules::parse(
            "src/ forbids http
src/ forbids std::fs
",
        );
        let added = vec![String::from("use http; let _ = std::fs;")];

        let violations = rules.check("src/a.rs", &added);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].forbidden, "http");
        assert_eq!(violations[1].forbidden, "std::fs");
    }

    #[test]
    fn violations_are_ordered_by_line_then_rule() {
        let rules = Rules::parse(
            "src/ forbids a
src/ forbids b
",
        );
        let added = vec![String::from("b a"), String::from("a")];

        let violations = rules.check("src/a.rs", &added);
        let got: Vec<(String, String)> = violations
            .iter()
            .map(|v| (v.line.clone(), v.forbidden.clone()))
            .collect();

        assert_eq!(
            got,
            vec![
                (String::from("b a"), String::from("a")),
                (String::from("b a"), String::from("b")),
                (String::from("a"), String::from("a")),
            ]
        );
    }
}
