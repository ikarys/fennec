//! Template names and rendering for workflow prompts, commands, and questions.

use super::definition::{NodeKind, Workflow};

pub fn names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut pos = 0;

    while let Some(offset) = text[pos..].find("{{") {
        let start = pos + offset;
        let rest = &text[start + 2..];

        match rest.find("}}") {
            Some(end) => {
                let name = rest[..end].trim();
                if !name.is_empty() {
                    names.push(name.to_string());
                }
                pos = start + 2 + end + 2;
            }
            None => break,
        }
    }

    names
}

pub fn render(text: &str, value: &dyn Fn(&str) -> Option<String>, quote: bool) -> String {
    let mut out = String::new();
    let mut pos = 0;

    while let Some(offset) = text[pos..].find("{{") {
        let start = pos + offset;
        let rest = &text[start + 2..];

        match rest.find("}}") {
            Some(end) => {
                let name = rest[..end].trim();
                out.push_str(&text[pos..start]);

                let value = value(name).unwrap_or_default();
                if quote {
                    out.push_str(&shell_quote(&value));
                } else {
                    out.push_str(&value);
                }

                pos = start + 2 + end + 2;
            }
            None => break,
        }
    }

    out.push_str(&text[pos..]);
    out
}

pub fn check_name(workflow: &Workflow, name: &str) -> Result<(), String> {
    match name {
        "task" | "filesChanged" | "pass" | "comment" | "criteria" => Ok(()),
        _ if name.starts_with("nodes.") => {
            let rest = &name["nodes.".len()..];
            let (id, field) = rest
                .split_once('.')
                .filter(|(id, _)| !id.is_empty())
                .ok_or_else(|| format!("unknown template name {{{{{name}}}}}"))?;

            let node = workflow
                .node(id)
                .ok_or_else(|| format!("unknown template name {{{{{name}}}}}"))?;

            match field {
                "output" | "port" => Ok(()),
                "exitCode" => {
                    if matches!(node.kind, NodeKind::Shell { .. }) {
                        Ok(())
                    } else {
                        Err(format!("unknown template name {{{{{name}}}}}"))
                    }
                }
                _ => Err(format!("unknown template name {{{{{name}}}}}")),
            }
        }
        _ => Err(format!("unknown template name {{{{{name}}}}}")),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workflow::definition::Node;

    fn workflow_with_shell() -> Workflow {
        Workflow {
            id: "w".into(),
            name: "w".into(),
            description: String::new(),
            max_steps: 50,
            nodes: vec![Node {
                id: "s".into(),
                name: None,
                position: Default::default(),
                kind: NodeKind::Shell {
                    command: "cmd".into(),
                    timeout_secs: None,
                    success_codes: None,
                },
            }],
            edges: Vec::new(),
        }
    }

    #[test]
    fn names_keeps_order_and_duplicates() {
        assert_eq!(
            names("{{task}} {{nodes.a.output}} {{task}}"),
            vec!["task", "nodes.a.output", "task"]
        );
    }

    #[test]
    fn names_ignores_unclosed_braces() {
        assert_eq!(names("{{a}} {{b"), vec!["a"]);
        assert!(names("{{a").is_empty());
    }

    #[test]
    fn render_replaces_values_and_omits_missing() {
        let rendered = render(
            "a {{x}} b {{missing}} c",
            &|name| (name == "x").then(|| "v".to_string()),
            false,
        );
        assert_eq!(rendered, "a v b  c");
    }

    #[test]
    fn render_quotes_shell_values() {
        let rendered = render(
            "echo {{x}}",
            &|name| (name == "x").then(|| "it's".to_string()),
            true,
        );
        assert_eq!(rendered, r"echo 'it'\''s'");
    }

    #[test]
    fn check_name_accepts_known_names() {
        let workflow = workflow_with_shell();
        assert!(check_name(&workflow, "task").is_ok());
        assert!(check_name(&workflow, "filesChanged").is_ok());
        assert!(check_name(&workflow, "pass").is_ok());
        assert!(check_name(&workflow, "comment").is_ok());
        assert!(check_name(&workflow, "criteria").is_ok());
        assert!(check_name(&workflow, "nodes.s.output").is_ok());
        assert!(check_name(&workflow, "nodes.s.port").is_ok());
        assert!(check_name(&workflow, "nodes.s.exitCode").is_ok());
    }

    #[test]
    fn check_name_rejects_unknown_names() {
        let workflow = workflow_with_shell();
        assert_eq!(
            check_name(&workflow, "nope").err(),
            Some("unknown template name {{nope}}".into())
        );
        assert_eq!(
            check_name(&workflow, "nodes.missing.output").err(),
            Some("unknown template name {{nodes.missing.output}}".into())
        );
        assert_eq!(check_name(&workflow, "nodes.s.exitCode").err(), None);
    }
}
