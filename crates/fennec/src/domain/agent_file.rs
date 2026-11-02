//! fennec's own: an agent as the Settings form edits it, and the
//! `*.agent.md` file it renders to.
//!
//! The form sends fields, never a file: the frontmatter format stays in one
//! place, here, and the browser parses and writes none of it. `parse_agent_md`
//! reads back what `render` writes (the round trip is tested below).

use super::agent::Phase;
use super::preferences::ReasoningEffort;

/// What the human filled in the agent form.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDraft {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub phase: Option<Phase>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub subagent: bool,
    /// `commands: read-only` in the file: the agent's commands may not write.
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// The method under the frontmatter.
    #[serde(default)]
    pub prompt: String,
}

impl AgentDraft {
    /// The `*.agent.md` file this draft is.
    pub fn render(&self) -> String {
        let mut out = String::from("---\n");
        out.push_str(&field("id", &self.id));
        out.push_str(&field("name", &self.name));
        if !self.description.is_empty() {
            out.push_str(&field("description", &self.description));
        }
        if let Some(phase) = self.phase {
            out.push_str(&field("phase", &scalar(phase)));
        }
        if let Some(color) = self.color.as_deref().filter(|c| !c.is_empty()) {
            out.push_str(&field("color", color));
        }
        if self.subagent {
            out.push_str("subagent: true\n");
        }
        if self.read_only {
            out.push_str("commands: read-only\n");
        }
        if let Some(effort) = self.reasoning_effort {
            out.push_str(&field("reasoningEffort", &scalar(effort)));
        }
        if !self.allowed_tools.is_empty() {
            out.push_str("allowedTools:\n");
            for tool in &self.allowed_tools {
                out.push_str(&format!("  - {tool}\n"));
            }
        }
        out.push_str("---\n\n");
        out.push_str(self.prompt.trim());
        out.push('\n');
        out
    }
}

/// A serde-named enum as the single word the frontmatter holds.
fn scalar<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// One frontmatter line, quoted when the value could be read as YAML syntax
/// (a leading `#` colour, a `: ` that would look like a nested key).
fn field(key: &str, value: &str) -> String {
    let quote = value.starts_with('#')
        || value.starts_with('\'')
        || value.starts_with('"')
        || value.contains(": ")
        || value.contains('\n');
    if quote {
        format!("{key}: '{}'\n", value.replace('\'', "''"))
    } else {
        format!("{key}: {value}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::{CommandAccess, parse_agent_md};

    fn draft() -> AgentDraft {
        AgentDraft {
            id: "mine".into(),
            name: "Mien".into(),
            description: "Fait une chose".into(),
            phase: Some(Phase::Understand),
            color: Some("#a855f7".into()),
            subagent: false,
            read_only: true,
            reasoning_effort: Some(ReasoningEffort::High),
            allowed_tools: vec!["read_file".into(), "run_command".into()],
            prompt: "Ta méthode.".into(),
        }
    }

    #[test]
    fn what_the_form_renders_parses_back_to_the_same_agent() {
        let draft = draft();
        let agent = parse_agent_md(&draft.render()).unwrap();

        assert_eq!(agent.id, draft.id);
        assert_eq!(agent.name, draft.name);
        assert_eq!(agent.description, draft.description);
        assert_eq!(agent.phase, draft.phase);
        // A '#' value must be quoted or the frontmatter loses it.
        assert_eq!(agent.color, draft.color);
        assert!(!agent.subagent);
        assert_eq!(agent.commands, CommandAccess::ReadOnly);
        assert_eq!(agent.reasoning_effort, draft.reasoning_effort);
        assert_eq!(agent.allowed_tools, draft.allowed_tools);
        assert_eq!(agent.prompt, draft.prompt);
    }

    #[test]
    fn the_empty_options_are_left_out_of_the_file() {
        let draft = AgentDraft {
            description: String::new(),
            phase: None,
            color: None,
            read_only: false,
            reasoning_effort: None,
            allowed_tools: Vec::new(),
            ..draft()
        };
        let file = draft.render();

        for absent in [
            "description:",
            "phase:",
            "color:",
            "commands:",
            "reasoningEffort:",
            "allowedTools:",
        ] {
            assert!(!file.contains(absent), "{absent} in {file}");
        }
        let agent = parse_agent_md(&file).unwrap();
        assert_eq!(agent.commands, CommandAccess::ReadWrite);
        assert!(agent.allowed_tools.is_empty());
    }

    #[test]
    fn a_subagent_round_trips() {
        let draft = AgentDraft {
            subagent: true,
            ..draft()
        };
        let agent = parse_agent_md(&draft.render()).unwrap();
        assert!(agent.subagent);
        // A sub-agent is out of the composer's menu, so it keeps no phase.
        assert_eq!(agent.phase, draft.phase);
    }

    #[test]
    fn a_colon_in_a_value_stays_in_the_value() {
        let draft = AgentDraft {
            description: "Fait ceci: puis cela".into(),
            name: "L'agent".into(),
            ..draft()
        };
        let agent = parse_agent_md(&draft.render()).unwrap();
        assert_eq!(agent.description, "Fait ceci: puis cela");
        assert_eq!(agent.name, "L'agent");
    }
}
