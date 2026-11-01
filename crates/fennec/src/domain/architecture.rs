//! Architecture rules from `.agents/architecture`: three kinds. Line rules
//! (`<path prefix> forbids <text>`) are checked only on the lines an
//! integration adds, so debt already in the file never blocks the check.
//! Module rules (`<A> never imports <B>`) are checked on the whole project
//! map. Sensitive zones (`sensitive <pattern>`) mark paths whose change needs
//! the human's review before integration. Rules match text, not syntax
//! (`use crate::{web}` or a `super::` path slips past line rules), so the
//! check is a tripwire shown to the human, not a proof.

/// One `<path prefix> forbids <text>` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub prefix: String,
    pub forbidden: String,
}

/// One `<A> never imports <B>` rule: a module id and the module id it must
/// not import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRule {
    pub from: String,
    pub to: String,
}

impl ModuleRule {
    /// A module id matches a rule side when it is the side itself or lives
    /// under it (`a/b` under `a`), so a rule on a parent covers its children.
    fn side_matches(id: &str, side: &str) -> bool {
        id == side || id.starts_with(&format!("{side}/"))
    }

    /// Whether the dependency `from -> to` breaks this rule.
    pub fn matches(&self, from: &str, to: &str) -> bool {
        Self::side_matches(from, &self.from) && Self::side_matches(to, &self.to)
    }
}

impl std::fmt::Display for ModuleRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} never imports {}", self.from, self.to)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rules {
    rules: Vec<Rule>,
    module_rules: Vec<ModuleRule>,
    sensitive: Vec<String>,
}

impl Rules {
    /// The module rules, in file order.
    pub fn module_rules(&self) -> &[ModuleRule] {
        &self.module_rules
    }

    /// Whether `path` is inside one of the sensitive zones.
    ///
    /// `path` is project-relative and `/`-separated.
    pub fn is_sensitive(&self, path: &str) -> bool {
        self.sensitive
            .iter()
            .any(|pattern| rules::path::covers(pattern, path))
    }

    /// Parse the content of `.agents/architecture`.
    ///
    /// Blank lines and `#` comments are ignored. A line starting with
    /// `sensitive ` is a sensitive zone pattern; a line containing the literal
    /// ` never imports ` is a module rule; a line containing ` forbids ` is a
    /// line rule. Lines that do not match a shape are skipped so a malformed
    /// file never panics.
    pub fn parse(content: &str) -> Rules {
        let mut rules = Vec::new();
        let mut module_rules = Vec::new();
        let mut sensitive = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(pattern) = line.strip_prefix("sensitive ") {
                if let Some(p) = rules::path::normalize(pattern.trim()) {
                    sensitive.push(p);
                }
                continue;
            }

            if let Some((from, to)) = line.split_once(" never imports ") {
                let from = from.trim();
                let to = to.trim();
                let from = from.strip_suffix('/').unwrap_or(from);
                let to = to.strip_suffix('/').unwrap_or(to);
                if from.is_empty() || to.is_empty() {
                    continue;
                }

                module_rules.push(ModuleRule {
                    from: from.to_string(),
                    to: to.to_string(),
                });
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

        Rules {
            rules,
            module_rules,
            sensitive,
        }
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
    fn empty_rules_report_nothing() {
        let rules = Rules::parse(
            "# only a comment
",
        );
        assert!(rules.rules.is_empty());
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

    #[test]
    fn parse_module_rules_trims_and_strips_one_trailing_slash() {
        let rules = Rules::parse(
            "a/ never imports b/
 a  never imports  b
/ never imports b
a never imports /
a never imports
",
        );

        assert_eq!(
            rules.module_rules(),
            &[
                ModuleRule {
                    from: "a".into(),
                    to: "b".into(),
                },
                ModuleRule {
                    from: "a".into(),
                    to: "b".into(),
                },
            ]
        );
    }

    #[test]
    fn module_rule_lines_are_not_line_rules() {
        let rules = Rules::parse("src/ never imports std::fs");

        assert!(rules.rules.is_empty());
        assert_eq!(rules.module_rules().len(), 1);
    }

    #[test]
    fn module_rule_matches_exact_and_child_modules() {
        let rule = ModuleRule {
            from: "crates/fennec/src/domain".into(),
            to: "crates/fennec/src/web".into(),
        };

        assert!(rule.matches("crates/fennec/src/domain", "crates/fennec/src/web"));
        assert!(rule.matches("crates/fennec/src/domain/a", "crates/fennec/src/web/b"));
        assert!(!rule.matches("crates/fennec/src/domainx", "crates/fennec/src/web"));
        assert!(!rule.matches("crates/fennec/src/domain", "crates/fennec/src/webx"));
    }

    #[test]
    fn parse_reads_sensitive_lines() {
        let rules = Rules::parse("sensitive src/auth/\nsensitive *.sql\nsensitive\n");

        assert_eq!(rules.sensitive.len(), 2);
        assert!(rules.is_sensitive("src/auth/login.rs"));
        assert!(rules.is_sensitive("db/migrate.sql"));
    }

    #[test]
    fn sensitive_matches_dir_and_glob_patterns() {
        let rules = Rules::parse("sensitive src/auth/\nsensitive *.sql\n");

        assert!(rules.is_sensitive("src/auth/login.rs"));
        assert!(rules.is_sensitive("db/migrate.sql"));
        assert!(!rules.is_sensitive("src/web/login.rs"));
        assert!(!rules.is_sensitive("db/migrate.txt"));
    }
}
