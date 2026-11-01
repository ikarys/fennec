//! Rule types and in-memory rule evaluation.

use serde::{Deserialize, Serialize};

use crate::command::{CommandPattern, PatternError, segments};

/// Where a rule applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Global,
    Project(String),
}

/// What fennec does when a rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Block,
    Ask,
}

/// The kind of thing a rule targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    Command,
    Tool,
    Path,
}

/// A rule target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Command(CommandPattern),
    /// A tool name, e.g. `web_search`.
    Tool(String),
    /// A gitignore-like pattern; the host evaluates it (this crate only carries it).
    Path(String),
}

/// A single stored rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub scope: Scope,
    pub target: Target,
    pub action: Action,
    pub note: Option<String>,
}

impl Target {
    /// Parses a pattern string into the requested target kind.
    pub fn parse(kind: TargetKind, pattern: &str) -> Result<Target, PatternError> {
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
                let path = pattern.trim();
                if path.is_empty() {
                    return Err(PatternError("path pattern is empty".to_string()));
                }
                Ok(Target::Path(path.to_string()))
            }
        }
    }

    /// Returns the target kind.
    pub fn kind(&self) -> TargetKind {
        match self {
            Target::Command(_) => TargetKind::Command,
            Target::Tool(_) => TargetKind::Tool,
            Target::Path(_) => TargetKind::Path,
        }
    }

    /// Returns the raw pattern text for display and refusal messages.
    pub fn pattern(&self) -> String {
        match self {
            Target::Command(command) => command.text().to_string(),
            Target::Tool(tool) => tool.clone(),
            Target::Path(path) => path.clone(),
        }
    }
}

impl Rule {
    /// Returns the refusal text shown to the model.
    pub fn refusal(&self) -> String {
        let scope = match self.scope {
            Scope::Global => "global",
            Scope::Project(_) => "project",
        };
        let pattern = self.target.pattern();
        let mut text = format!("Refused by fennec rule \"{pattern}\" ({scope})");
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

    /// Returns the winning command rule, if any.
    pub fn check_command(&self, command: &str) -> Option<&Rule> {
        let command_segments = segments(command);
        Self::pick(
            self.0
                .iter()
                .filter(|rule| command_matches_rule(rule, &command_segments)),
        )
    }

    /// Returns the winning tool rule, if any.
    pub fn check_tool(&self, name: &str) -> Option<&Rule> {
        Self::pick(
            self.0
                .iter()
                .filter(|rule| matches!(&rule.target, Target::Tool(tool) if tool == name)),
        )
    }

    /// Returns path rules for the host to evaluate.
    pub fn paths(&self) -> impl Iterator<Item = &Rule> {
        self.0
            .iter()
            .filter(|rule| matches!(rule.target, Target::Path(_)))
    }

    /// Chooses the strongest matching rule: Block beats Ask, then first wins.
    pub fn pick<'a>(matches: impl Iterator<Item = &'a Rule>) -> Option<&'a Rule> {
        let mut best: Option<&'a Rule> = None;
        for rule in matches {
            match best {
                None => best = Some(rule),
                Some(current) => {
                    if rule.action == Action::Block && current.action == Action::Ask {
                        best = Some(rule);
                    }
                }
            }
        }
        best
    }
}

fn command_matches_rule(rule: &Rule, command_segments: &[Vec<String>]) -> bool {
    if let Target::Command(pattern) = &rule.target {
        command_segments
            .iter()
            .any(|segment| pattern.matches(segment))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, action: Action, target: Target) -> Rule {
        Rule {
            id: id.to_string(),
            scope: Scope::Global,
            target,
            action,
            note: None,
        }
    }

    #[test]
    fn block_wins_over_ask() {
        let ask = rule(
            "ask",
            Action::Ask,
            Target::Command(CommandPattern::parse("rm -rf").unwrap()),
        );
        let block = rule(
            "block",
            Action::Block,
            Target::Command(CommandPattern::parse("rm -rf").unwrap()),
        );
        let rules = Rules::new(vec![ask, block]);
        let matched = rules.check_command("rm -rf /tmp/x").unwrap();
        assert_eq!(matched.id, "block");
    }

    #[test]
    fn check_tool_is_exact() {
        let rules = Rules::new(vec![rule(
            "web",
            Action::Block,
            Target::Tool("web_search".into()),
        )]);
        assert!(rules.check_tool("web_search").is_some());
        assert!(rules.check_tool("web").is_none());
    }

    #[test]
    fn refusal_without_note() {
        let r = rule(
            "rm",
            Action::Block,
            Target::Command(CommandPattern::parse("rm -rf").unwrap()),
        );
        assert_eq!(
            r.refusal(),
            "Refused by fennec rule \"rm -rf\" (global). Do not retry it in another form."
        );
    }

    #[test]
    fn refusal_with_note() {
        let mut r = rule(
            "rm",
            Action::Block,
            Target::Command(CommandPattern::parse("rm -rf").unwrap()),
        );
        r.note = Some("destructive".to_string());
        assert_eq!(
            r.refusal(),
            "Refused by fennec rule \"rm -rf\" (global): destructive. Do not retry it in another form."
        );
    }

    #[test]
    fn target_parse_rejects_bad_tool() {
        assert!(Target::parse(TargetKind::Tool, "web search").is_err());
        assert!(Target::parse(TargetKind::Tool, "  ").is_err());
    }

    #[test]
    fn action_serde() {
        let action: Action = serde_json::from_str(r#""block""#).unwrap();
        assert_eq!(action, Action::Block);
    }
}
