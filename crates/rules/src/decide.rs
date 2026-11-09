//! Decision logic: turn a request into Allow, Ask, or Block.

use crate::rule::{Access, Action, Rule, Rules, Scope, Target};
use crate::shell::segments;

/// A canonicalized path request from the host.
///
/// The host canonicalizes the path itself because it is trusted code. The crate only
/// compares the supplied forms against stored patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRequest {
    pub absolute: String,
    pub relative: Option<String>,
    pub home: Option<String>,
    pub access: Access,
}

/// A request to decide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request<'a> {
    Command(&'a str),
    Tool(&'a str),
    Path(&'a PathRequest),
}

/// The decision for a request.
///
/// The `Option` is the rule that decided; `None` is the default when no rule matched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision<'r> {
    Allow(Option<&'r Rule>),
    Ask(Option<&'r Rule>),
    Block(&'r Rule),
}

impl<'r> Decision<'r> {
    /// Returns the stricter of two decisions.
    ///
    /// Block beats Ask beats Allow. On a tie, `self` is kept so the first stored rule
    /// wins when scopes are equal.
    pub fn worst(self, other: Decision<'r>) -> Decision<'r> {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Decision::Allow(_) => 0,
            Decision::Ask(_) => 1,
            Decision::Block(_) => 2,
        }
    }
}

impl Rules {
    /// Decides a request against all rules in this collection.
    pub fn decide<'r>(&'r self, request: &Request) -> Decision<'r> {
        match request {
            Request::Command(command) => self.decide_command(command),
            Request::Tool(name) => self.decide_tool(name),
            Request::Path(path) => self.decide_path(path),
        }
    }

    fn decide_command<'r>(&'r self, command: &str) -> Decision<'r> {
        // A compound command is only as safe as its weakest segment.
        segments(command)
            .iter()
            .map(|segment| self.decide_segment(segment))
            .reduce(Decision::worst)
            .unwrap_or(Decision::Allow(None))
    }

    fn decide_segment<'r>(&'r self, segment: &[String]) -> Decision<'r> {
        self.select(Decision::Allow(None), |rule| match &rule.target {
            // An Allow matches exactly, so an allowed command never covers added flags
            // or words; Block and Ask match as a subset (`rm -rf` covers `rm -rf /`).
            Target::Command(pattern) if rule.action == Action::Allow => {
                pattern.matches_exactly(segment)
            }
            Target::Command(pattern) => pattern.matches(segment),
            _ => false,
        })
    }

    fn decide_tool<'r>(&'r self, name: &str) -> Decision<'r> {
        self.select(
            Decision::Allow(None),
            |rule| matches!(&rule.target, Target::Tool(tool) if tool == name),
        )
    }

    fn decide_path<'r>(&'r self, request: &PathRequest) -> Decision<'r> {
        // Inside the project, no rule means allow. Outside the project, ask before a
        // tool reaches a path the project rules do not cover.
        let default = if request.relative.is_some() {
            Decision::Allow(None)
        } else {
            Decision::Ask(None)
        };
        self.select(default, |rule| match &rule.target {
            Target::Path { pattern, access } => {
                access.covers(request.access) && pattern.matches(request)
            }
            _ => false,
        })
    }

    /// The matching rules' decision: Block, then Allow, then Ask, each won by its most
    /// specific scope; `default` when none matches.
    fn select<'r>(
        &'r self,
        default: Decision<'r>,
        matches: impl Fn(&Rule) -> bool,
    ) -> Decision<'r> {
        let (mut block, mut allow, mut ask) = (None, None, None);
        for rule in self.all().iter().filter(|rule| matches(rule)) {
            let slot = match rule.action {
                Action::Block => &mut block,
                Action::Allow => &mut allow,
                Action::Ask => &mut ask,
            };
            *slot = Self::better_scope(*slot, rule);
        }
        Self::choose(block, allow, ask, default)
    }

    fn choose<'r>(
        block: Option<&'r Rule>,
        allow: Option<&'r Rule>,
        ask: Option<&'r Rule>,
        default: Decision<'r>,
    ) -> Decision<'r> {
        if let Some(rule) = block {
            Decision::Block(rule)
        } else if let Some(rule) = allow {
            Decision::Allow(Some(rule))
        } else if let Some(rule) = ask {
            Decision::Ask(Some(rule))
        } else {
            default
        }
    }

    fn better_scope<'r>(current: Option<&'r Rule>, candidate: &'r Rule) -> Option<&'r Rule> {
        match current {
            None => Some(candidate),
            Some(current_rule)
                if scope_rank(&candidate.scope) > scope_rank(&current_rule.scope) =>
            {
                Some(candidate)
            }
            Some(current_rule) => Some(current_rule),
        }
    }
}

fn scope_rank(scope: &Scope) -> u8 {
    match scope {
        Scope::Global => 1,
        Scope::Project(_) => 2,
        Scope::Session(_) => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::{Origin, TargetKind};

    fn rule(id: i64, scope: Scope, action: Action, target: Target) -> Rule {
        Rule {
            id,
            scope,
            target,
            action,
            note: None,
            origin: Origin::Human,
        }
    }

    fn command_target(pattern: &str) -> Target {
        Target::parse(TargetKind::Command, pattern, None).unwrap()
    }

    #[test]
    fn block_beats_allow_beats_ask() {
        let rules = Rules::new(vec![
            rule(1, Scope::Global, Action::Ask, command_target("git push")),
            rule(2, Scope::Global, Action::Allow, command_target("git push")),
            rule(3, Scope::Global, Action::Block, command_target("git push")),
        ]);
        let decision = rules.decide(&Request::Command("git push origin main"));
        assert!(matches!(decision, Decision::Block(rule) if rule.id == 3));
    }

    #[test]
    fn allow_beats_ask() {
        let rules = Rules::new(vec![
            rule(1, Scope::Global, Action::Ask, command_target("git push")),
            rule(
                2,
                Scope::Global,
                Action::Allow,
                command_target("git push origin main"),
            ),
        ]);
        let decision = rules.decide(&Request::Command("git push origin main"));
        assert!(matches!(decision, Decision::Allow(Some(rule)) if rule.id == 2));
    }

    #[test]
    fn session_beats_project_beats_global_within_one_action() {
        let rules = Rules::new(vec![
            rule(
                1,
                Scope::Global,
                Action::Ask,
                Target::Tool("web_search".into()),
            ),
            rule(
                2,
                Scope::Project("p".into()),
                Action::Ask,
                Target::Tool("web_search".into()),
            ),
            rule(
                3,
                Scope::Session("s".into()),
                Action::Ask,
                Target::Tool("web_search".into()),
            ),
        ]);
        let decision = rules.decide(&Request::Tool("web_search"));
        assert!(matches!(decision, Decision::Ask(Some(rule)) if rule.id == 3));
    }

    #[test]
    fn allowed_command_is_exact() {
        let rules = Rules::new(vec![
            rule(
                1,
                Scope::Global,
                Action::Allow,
                command_target("git push origin main"),
            ),
            rule(2, Scope::Global, Action::Ask, command_target("git push")),
        ]);

        let exact = rules.decide(&Request::Command("git push origin main"));
        assert!(matches!(exact, Decision::Allow(Some(rule)) if rule.id == 1));

        let with_flag = rules.decide(&Request::Command("git push origin main --force"));
        assert!(matches!(with_flag, Decision::Ask(Some(rule)) if rule.id == 2));
    }

    #[test]
    fn block_on_one_segment_blocks_compound_command() {
        let rules = Rules::new(vec![rule(
            1,
            Scope::Global,
            Action::Block,
            command_target("rm -rf"),
        )]);
        let decision = rules.decide(&Request::Command("cargo test && rm -rf /"));
        assert!(matches!(decision, Decision::Block(rule) if rule.id == 1));
    }

    #[test]
    fn path_default_inside_project_is_allow() {
        let rules = Rules::new(Vec::new());
        let request = PathRequest {
            absolute: "/project/file.txt".into(),
            relative: Some("file.txt".into()),
            home: None,
            access: Access::Read,
        };
        let decision = rules.decide(&Request::Path(&request));
        assert_eq!(decision, Decision::Allow(None));
    }

    #[test]
    fn path_default_outside_project_is_ask() {
        let rules = Rules::new(Vec::new());
        let request = PathRequest {
            absolute: "/tmp/file.txt".into(),
            relative: None,
            home: None,
            access: Access::Read,
        };
        let decision = rules.decide(&Request::Path(&request));
        assert_eq!(decision, Decision::Ask(None));
    }

    #[test]
    fn home_path_read_rule_matches_read_not_write() {
        let rules = Rules::new(vec![rule(
            1,
            Scope::Global,
            Action::Block,
            Target::parse(TargetKind::Path, "~/.ssh/", Some(Access::Read)).unwrap(),
        )]);

        let read = PathRequest {
            absolute: "/home/u/.ssh/config".into(),
            relative: None,
            home: Some("/home/u".into()),
            access: Access::Read,
        };
        let read_decision = rules.decide(&Request::Path(&read));
        assert!(matches!(read_decision, Decision::Block(rule) if rule.id == 1));

        let write = PathRequest {
            absolute: "/home/u/.ssh/config".into(),
            relative: None,
            home: Some("/home/u".into()),
            access: Access::Write,
        };
        let write_decision = rules.decide(&Request::Path(&write));
        assert_eq!(write_decision, Decision::Ask(None));
    }

    #[test]
    fn absolute_pattern_matches_subpath_not_prefix() {
        let rules = Rules::new(vec![rule(
            1,
            Scope::Global,
            Action::Block,
            Target::parse(TargetKind::Path, "/etc", None).unwrap(),
        )]);

        let passwd = PathRequest {
            absolute: "/etc/passwd".into(),
            relative: None,
            home: None,
            access: Access::Read,
        };
        let passwd_decision = rules.decide(&Request::Path(&passwd));
        assert!(matches!(passwd_decision, Decision::Block(rule) if rule.id == 1));

        let etcetera = PathRequest {
            absolute: "/etcetera".into(),
            relative: None,
            home: None,
            access: Access::Read,
        };
        let etcetera_decision = rules.decide(&Request::Path(&etcetera));
        assert_eq!(etcetera_decision, Decision::Ask(None));
    }

    #[test]
    fn relative_pattern_does_not_match_outside_project() {
        let rules = Rules::new(vec![rule(
            1,
            Scope::Global,
            Action::Block,
            Target::parse(TargetKind::Path, "secrets/", None).unwrap(),
        )]);

        let outside = PathRequest {
            absolute: "/outside/secrets/key".into(),
            relative: None,
            home: None,
            access: Access::Read,
        };
        let decision = rules.decide(&Request::Path(&outside));
        assert_eq!(decision, Decision::Ask(None));
    }
}
