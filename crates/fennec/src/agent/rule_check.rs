//! The human's rules, checked before a tool call runs: fennec's code decides,
//! never the model. The `rules` crate owns the decision; this file only turns
//! a `RuleCheck` into the agent's refusal and log event.

use crate::agent::ports::RuleCheck;
use crate::domain::event::Event;
use rules::Rule;

/// An `Ask` rule is refused until the approval flow lands. An `Ask` with no
/// rule (a path outside the project) lets the tool run, whose own guard denies
/// the path unless an `Allow` rule grants it.
pub(super) fn refusal<'r>(check: &RuleCheck<'r>) -> Option<&'r Rule> {
    match check.decision {
        rules::Decision::Block(rule) => Some(rule),
        rules::Decision::Ask(Some(rule)) => Some(rule),
        rules::Decision::Allow(_) | rules::Decision::Ask(None) => None,
    }
}

pub(super) fn event(call_id: &str, rule: &Rule, subject: &str) -> Event {
    Event::RuleBlocked {
        call_id: call_id.to_string(),
        rule_id: rule.id.to_string(),
        kind: rule.target.kind().as_str().to_string(),
        pattern: rule.target.pattern(),
        scope: rule.scope.label().to_string(),
        subject: subject.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{event, refusal};
    use crate::agent::ports::RuleCheck;
    use crate::domain::event::Event;
    use rules::{Action, Origin, Rule, Rules, Scope, Target, TargetKind};

    fn rule(id: i64, action: Action) -> Rule {
        Rule {
            id,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Tool, "web_search", None).unwrap(),
            action,
            note: None,
            origin: Origin::Human,
        }
    }

    #[test]
    fn refusal_uses_block_and_explicit_ask_rules_only() {
        let rules = Rules::new(vec![
            rule(1, Action::Block),
            rule(2, Action::Ask),
            rule(3, Action::Allow),
        ]);
        let (block, ask, allow) = (&rules.all()[0], &rules.all()[1], &rules.all()[2]);

        let block_check = RuleCheck {
            decision: rules::Decision::Block(block),
            subject: "web_search".into(),
        };
        assert!(refusal(&block_check).is_some_and(|r| r.id == 1));

        let ask_check = RuleCheck {
            decision: rules::Decision::Ask(Some(ask)),
            subject: "web_search".into(),
        };
        assert!(refusal(&ask_check).is_some_and(|r| r.id == 2));

        let default_ask_check = RuleCheck {
            decision: rules::Decision::Ask(None),
            subject: "/outside".into(),
        };
        assert!(refusal(&default_ask_check).is_none());

        let allow_check = RuleCheck {
            decision: rules::Decision::Allow(Some(allow)),
            subject: "web_search".into(),
        };
        assert!(refusal(&allow_check).is_none());
    }

    #[test]
    fn event_names_the_rule_and_subject() {
        let rule = Rule {
            id: 7,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Command, "rm -rf", None).unwrap(),
            action: Action::Block,
            note: None,
            origin: Origin::Human,
        };

        let event = event("c1", &rule, "rm -rf target");
        assert!(matches!(
            event,
            Event::RuleBlocked {
                call_id,
                rule_id,
                kind,
                pattern,
                scope,
                subject
            } if call_id == "c1"
                && rule_id == "7"
                && kind == "command"
                && pattern == "rm -rf"
                && scope == "global"
                && subject == "rm -rf target"
        ));
    }
}
