//! The rule model: what a rule targets, what it does, and where it applies.

use serde::{Deserialize, Serialize};

use crate::command::{CommandPattern, PatternError};
use crate::decide::PathRequest;
use crate::path;

/// Where a rule applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Global,
    Project(String),
    Session(String),
}

impl Scope {
    /// Stable label used in storage and refusal text.
    pub fn label(&self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Project(_) => "project",
            Scope::Session(_) => "session",
        }
    }
}

/// What fennec does when a rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Allow,
    Ask,
    Block,
}

impl Action {
    /// Storage and wire form of the action.
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Allow => "allow",
            Action::Ask => "ask",
            Action::Block => "block",
        }
    }

    /// Parses the storage and wire form of an action.
    pub fn parse(text: &str) -> Option<Action> {
        match text {
            "allow" => Some(Action::Allow),
            "ask" => Some(Action::Ask),
            "block" => Some(Action::Block),
            _ => None,
        }
    }
}

/// The kind of thing a rule targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    Command,
    Tool,
    Path,
}

impl TargetKind {
    /// Storage and wire form of the target kind.
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Command => "command",
            TargetKind::Tool => "tool",
            TargetKind::Path => "path",
        }
    }

    /// Parses the storage and wire form of a target kind.
    pub fn parse(text: &str) -> Option<TargetKind> {
        match text {
            "command" => Some(TargetKind::Command),
            "tool" => Some(TargetKind::Tool),
            "path" => Some(TargetKind::Path),
            _ => None,
        }
    }
}

/// The kind of access a path rule grants or restricts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Access {
    Read,
    Write,
    ReadWrite,
}

impl Access {
    /// Storage and wire form of the access.
    pub fn as_str(self) -> &'static str {
        match self {
            Access::Read => "read",
            Access::Write => "write",
            Access::ReadWrite => "readWrite",
        }
    }

    /// Parses the storage and wire form of an access.
    pub fn parse(text: &str) -> Option<Access> {
        match text {
            "read" => Some(Access::Read),
            "write" => Some(Access::Write),
            "readWrite" => Some(Access::ReadWrite),
            _ => None,
        }
    }

    /// True when this access includes the access a request needs.
    pub fn covers(self, needed: Access) -> bool {
        self == Access::ReadWrite || self == needed
    }
}

/// A path rule pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPattern {
    text: String,
}

/// Where a pattern falls relative to a work folder root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Under {
    /// The pattern covers the whole work folder.
    Root,
    /// The pattern rebased to a root-relative path for the sandbox mounts.
    Pattern(String),
}

impl PathPattern {
    /// Parses a path pattern.
    ///
    /// The text is kept as typed so refusals and proposals can show it, while matching
    /// uses the normalized form. `..` is rejected because it makes the intended scope
    /// ambiguous.
    pub fn parse(pattern: &str) -> Result<PathPattern, PatternError> {
        let text = pattern.trim();
        if text.is_empty() {
            return Err(PatternError("path pattern is empty".to_string()));
        }
        if text.split('/').any(|segment| segment == "..") {
            return Err(PatternError("a path rule may not contain '..'".to_string()));
        }
        if path::normalize(text).is_none() {
            return Err(PatternError("path pattern is not usable".to_string()));
        }
        Ok(PathPattern {
            text: text.to_string(),
        })
    }

    /// Returns the raw pattern text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns true when the canonicalized request lies under this pattern.
    pub fn matches(&self, request: &PathRequest) -> bool {
        let Some(normalized) = path::normalize(&self.text) else {
            return false;
        };

        if self.text.starts_with('/') {
            Self::match_absolute(&normalized, &request.absolute)
        } else if self.text == "~" || self.text.starts_with("~/") {
            let Some(home) = &request.home else {
                return false;
            };
            let expanded = if self.text == "~" {
                home.clone()
            } else {
                format!("{}{}", home, &self.text[1..])
            };
            let Some(expanded_normalized) = path::normalize(&expanded) else {
                return false;
            };
            Self::match_absolute(&expanded_normalized, &request.absolute)
        } else {
            let Some(relative) = &request.relative else {
                return false;
            };
            path::covers(&normalized, relative)
        }
    }

    /// True for a pattern relative to the project (neither `/…` nor `~/…`): the only form
    /// the host's project-relative matchers (protected files) can take.
    pub fn is_relative(&self) -> bool {
        !self.text.starts_with('/') && !self.text.starts_with('~')
    }

    /// Where this pattern falls relative to `root` (an absolute, normalized directory):
    /// the whole root, a root-relative pattern, or nothing.
    pub fn under(&self, root: &str, home: Option<&str>) -> Option<Under> {
        if self.is_relative() {
            return Some(Under::Pattern(self.text.clone()));
        }

        let expanded = if self.text == "~" || self.text.starts_with("~/") {
            let home = home?;
            if self.text == "~" {
                home.to_string()
            } else {
                format!("{}{}", home, &self.text[1..])
            }
        } else {
            self.text.clone()
        };
        let normalized = path::normalize(&expanded)?;

        let root = root.trim_end_matches('/');
        let root = if root.is_empty() { "/" } else { root };
        if !normalized.starts_with('/') {
            return None;
        }

        if (normalized == root || root.starts_with(&format!("{normalized}/")))
            && !normalized.contains(['*', '?', '['])
        {
            return Some(Under::Root);
        }
        if let Some(rest) = normalized.strip_prefix(&format!("{root}/")) {
            return Some(Under::Pattern(format!("/{rest}")));
        }
        // ponytail: a glob above the work folder is not rebased, the file tools still refuse it.
        None
    }

    /// Human-readable description for rules pages and refusals.
    pub fn describe(&self) -> String {
        let prefix = if self.text.starts_with('/') {
            "absolute path"
        } else if self.text == "~" || self.text.starts_with("~/") {
            "path under the home directory"
        } else {
            "path relative to the project"
        };
        format!("{prefix}: {}", self.text)
    }

    fn match_absolute(pattern: &str, absolute: &str) -> bool {
        let pattern_rel = pattern.strip_prefix('/').unwrap_or(pattern);
        let absolute_rel = absolute.strip_prefix('/').unwrap_or(absolute);
        path::covers(&format!("/{}", pattern_rel), absolute_rel)
    }
}

/// A rule target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Command(CommandPattern),
    /// A tool name, e.g. `web_search`.
    Tool(String),
    Path {
        pattern: PathPattern,
        access: Access,
    },
}

impl Target {
    /// Parses a pattern string into the requested target kind.
    pub fn parse(
        kind: TargetKind,
        pattern: &str,
        access: Option<Access>,
    ) -> Result<Target, PatternError> {
        match kind {
            TargetKind::Command => Ok(Target::Command(CommandPattern::parse(pattern)?)),
            TargetKind::Tool => {
                let tool = pattern.trim();
                if tool.is_empty() {
                    return Err(PatternError("tool name is empty".to_string()));
                }
                if !tool
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(PatternError(
                        "tool name may only contain letters, digits, '_' and '-'".to_string(),
                    ));
                }
                Ok(Target::Tool(tool.to_string()))
            }
            TargetKind::Path => {
                let pattern = PathPattern::parse(pattern)?;
                Ok(Target::Path {
                    pattern,
                    access: access.unwrap_or(Access::ReadWrite),
                })
            }
        }
    }

    /// Returns the target kind.
    pub fn kind(&self) -> TargetKind {
        match self {
            Target::Command(_) => TargetKind::Command,
            Target::Tool(_) => TargetKind::Tool,
            Target::Path { .. } => TargetKind::Path,
        }
    }

    /// Returns the raw pattern text for display and refusal messages.
    pub fn pattern(&self) -> String {
        match self {
            Target::Command(command) => command.text().to_string(),
            Target::Tool(tool) => tool.clone(),
            Target::Path { pattern, .. } => pattern.text().to_string(),
        }
    }

    /// Returns the access for path targets, if any.
    pub fn access(&self) -> Option<Access> {
        match self {
            Target::Path { access, .. } => Some(*access),
            _ => None,
        }
    }

    /// Human-readable description for rules pages.
    pub fn describe(&self) -> String {
        match self {
            Target::Command(command) => command.describe(),
            Target::Tool(tool) => format!("tool {tool}"),
            Target::Path { pattern, access } => {
                let access_text = match access {
                    Access::Read => "read",
                    Access::Write => "write",
                    Access::ReadWrite => "read and write",
                };
                format!("{}, {}", pattern.describe(), access_text)
            }
        }
    }
}

/// Who made the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Human,
    Answer {
        session_id: String,
    },
    /// A rule fennec's kernel builds in code: never stored, never deleted, applied with the human's.
    Fennec,
}

/// A single stored rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: i64,
    pub scope: Scope,
    pub target: Target,
    pub action: Action,
    pub note: Option<String>,
    pub origin: Origin,
}

impl Rule {
    /// Returns the refusal text shown to the model.
    pub fn refusal(&self) -> String {
        let pattern = self.target.pattern();
        let mut text = format!(
            "Refused by fennec rule \"{pattern}\" ({})",
            self.scope.label()
        );
        if let Some(note) = &self.note {
            text.push_str(": ");
            text.push_str(note);
        }
        text.push_str(". Do not retry it in another form.");
        text
    }
}

/// A collection of rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rules(Vec<Rule>);

/// No rules at all: what a host without stored rules (or a test) lends.
pub static NO_RULES: Rules = Rules(Vec::new());

impl Rules {
    /// Creates a rule collection.
    pub fn new(rules: Vec<Rule>) -> Rules {
        Rules(rules)
    }

    /// Returns all rules in stored order.
    pub fn all(&self) -> &[Rule] {
        &self.0
    }

    /// Returns true when no rules are stored.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_serde() {
        let action: Action = serde_json::from_str(r#""block""#).unwrap();
        assert_eq!(action, Action::Block);
    }

    #[test]
    fn access_covers() {
        assert!(Access::ReadWrite.covers(Access::Read));
        assert!(Access::ReadWrite.covers(Access::Write));
        assert!(Access::Read.covers(Access::Read));
        assert!(!Access::Read.covers(Access::Write));
        assert!(!Access::Read.covers(Access::ReadWrite));
    }

    #[test]
    fn path_pattern_rejects_dotdot() {
        assert!(PathPattern::parse("a/../b").is_err());
        assert!(PathPattern::parse("..").is_err());
        assert!(PathPattern::parse("a..b").is_ok());
    }

    #[test]
    fn under_rebases_path_rules_to_the_work_folder() {
        assert_eq!(
            PathPattern::parse("todo/cli.py")
                .unwrap()
                .under("/tmp/wt/s1", None),
            Some(Under::Pattern("todo/cli.py".into()))
        );
        assert_eq!(
            PathPattern::parse("/tmp/wt/s1/todo/cli.py")
                .unwrap()
                .under("/tmp/wt/s1", None),
            Some(Under::Pattern("/todo/cli.py".into()))
        );
        assert_eq!(
            PathPattern::parse("~/proj/a.py")
                .unwrap()
                .under("/home/u/proj", Some("/home/u")),
            Some(Under::Pattern("/a.py".into()))
        );
        assert_eq!(
            PathPattern::parse("/tmp/")
                .unwrap()
                .under("/tmp/wt/s1", None),
            Some(Under::Root)
        );
        assert_eq!(
            PathPattern::parse("/tmp/wt/s1")
                .unwrap()
                .under("/tmp/wt/s1", None),
            Some(Under::Root)
        );
        assert_eq!(
            PathPattern::parse("/tmp/*/")
                .unwrap()
                .under("/tmp/wt", None),
            None
        );
        assert_eq!(
            PathPattern::parse("/etc/x")
                .unwrap()
                .under("/tmp/wt/s1", None),
            None
        );
        assert_eq!(
            PathPattern::parse("/home/u/project2/x")
                .unwrap()
                .under("/home/u/project", None),
            None
        );
    }
}
