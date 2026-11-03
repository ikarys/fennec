//! Agent definitions: the `*.agent.md` file parsed, the reminder and its tool
//! permissions section, the permission error, and the mode fold.
//!
//! This module does not do registry discovery/CRUD (see `crate::agents`),
//! config `agents` dirs, granular `tool:action` permissions, MCP rules,
//! `results`, sub-agent `return_value`/`step_done` extras, small reminders,
//! per-agent model overrides, or project/DB default agent setting.

use super::compaction::model_view;
use super::event::{Event, MessageMetadata, MetadataSource};
use super::frontmatter::split;
use super::preferences::ReasoningEffort;

/// What the commands of an agent may do to the working directory: the
/// plan reads only, by construction (its commands see the workdir
/// read-only), while verify and review run tests that write build output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandAccess {
    #[default]
    ReadWrite,
    ReadOnly,
}

/// fennec's own: `phase` in the frontmatter, the step of a project an agent serves; the composer groups its menu by it, in this order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Start,
    Frame,
    Build,
    Understand,
    Check,
}

/// fennec's own: where an agent was found, which decides who may edit it.
/// A default is embedded in the binary, a user agent lives in
/// `~/.agents/agents` and only the human's CRUD writes it, a project agent
/// lives in the repository and belongs to the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentOrigin {
    Default,
    User,
    Project,
}

impl AgentOrigin {
    /// Whether the human's agent CRUD may write or delete this agent.
    pub fn editable(self) -> bool {
        self == Self::User
    }
}

/// `AgentDefinition` (metadata flattened).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub subagent: bool,
    pub allowed_tools: Vec<String>,
    /// fennec's own: `commands: read-only` in the frontmatter.
    pub commands: CommandAccess,
    /// fennec's own: `reasoningEffort` in the frontmatter; overrides the model's default effort for this agent's rounds.
    pub reasoning_effort: Option<ReasoningEffort>,
    pub color: Option<String>,
    /// None for sub-agents and agents that name no known phase.
    pub phase: Option<Phase>,
    pub prompt: String,
    /// fennec's own: where this definition was found.
    pub origin: AgentOrigin,
    /// The file it was read from; None for an embedded default.
    pub path: Option<String>,
}

/// fennec's own: the rules every read-only agent shares (never write, source
/// every claim as `path:line`, explore broad to narrow). Kept in one place so
/// the agents that analyse, answer and audit cannot drift apart on them; the
/// agent's own prompt follows it and refines it.
const READ_ONLY_PREAMBLE: &str = include_str!("../../prompts/read-only.md");

/// Every tool an agent may list in `allowedTools`: the toolbox's tools (see
/// `crate::tools`) and the loop's own ones (`crate::agent`). The human's agent
/// CRUD refuses anything else, so a typo cannot silently disable a tool.
pub const KNOWN_TOOLS: [&str; 13] = [
    "read_file",
    "write_file",
    "edit_file",
    "run_command",
    "load_skill",
    "web_search",
    "web_fetch",
    "ask_user",
    "record_decision",
    "propose_criteria",
    "propose_task",
    "request_waiver",
    "suggest_agent",
];

/// The default agent: a new session plans first,
/// nothing is written before the human switches to the builder.
pub const DEFAULT_AGENT: &str = "plan";
/// `color ?? '#6b7280'` for the reminder metadata.
pub const DEFAULT_COLOR: &str = "#6b7280";

/// Why a `*.agent.md` file is not an agent. Discovery ignores a bad file, but
/// the human's CRUD must say which field is at fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentParseError {
    /// No `---` frontmatter block, or it is never closed.
    MissingFrontmatter,
    /// No `id:`, or an empty one: nothing to name the agent by.
    MissingId,
    /// Frontmatter but no prompt under it.
    EmptyBody,
}

impl AgentParseError {
    /// The frontmatter field or document part at fault, as the API reports it.
    pub fn field(self) -> &'static str {
        match self {
            Self::MissingFrontmatter => "frontmatter",
            Self::MissingId => "id",
            Self::EmptyBody => "prompt",
        }
    }
}

impl std::fmt::Display for AgentParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::MissingFrontmatter => "no frontmatter: the file must start with a `---` block",
            Self::MissingId => "the frontmatter needs a non-empty `id`",
            Self::EmptyBody => "the prompt under the frontmatter is empty",
        };
        f.write_str(message)
    }
}

pub fn parse_agent_md(content: &str) -> Result<AgentDefinition, AgentParseError> {
    let (fields, body) = split(content).ok_or(AgentParseError::MissingFrontmatter)?;
    if body.is_empty() {
        return Err(AgentParseError::EmptyBody);
    }

    let id = fields
        .scalar("id")
        .filter(|value| !value.is_empty())
        .ok_or(AgentParseError::MissingId)?;
    let name = fields
        .scalar("name")
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let description = fields.scalar("description").unwrap_or("");
    let subagent = fields.scalar("subagent") == Some("true");
    let allowed_tools = fields
        .list("allowedTools")
        .map(|tools| tools.to_vec())
        .unwrap_or_default();
    let commands = if fields.scalar("commands") == Some("read-only") {
        CommandAccess::ReadOnly
    } else {
        CommandAccess::ReadWrite
    };
    let reasoning_effort = fields.scalar("reasoningEffort").and_then(|value| {
        serde_json::from_value::<ReasoningEffort>(serde_json::Value::String(value.to_string())).ok()
    });
    let color = fields
        .scalar("color")
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let phase = fields.scalar("phase").and_then(|value| {
        serde_json::from_value::<Phase>(serde_json::Value::String(value.to_string())).ok()
    });

    Ok(AgentDefinition {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        subagent,
        allowed_tools,
        commands,
        reasoning_effort,
        color,
        phase,
        prompt: body,
        // The file's content cannot say where the file is: discovery sets it.
        origin: AgentOrigin::Default,
        path: None,
    })
}

impl AgentDefinition {
    /// Whether a call to `tool` may run in this mode. All tool definitions
    /// are always sent (the prompt prefix stays cached across modes); the
    /// mode is enforced here, at execution.
    pub fn allows(&self, tool: &str) -> bool {
        self.allowed_tools.iter().any(|name| name == tool)
    }

    /// The reminder: `<system-reminder>\n{prompt}{tools}\n</system-reminder>`
    /// where `{tools}` is `"\n\n## AVAILABLE TOOLS\n\nYou have no tools available."`
    /// when `allowed_tools` is empty, else `"\n\n## AVAILABLE TOOLS\n\nYou have access to
    /// these tools: a, b, c"` (file order).
    pub fn reminder(&self) -> String {
        let tools = if self.allowed_tools.is_empty() {
            "\n\n## AVAILABLE TOOLS\n\nYou have no tools available.".to_string()
        } else {
            format!(
                "\n\n## AVAILABLE TOOLS\n\nYou have access to these tools: {}",
                self.allowed_tools.join(", ")
            )
        };

        format!(
            "<system-reminder>\n{}{}\n</system-reminder>",
            self.body(),
            tools
        )
    }

    /// The prompt the reminder carries: a read-only agent gets the shared
    /// preamble first, then its own prompt, which refines it.
    fn body(&self) -> String {
        if self.commands == CommandAccess::ReadOnly {
            format!("{}\n{}", READ_ONLY_PREAMBLE.trim(), self.prompt)
        } else {
            self.prompt.clone()
        }
    }

    /// The permission error with an agent id:
    /// `"Tool '{tool}' is not available in '{id}' mode. Available: {sorted allowed tools
    /// joined by ', '}"`.
    pub fn denied(&self, tool: &str) -> String {
        let mut available = self.allowed_tools.clone();
        available.sort();

        format!(
            "Tool '{tool}' is not available in '{}' mode. Available: {}",
            self.id,
            available.join(", ")
        )
    }
}

/// The mode of the last `mode.changed`, None if the human never chose one.
pub fn chosen_mode(events: &[Event]) -> Option<&str> {
    events.iter().rev().find_map(|event| match event {
        Event::ModeChanged { mode, .. } => Some(mode.as_str()),
        _ => None,
    })
}

/// The top-level agent a turn runs as: the chosen mode when it names a known
/// top-level (non-subagent) agent, else DEFAULT_AGENT, else the first top-level agent.
/// None only when there is no top-level agent at all.
pub fn resolve<'a>(agents: &'a [AgentDefinition], events: &[Event]) -> Option<&'a AgentDefinition> {
    resolve_mode(agents, chosen_mode(events))
}

/// The top-level agent for a chosen mode: that agent when it is a known
/// top-level one, else DEFAULT_AGENT, else the first top-level agent. None
/// only when there is no top-level agent at all.
pub fn resolve_mode<'a>(
    agents: &'a [AgentDefinition],
    mode: Option<&str>,
) -> Option<&'a AgentDefinition> {
    let top_level = || agents.iter().filter(|agent| !agent.subagent);
    mode.and_then(|mode| top_level().find(|agent| agent.id == mode))
        .or_else(|| top_level().find(|agent| agent.id == DEFAULT_AGENT))
        .or_else(|| top_level().next())
}

/// The event that switches a session to `requested`, None when it already
/// runs as that agent. Only a top-level agent can be chosen: a sub-agent or
/// an unknown id is refused with "unknown mode: {requested}".
pub fn switch_to(
    agents: &[AgentDefinition],
    events: &[Event],
    requested: &str,
) -> Result<Option<Event>, String> {
    if !agents
        .iter()
        .any(|agent| !agent.subagent && agent.id == requested)
    {
        return Err(format!("unknown mode: {requested}"));
    }

    let already = resolve(agents, events).is_some_and(|agent| agent.id == requested);
    Ok(if already {
        None
    } else {
        Some(Event::ModeChanged {
            mode: requested.to_string(),
            auto: false,
            reason: None,
        })
    })
}

/// The `name` of the last agent reminder in the
/// model's current view (`compaction::model_view`), None when the window has none.
pub fn last_reminder(events: &[Event]) -> Option<&str> {
    model_view(events)
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::MessageStart {
                is_system_generated: true,
                metadata:
                    Some(MessageMetadata {
                        source: MetadataSource::Agent,
                        name,
                        ..
                    }),
                ..
            } => Some(name.as_str()),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{ReminderKind, Role};

    fn agent(id: &str, name: &str, subagent: bool, tools: &[&str]) -> AgentDefinition {
        AgentDefinition {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            subagent,
            allowed_tools: tools.iter().map(|tool| tool.to_string()).collect(),
            commands: CommandAccess::ReadWrite,
            reasoning_effort: None,
            color: None,
            phase: None,
            prompt: "prompt".to_string(),
            origin: AgentOrigin::Default,
            path: None,
        }
    }

    fn reminder(name: &str, kind: ReminderKind) -> Event {
        Event::MessageStart {
            message_id: "m".to_string(),
            role: Role::User,
            content: Some("reminder".to_string()),
            is_system_generated: true,
            metadata: Some(MessageMetadata {
                source: MetadataSource::Agent,
                name: name.to_string(),
                color: "#000000".to_string(),
                kind,
            }),
            attachments: Vec::new(),
        }
    }

    fn assert_mode_event(event: &Event, mode: &str) {
        match event {
            Event::ModeChanged {
                mode: actual,
                auto,
                reason,
            } => {
                assert_eq!(actual, mode);
                assert!(!*auto);
                assert!(reason.is_none());
            }
            other => panic!("expected ModeChanged, got {other:?}"),
        }
    }

    #[test]
    fn parses_full_agent_file() {
        let content = "\
---
id: plan
name: Plan
description: Plans
subagent: false
color: '#a855f7'
allowedTools:
  - read_file
  - ask_user
---

# Plan
";
        let agent = parse_agent_md(content).unwrap();
        assert_eq!(agent.id, "plan");
        assert_eq!(agent.name, "Plan");
        assert_eq!(agent.description, "Plans");
        assert!(!agent.subagent);
        assert_eq!(agent.allowed_tools, vec!["read_file", "ask_user"]);
        assert_eq!(agent.color.as_deref(), Some("#a855f7"));
        assert_eq!(agent.prompt, "# Plan");
    }

    #[test]
    fn name_defaults_to_id() {
        let content = "---\nid: plan\n---\nbody\n";
        let agent = parse_agent_md(content).unwrap();
        assert_eq!(agent.id, "plan");
        assert_eq!(agent.name, "plan");
    }

    #[test]
    fn missing_id_is_none() {
        assert_eq!(
            parse_agent_md("---\nname: Plan\n---\nbody\n"),
            Err(AgentParseError::MissingId)
        );
    }

    #[test]
    fn empty_body_is_none() {
        assert_eq!(
            parse_agent_md("---\nid: plan\n---\n"),
            Err(AgentParseError::EmptyBody)
        );
    }

    #[test]
    fn phase_is_parsed() {
        let content = "---\nid: plan\nphase: understand\n---\nbody\n";
        let agent = parse_agent_md(content).unwrap();
        assert_eq!(agent.phase, Some(Phase::Understand));
    }

    #[test]
    fn unknown_or_missing_phase_is_none() {
        let unknown = parse_agent_md("---\nid: plan\nphase: later\n---\nbody\n").unwrap();
        assert_eq!(unknown.phase, None);

        let missing = parse_agent_md("---\nid: plan\n---\nbody\n").unwrap();
        assert_eq!(missing.phase, None);
    }

    #[test]
    fn subagent_yes_is_false() {
        let content = "---\nid: plan\nsubagent: yes\n---\nbody\n";
        let agent = parse_agent_md(content).unwrap();
        assert!(!agent.subagent);
    }

    #[test]
    fn parses_inline_allowed_tools() {
        let content = "---\nid: plan\nallowedTools: [read_file, ask_user]\n---\nbody\n";
        let agent = parse_agent_md(content).unwrap();
        assert_eq!(agent.allowed_tools, vec!["read_file", "ask_user"]);
    }

    #[test]
    fn parses_block_allowed_tools() {
        let content = "\
---
id: plan
allowedTools:
  - read_file
  - ask_user
---
body
";
        let agent = parse_agent_md(content).unwrap();
        assert_eq!(agent.allowed_tools, vec!["read_file", "ask_user"]);
    }

    #[test]
    fn parses_read_only_commands() {
        let read_only = parse_agent_md("---\nid: plan\ncommands: read-only\n---\nbody\n").unwrap();
        assert_eq!(read_only.commands, CommandAccess::ReadOnly);

        let read_write = parse_agent_md("---\nid: plan\n---\nbody\n").unwrap();
        assert_eq!(read_write.commands, CommandAccess::ReadWrite);
    }

    #[test]
    fn parses_reasoning_effort() {
        let xhigh = parse_agent_md("---\nid: plan\nreasoningEffort: xhigh\n---\nbody\n").unwrap();
        assert_eq!(xhigh.reasoning_effort, Some(ReasoningEffort::XHigh));

        let absent = parse_agent_md("---\nid: plan\n---\nbody\n").unwrap();
        assert_eq!(absent.reasoning_effort, None);

        let bogus = parse_agent_md("---\nid: plan\nreasoningEffort: bogus\n---\nbody\n").unwrap();
        assert_eq!(bogus.reasoning_effort, None);
    }

    #[test]
    fn a_read_only_agent_gets_the_shared_preamble_before_its_own_prompt() {
        let mut agent = agent("ask", "Question", false, &["read_file"]);
        agent.commands = CommandAccess::ReadOnly;
        agent.prompt = "Answer.".to_string();

        let reminder = agent.reminder();
        assert!(reminder.contains("## Read-only agent"), "{reminder}");
        assert!(reminder.contains("never write a file"), "{reminder}");
        assert!(reminder.contains("`<path>`:`<line>`") || reminder.contains("<path>:<line>"));
        // The agent's own prompt follows the preamble, so it can refine it.
        let preamble = reminder.find("## Read-only agent").unwrap();
        let own = reminder.find("Answer.").unwrap();
        assert!(preamble < own);
    }

    #[test]
    fn a_read_write_agent_gets_only_its_own_prompt() {
        let mut agent = agent("builder", "Builder", false, &["write_file"]);
        agent.prompt = "Build.".to_string();

        let reminder = agent.reminder();
        assert!(!reminder.contains("## Read-only agent"), "{reminder}");
        assert!(reminder.contains("Build."));
    }

    #[test]
    fn allows_only_listed_tools() {
        let agent = agent("plan", "Plan", false, &["read_file"]);
        assert!(agent.allows("read_file"));
        assert!(!agent.allows("write_file"));
    }

    #[test]
    fn denied_message_is_exact_and_sorted() {
        let agent = agent("plan", "Plan", false, &["b", "a"]);
        assert_eq!(
            agent.denied("write_file"),
            "Tool 'write_file' is not available in 'plan' mode. Available: a, b"
        );
    }

    #[test]
    fn reminder_is_exact_without_tools() {
        let mut agent = agent("plan", "Plan", false, &[]);
        agent.prompt = "Plan.".to_string();
        assert_eq!(
            agent.reminder(),
            "<system-reminder>\nPlan.\n\n## AVAILABLE TOOLS\n\nYou have no tools available.\n</system-reminder>"
        );
    }

    #[test]
    fn reminder_is_exact_with_tools_in_file_order() {
        let mut agent = agent("builder", "Builder", false, &["write_file", "read_file"]);
        agent.prompt = "Build.".to_string();
        assert_eq!(
            agent.reminder(),
            "<system-reminder>\nBuild.\n\n## AVAILABLE TOOLS\n\nYou have access to these tools: write_file, read_file\n</system-reminder>"
        );
    }

    #[test]
    fn chosen_mode_last_wins() {
        let events = vec![
            Event::ModeChanged {
                mode: "builder".to_string(),
                auto: false,
                reason: None,
            },
            Event::ModeChanged {
                mode: "plan".to_string(),
                auto: false,
                reason: None,
            },
        ];
        assert_eq!(chosen_mode(&events), Some("plan"));
    }

    #[test]
    fn chosen_mode_none_without_mode_changed() {
        assert_eq!(chosen_mode(&[]), None);
    }

    #[test]
    fn resolve_uses_chosen_top_level_agent() {
        let agents = vec![
            agent("plan", "Plan", false, &[]),
            agent("builder", "Builder", false, &[]),
        ];
        let events = vec![Event::ModeChanged {
            mode: "builder".to_string(),
            auto: false,
            reason: None,
        }];
        assert_eq!(
            resolve(&agents, &events).map(|agent| agent.id.as_str()),
            Some("builder")
        );
    }

    #[test]
    fn resolve_falls_back_to_default_for_unknown_mode() {
        let agents = vec![
            agent("plan", "Plan", false, &[]),
            agent("builder", "Builder", false, &[]),
        ];
        let events = vec![Event::ModeChanged {
            mode: "missing".to_string(),
            auto: false,
            reason: None,
        }];
        assert_eq!(
            resolve(&agents, &events).map(|agent| agent.id.as_str()),
            Some("plan")
        );
    }

    #[test]
    fn resolve_falls_back_to_default_for_subagent_mode() {
        let agents = vec![
            agent("plan", "Plan", false, &[]),
            agent("verifier", "Verifier", true, &[]),
        ];
        let events = vec![Event::ModeChanged {
            mode: "verifier".to_string(),
            auto: false,
            reason: None,
        }];
        assert_eq!(
            resolve(&agents, &events).map(|agent| agent.id.as_str()),
            Some("plan")
        );
    }

    #[test]
    fn resolve_uses_first_top_level_when_default_is_missing() {
        let agents = vec![
            agent("builder", "Builder", false, &[]),
            agent("verifier", "Verifier", true, &[]),
        ];
        assert_eq!(
            resolve(&agents, &[]).map(|agent| agent.id.as_str()),
            Some("builder")
        );
    }

    #[test]
    fn resolve_is_none_without_top_level_agents() {
        let agents = vec![agent("verifier", "Verifier", true, &[])];
        assert_eq!(resolve(&agents, &[]), None);
    }

    #[test]
    fn last_reminder_latest_wins() {
        let events = vec![
            reminder("Plan", ReminderKind::Definition),
            reminder("Builder", ReminderKind::Reminder),
        ];
        assert_eq!(last_reminder(&events), Some("Builder"));
    }

    #[test]
    fn last_reminder_is_hidden_by_a_rewind() {
        let events = vec![
            reminder("Plan", ReminderKind::Definition),
            Event::SessionRewound {
                message_id: "m".into(),
            },
        ];
        assert_eq!(last_reminder(&events), None);
    }

    #[test]
    fn last_reminder_is_hidden_by_later_compaction() {
        let events = vec![
            reminder("Plan", ReminderKind::Definition),
            Event::ContextCompacted {
                before_tokens: 1,
                after_tokens: 2,
                summary: "summary".to_string(),
            },
        ];
        assert_eq!(last_reminder(&events), None);
    }

    #[test]
    fn switch_to_returns_an_event_for_a_new_top_level_agent() {
        let agents = vec![
            agent("plan", "Plan", false, &[]),
            agent("builder", "Builder", false, &[]),
        ];
        let event = switch_to(&agents, &[], "builder")
            .unwrap()
            .expect("a switch");
        assert_mode_event(&event, "builder");
    }

    #[test]
    fn switch_to_is_none_when_already_the_default() {
        let agents = vec![
            agent("plan", "Plan", false, &[]),
            agent("builder", "Builder", false, &[]),
        ];
        assert!(switch_to(&agents, &[], "plan").unwrap().is_none());
    }

    #[test]
    fn switch_to_is_none_after_an_explicit_mode_changed() {
        let agents = vec![
            agent("plan", "Plan", false, &[]),
            agent("builder", "Builder", false, &[]),
        ];
        let events = vec![Event::ModeChanged {
            mode: "plan".to_string(),
            auto: false,
            reason: None,
        }];
        assert!(switch_to(&agents, &events, "plan").unwrap().is_none());
    }

    #[test]
    fn switch_to_refuses_unknown_and_subagent_modes() {
        let agents = vec![
            agent("plan", "Plan", false, &[]),
            agent("verifier", "Verifier", true, &[]),
        ];
        assert_eq!(
            switch_to(&agents, &[], "missing"),
            Err("unknown mode: missing".to_string())
        );
        assert_eq!(
            switch_to(&agents, &[], "verifier"),
            Err("unknown mode: verifier".to_string())
        );
    }
}
