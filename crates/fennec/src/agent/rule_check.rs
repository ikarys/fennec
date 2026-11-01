//! The human's rules, checked before a tool call runs: fennec's code decides, never the model.
//! Tool rules match the call's name, command rules the `run_command` command, path rules the
//! `write_file`/`edit_file` path (relative to the workdir). The path check here is lexical and
//! exists to name the rule in the log; the hard guard is the same patterns merged into the
//! turn's `Protection` (canonical paths, file tools and sandbox).

use std::path::Path;

use crate::domain::event::{Event, ToolCall};
use crate::domain::protection::Protection;
use rules::{Rule, Rules, Scope, TargetKind};

pub(super) struct Refusal<'r> {
    pub rule: &'r Rule,
    pub subject: String,
}

pub(super) fn check<'r>(rules: &'r Rules, workdir: &Path, call: &ToolCall) -> Option<Refusal<'r>> {
    let mut candidates: Vec<(&Rule, String)> = Vec::new();

    if let Some(rule) = rules.check_tool(&call.name) {
        candidates.push((rule, call.name.clone()));
    }

    let argument = |name: &str| call.arguments.get(name).and_then(|value| value.as_str());
    if call.name == "run_command"
        && let Some(command) = argument("command")
        && let Some(rule) = rules.check_command(command)
    {
        candidates.push((rule, command.to_string()));
    }

    if (call.name == "write_file" || call.name == "edit_file")
        && let Some(path) = argument("path")
    {
        let relative = relative(workdir, path);
        for rule in rules.paths() {
            if Protection::only(&[rule.target.pattern()]).is_protected(&relative) {
                candidates.push((rule, relative.clone()));
            }
        }
    }

    let rule = Rules::pick(candidates.iter().map(|(rule, _)| *rule))?;
    let subject = candidates
        .iter()
        .find(|(candidate, _)| std::ptr::eq(*candidate, rule))?
        .1
        .clone();

    Some(Refusal { rule, subject })
}

fn relative(workdir: &Path, path: &str) -> String {
    let workdir = workdir.to_string_lossy().replace('\\', "/");
    let mut path = path.replace('\\', "/");

    if !workdir.is_empty() {
        if path == workdir {
            return String::new();
        }
        if let Some(rest) = path.strip_prefix(&format!("{workdir}/")) {
            path = rest.to_string();
        }
    }

    path.trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

pub(super) fn event(call_id: &str, refusal: &Refusal) -> Event {
    let kind = match refusal.rule.target.kind() {
        TargetKind::Command => "command",
        TargetKind::Tool => "tool",
        TargetKind::Path => "path",
    };
    let scope = match &refusal.rule.scope {
        Scope::Global => "global",
        Scope::Project(_) => "project",
    };

    Event::RuleBlocked {
        call_id: call_id.to_string(),
        rule_id: refusal.rule.id.clone(),
        kind: kind.to_string(),
        pattern: refusal.rule.target.pattern(),
        scope: scope.to_string(),
        subject: refusal.subject.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rules::Target;
    use serde_json::json;

    fn call(name: &str, arguments: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "c1".into(),
            name: name.into(),
            arguments,
            parse_error: None,
            raw_arguments: None,
        }
    }

    #[test]
    fn command_rule_refuses_matching_run_command() {
        let rules = Rules::new(vec![Rule {
            id: "block-rm".into(),
            scope: Scope::Global,
            target: Target::parse(TargetKind::Command, "rm -rf").unwrap(),
            action: rules::Action::Block,
            note: None,
        }]);
        let workdir = Path::new("/work");

        let refused = check(
            &rules,
            workdir,
            &call("run_command", json!({"command": "cd x && rm -fr y"})),
        );
        assert!(refused.is_some());
        assert_eq!(refused.unwrap().subject, "cd x && rm -fr y");

        let allowed = check(
            &rules,
            workdir,
            &call("run_command", json!({"command": "ls"})),
        );
        assert!(allowed.is_none());
    }

    #[test]
    fn tool_rule_refuses_matching_tool_name() {
        let rules = Rules::new(vec![Rule {
            id: "block-web".into(),
            scope: Scope::Global,
            target: Target::parse(TargetKind::Tool, "web_search").unwrap(),
            action: rules::Action::Block,
            note: None,
        }]);
        let workdir = Path::new("/work");

        let refused = check(&rules, workdir, &call("web_search", json!({})));
        assert!(refused.is_some());
        assert_eq!(refused.unwrap().subject, "web_search");
    }

    #[test]
    fn path_rule_refuses_matching_write_paths() {
        let rules = Rules::new(vec![Rule {
            id: "block-secrets".into(),
            scope: Scope::Global,
            target: Target::parse(TargetKind::Path, "secrets/").unwrap(),
            action: rules::Action::Block,
            note: None,
        }]);
        let workdir = Path::new("/work");

        let relative = check(
            &rules,
            workdir,
            &call("write_file", json!({"path": "secrets/a.txt"})),
        );
        assert!(relative.is_some());
        assert_eq!(relative.unwrap().subject, "secrets/a.txt");

        let absolute = check(
            &rules,
            workdir,
            &call("write_file", json!({"path": "/work/secrets/a.txt"})),
        );
        assert!(absolute.is_some());
        assert_eq!(absolute.unwrap().subject, "secrets/a.txt");

        let other = check(
            &rules,
            workdir,
            &call("write_file", json!({"path": "src/a.rs"})),
        );
        assert!(other.is_none());
    }

    #[test]
    fn block_rule_beats_ask_rule() {
        let rules = Rules::new(vec![
            Rule {
                id: "ask-rm".into(),
                scope: Scope::Global,
                target: Target::parse(TargetKind::Command, "rm").unwrap(),
                action: rules::Action::Ask,
                note: None,
            },
            Rule {
                id: "block-rm".into(),
                scope: Scope::Global,
                target: Target::parse(TargetKind::Command, "rm").unwrap(),
                action: rules::Action::Block,
                note: None,
            },
        ]);
        let workdir = Path::new("/work");

        let refused = check(
            &rules,
            workdir,
            &call("run_command", json!({"command": "rm x"})),
        );
        assert!(refused.is_some());
        assert_eq!(refused.unwrap().rule.id, "block-rm");
    }
}
