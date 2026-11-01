//! Builds the `rules` crate's requests for one tool call and keeps the worst
//! decision: the agent loop only enforces the crate's answer.

use crate::agent::ports::RuleCheck;
use crate::tools::path_guard;
use rules::{Access, Decision, Request, Rules};
use serde_json::Value;
use std::path::Path;

pub(super) fn check<'r>(
    rules: &'r Rules,
    workdir: &Path,
    home: Option<&str>,
    name: &str,
    args: &Value,
) -> RuleCheck<'r> {
    let mut check = RuleCheck {
        decision: rules.decide(&Request::Tool(name)),
        subject: name.to_string(),
    };
    let mut weigh = |decision: Decision<'r>, subject: &dyn Fn() -> String| {
        let combined = combine(check.decision, decision);
        if combined != check.decision {
            check = RuleCheck {
                decision: combined,
                subject: subject(),
            };
        }
    };

    let argument = |key: &str| args.get(key).and_then(Value::as_str);
    if name == "run_command"
        && let Some(command) = argument("command")
    {
        weigh(rules.decide(&Request::Command(command)), &|| {
            command.to_string()
        });
    }

    let access = match name {
        "read_file" => Some(Access::Read),
        "write_file" | "edit_file" => Some(Access::Write),
        _ => None,
    };
    if let Some(access) = access
        && let Some(path) = argument("path")
        && let Some(request) = path_guard::path_request(workdir, home, path, access)
    {
        weigh(rules.decide(&Request::Path(&request)), &|| {
            request
                .relative
                .clone()
                .unwrap_or_else(|| request.absolute.clone())
        });
    }

    check
}

/// `Decision::worst` keeps the left decision on a tie. An explicit rule is
/// more useful in the log than the default, so a tied `Some(rule)` wins over
/// a tied `None`.
fn combine<'r>(current: Decision<'r>, other: Decision<'r>) -> Decision<'r> {
    let worst = current.worst(other);
    if worst == current && !has_rule(current) && has_rule(other) {
        other
    } else {
        worst
    }
}

fn has_rule(decision: Decision) -> bool {
    matches!(
        decision,
        Decision::Allow(Some(_)) | Decision::Ask(Some(_)) | Decision::Block(_)
    )
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::domain::id::new_id;
    use rules::{Access, Action, Decision, Origin, Rule, Rules, Scope, Target, TargetKind};
    use serde_json::json;
    use std::path::PathBuf;

    fn temp_workdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fennec-rules-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn command_rule(id: i64, pattern: &str, action: Action) -> Rule {
        Rule {
            id,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Command, pattern, None).unwrap(),
            action,
            note: None,
            origin: Origin::Human,
        }
    }

    fn tool_rule(id: i64, pattern: &str, action: Action) -> Rule {
        Rule {
            id,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Tool, pattern, None).unwrap(),
            action,
            note: None,
            origin: Origin::Human,
        }
    }

    fn path_rule(id: i64, pattern: &str, access: Access, action: Action) -> Rule {
        Rule {
            id,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Path, pattern, Some(access)).unwrap(),
            action,
            note: None,
            origin: Origin::Human,
        }
    }

    #[test]
    fn a_block_command_rule_refuses_the_whole_command() {
        let workdir = temp_workdir();
        let rules = Rules::new(vec![command_rule(1, "rm -rf", Action::Block)]);

        let refused = check(
            &rules,
            &workdir,
            None,
            "run_command",
            &json!({"command": "cd x && rm -fr y"}),
        );
        assert!(matches!(
            refused.decision,
            Decision::Block(rule) if rule.id == 1
        ));
        assert_eq!(refused.subject, "cd x && rm -fr y");

        let allowed = check(
            &rules,
            &workdir,
            None,
            "run_command",
            &json!({"command": "ls"}),
        );
        assert_eq!(allowed.decision, Decision::Allow(None));

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn a_block_tool_rule_refuses_the_tool_name() {
        let workdir = temp_workdir();
        let rules = Rules::new(vec![tool_rule(1, "web_search", Action::Block)]);

        let refused = check(&rules, &workdir, None, "web_search", &json!({}));
        assert!(matches!(
            refused.decision,
            Decision::Block(rule) if rule.id == 1
        ));
        assert_eq!(refused.subject, "web_search");

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn a_block_path_rule_refuses_matching_write_paths() {
        let workdir = temp_workdir();
        let rules = Rules::new(vec![path_rule(1, "secrets/", Access::Write, Action::Block)]);

        let relative = check(
            &rules,
            &workdir,
            None,
            "write_file",
            &json!({"path": "secrets/a.txt"}),
        );
        assert!(matches!(
            relative.decision,
            Decision::Block(rule) if rule.id == 1
        ));
        assert_eq!(relative.subject, "secrets/a.txt");

        let absolute = check(
            &rules,
            &workdir,
            None,
            "write_file",
            &json!({"path": workdir.join("secrets/a.txt").to_string_lossy()}),
        );
        assert!(matches!(
            absolute.decision,
            Decision::Block(rule) if rule.id == 1
        ));
        assert_eq!(absolute.subject, "secrets/a.txt");

        let other = check(
            &rules,
            &workdir,
            None,
            "write_file",
            &json!({"path": "src/a.rs"}),
        );
        assert_eq!(other.decision, Decision::Allow(None));

        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn a_read_outside_the_workdir_asks_by_default() {
        let workdir = temp_workdir();
        let outside = std::env::temp_dir().join(format!("fennec-rules-outside-{}", new_id()));
        std::fs::write(&outside, "outside").unwrap();
        let rules = Rules::default();

        let check = check(
            &rules,
            &workdir,
            None,
            "read_file",
            &json!({"path": outside.to_string_lossy()}),
        );
        assert_eq!(check.decision, Decision::Ask(None));

        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn an_allow_path_rule_grants_a_home_path_read() {
        let workdir = temp_workdir();
        let home = std::env::temp_dir().join(format!("fennec-rules-home-{}", new_id()));
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(home.join(".ssh").join("config"), "host example").unwrap();
        let rules = Rules::new(vec![path_rule(1, "~/.ssh/", Access::Read, Action::Allow)]);

        let check = check(
            &rules,
            &workdir,
            Some(home.to_string_lossy().as_ref()),
            "read_file",
            &json!({"path": home.join(".ssh").join("config").to_string_lossy()}),
        );
        assert!(matches!(
            check.decision,
            Decision::Allow(Some(rule)) if rule.id == 1
        ));

        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&workdir);
    }
}
