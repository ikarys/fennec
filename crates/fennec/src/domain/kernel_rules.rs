//! The rules fennec's kernel enforces itself, before the human's: the git commands that push,
//! or create, move or delete a branch, which the model never runs because fennec integrates
//! (rule 1 of AGENTS.md); built in code so no store and no UI can drop them; a guard rail,
//! the sandbox stays the wall.

use rules::{Action, Origin, Rule, Scope, Target, TargetKind};

/// The command patterns the kernel blocks, in `rules` crate syntax.
const BLOCKED_COMMANDS: &[&str] = &[
    "git push",
    "git update-ref",
    "git symbolic-ref",
    "git branch -D",
    "git branch -d",
    "git branch --delete",
    "git branch -f",
    "git branch --force",
    "git branch -m",
    "git branch -M",
    "git branch --move",
    "git switch -C",
    "git checkout -B",
    "git worktree",
    "gh pr merge",
];

const NOTE: &str = "fennec's own rule: the human integrates and pushes from fennec; the model never pushes, nor creates, moves or deletes a branch";

/// The kernel's rules: global `Block` rules with negative ids (a stored rule's id is positive), origin `Fennec`.
pub fn all() -> Vec<Rule> {
    BLOCKED_COMMANDS
        .iter()
        .enumerate()
        .map(|(index, pattern)| Rule {
            id: -(index as i64) - 1,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Command, pattern, None).expect(
                "a kernel pattern parses; the list is a const, so failure is a programming error",
            ),
            action: Action::Block,
            note: Some(NOTE.to_string()),
            origin: Origin::Fennec,
        })
        .collect()
}

/// The human's rules with the kernel's appended: what a turn decides with.
pub fn with_kernel(human: rules::Rules) -> rules::Rules {
    rules::Rules::new(human.all().iter().cloned().chain(all()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rules::{Decision, Request};

    fn assert_kernel_block(decision: Decision<'_>, command: &str) {
        assert!(
            matches!(
                decision,
                Decision::Block(rule)
                    if matches!(&rule.origin, Origin::Fennec) && rule.id < 0
            ),
            "expected a kernel block for {command}"
        );
    }

    #[test]
    fn blocks_push_variants() {
        let rules = with_kernel(rules::Rules::default());
        for command in [
            "git push origin main",
            "git -C /repo push --force",
            "cd a && git push",
            "sh -c \"git push\"",
        ] {
            let decision = rules.decide(&Request::Command(command));
            assert_kernel_block(decision, command);
        }
    }

    #[test]
    fn blocks_branch_and_ref_changes() {
        let rules = with_kernel(rules::Rules::default());
        for command in [
            "git branch -D feature",
            "git update-ref refs/heads/main abc",
            "git worktree add ../x",
            "gh pr merge 3",
        ] {
            let decision = rules.decide(&Request::Command(command));
            assert_kernel_block(decision, command);
        }
    }

    #[test]
    fn allows_read_only_and_local_git_commands() {
        let rules = with_kernel(rules::Rules::default());
        for command in [
            "git status",
            "git commit -m x",
            "git branch",
            "git log --oneline",
            "git switch feature",
            "git checkout -b feature",
            "gh pr view 3",
        ] {
            let decision = rules.decide(&Request::Command(command));
            assert!(
                matches!(decision, Decision::Allow(_)),
                "expected allow for {command}"
            );
        }
    }

    #[test]
    fn human_allow_does_not_override_kernel_block() {
        let human = Rule {
            id: 1,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Command, "git push", None).unwrap(),
            action: Action::Allow,
            note: None,
            origin: Origin::Human,
        };
        let rules = with_kernel(rules::Rules::new(vec![human]));
        let decision = rules.decide(&Request::Command("git push"));
        assert_kernel_block(decision, "git push");
    }

    #[test]
    fn kernel_rules_have_negative_distinct_ids() {
        let rules = all();
        assert_eq!(rules.len(), BLOCKED_COMMANDS.len());
        let mut ids: Vec<i64> = rules.iter().map(|rule| rule.id).collect();
        assert!(ids.iter().all(|&id| id < 0));
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), rules.len());
    }
}
