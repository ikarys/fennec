//! The human's rules, checked before a tool call runs: fennec's code decides,
//! never the model. The `rules` crate owns the decision; this file turns a
//! `RuleCheck` into the agent's refusal, its approval request, and log events.

use crate::agent::ports::RuleCheck;
use crate::domain::event::Event;
use rules::{Access, Rule};

/// Refuses a call when the human cannot be asked: a `Block` rule, or an
/// explicit `Ask` while the human is away. An `Ask` with no rule (a path
/// outside the project) lets the tool run, whose own guard denies the path
/// unless an `Allow` rule grants it.
pub(super) fn refusal<'r>(check: &RuleCheck<'r>) -> Option<&'r Rule> {
    match check.decision {
        rules::Decision::Block(rule) => Some(rule),
        rules::Decision::Ask(Some(rule)) => Some(rule),
        rules::Decision::Allow(_) | rules::Decision::Ask(None) => None,
    }
}

/// The pause for the human: what is asked, and the rule each answer would store.
pub(super) fn approval(call_id: &str, rule: Option<&Rule>, check: &RuleCheck) -> Event {
    let (kind, pattern, access) = match &check.proposal {
        Some(proposal) => (
            proposal.kind.as_str().to_string(),
            proposal.pattern.clone(),
            proposal.access.map(Access::as_str).map(str::to_string),
        ),
        None => ("tool".to_string(), check.subject.clone(), None),
    };
    Event::RuleApprovalRequested {
        call_id: call_id.to_string(),
        subject: check.subject.clone(),
        rule_id: rule.map(|rule| rule.id.to_string()),
        kind,
        pattern,
        access,
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
    use super::{approval, event, refusal};
    use crate::agent::ports::RuleCheck;
    use crate::domain::event::Event;
    use rules::{Action, Origin, Proposal, Rule, Rules, Scope, Target, TargetKind};

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
            proposal: None,
        };
        assert!(refusal(&block_check).is_some_and(|r| r.id == 1));

        let ask_check = RuleCheck {
            decision: rules::Decision::Ask(Some(ask)),
            subject: "web_search".into(),
            proposal: None,
        };
        assert!(refusal(&ask_check).is_some_and(|r| r.id == 2));

        let default_ask_check = RuleCheck {
            decision: rules::Decision::Ask(None),
            subject: "/outside".into(),
            proposal: None,
        };
        assert!(refusal(&default_ask_check).is_none());

        let allow_check = RuleCheck {
            decision: rules::Decision::Allow(Some(allow)),
            subject: "web_search".into(),
            proposal: None,
        };
        assert!(refusal(&allow_check).is_none());
    }

    #[test]
    fn approval_uses_the_check_proposal() {
        let rule = Rule {
            id: 1,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Command, "touch", None).unwrap(),
            action: Action::Ask,
            note: None,
            origin: Origin::Human,
        };
        let check = RuleCheck {
            decision: rules::Decision::Ask(Some(&rule)),
            subject: "touch made".into(),
            proposal: Some(Proposal {
                kind: TargetKind::Command,
                pattern: "touch made".into(),
                access: None,
            }),
        };

        assert!(matches!(
            approval("c1", Some(&rule), &check),
            Event::RuleApprovalRequested {
                call_id,
                subject,
                rule_id,
                kind,
                pattern,
                access
            } if call_id == "c1"
                && subject == "touch made"
                && rule_id == Some("1".into())
                && kind == "command"
                && pattern == "touch made"
                && access.is_none()
        ));
    }

    #[test]
    fn approval_without_a_proposal_asks_for_the_tool() {
        let check = RuleCheck {
            decision: rules::Decision::Ask(None),
            subject: "/outside/file".into(),
            proposal: None,
        };

        assert!(matches!(
            approval("c2", None, &check),
            Event::RuleApprovalRequested {
                call_id,
                subject,
                rule_id,
                kind,
                pattern,
                access
            } if call_id == "c2"
                && subject == "/outside/file"
                && rule_id.is_none()
                && kind == "tool"
                && pattern == "/outside/file"
                && access.is_none()
        ));
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
