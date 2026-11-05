//! The `suggest_agent` tool, owned by the loop and fennec's own: the agent suggests the human go on in a new session with another agent, when that agent serves the next step better. It switches nothing and does not pause the turn; the human decides from a card.

use crate::domain::agent::AgentDefinition;
use crate::domain::event::Event;
use serde_json::{Value, json};

pub const NAME: &str = "suggest_agent";

/// What the model reads back once the suggestion is in the log.
pub const SUGGESTED: &str =
    "Suggestion shown to the human; a new session starts on that agent only if they choose it.";

/// The OpenAI tool definition.
pub fn definition(agents: &[AgentDefinition]) -> Value {
    let non_sub_agents: Vec<&AgentDefinition> = agents.iter().filter(|a| !a.subagent).collect();
    let enum_values: Vec<String> = non_sub_agents.iter().map(|a| a.id.clone()).collect();
    let agent_list = non_sub_agents
        .iter()
        .map(|a| format!("{}: {}", a.id, a.description))
        .collect::<Vec<_>>()
        .join("; ");

    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": format!(
                "Suggest the human go on in a new session with another agent, when the next step is that agent's job rather than yours (for example: framing a change, building it, auditing it). Say why and write the brief that opens the new session. The human decides; nothing switches by itself and the work does not pause. Agents: {agent_list}"
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "agent": {
                        "type": "string",
                        "enum": enum_values,
                        "description": "Id of the agent to go on with",
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why this agent, for this next step, in one or two sentences",
                    },
                    "brief": {
                        "type": "string",
                        "description": "The first message of the new session: what to do, with the context it needs, self-contained",
                    },
                },
                "required": ["agent", "reason", "brief"],
            },
        },
    })
}

pub struct Suggestion {
    pub agent: String,
    pub reason: String,
    pub brief: String,
}

fn required(args: &Value, field: &str) -> Result<String, String> {
    args.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("suggest_agent requires a non-empty '{field}' string."))
}

/// Validates the model's `suggest_agent` arguments. The error is a result
/// the model reads, so it must say what to fix.
pub fn parse(
    args: &Value,
    agents: &[AgentDefinition],
    current: &str,
) -> Result<Suggestion, String> {
    let agent = required(args, "agent")?;
    let reason = required(args, "reason")?;
    let brief = required(args, "brief")?;

    let available: Vec<&str> = agents
        .iter()
        .filter(|a| !a.subagent)
        .map(|a| a.id.as_str())
        .collect();

    if !agents.iter().any(|a| a.id == agent && !a.subagent) {
        return Err(format!(
            "Unknown agent '{agent}'. Available: {}",
            available.join(", ")
        ));
    }

    if agent == current {
        return Err(format!(
            "You are already '{current}': go on yourself instead of suggesting it."
        ));
    }

    if brief.chars().count() > 4000 {
        return Err("'brief' must be at most 4000 characters.".to_string());
    }

    Ok(Suggestion {
        agent,
        reason,
        brief,
    })
}

pub fn event(call_id: &str, s: Suggestion) -> Event {
    Event::AgentSuggested {
        call_id: call_id.into(),
        agent: s.agent,
        reason: s.reason,
        brief: s.brief,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::{AgentOrigin, CommandAccess};

    fn make_agent(id: &str, name: &str, description: &str, subagent: bool) -> AgentDefinition {
        AgentDefinition {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            subagent,
            composer: true,
            allowed_tools: Vec::new(),
            commands: CommandAccess::ReadOnly,
            writes: crate::domain::agent::WriteScope::Any,
            reasoning_effort: None,
            color: None,
            phase: None,
            workflow: None,
            prompt: "prompt".into(),
            origin: AgentOrigin::Default,
            path: None,
        }
    }

    fn agents() -> Vec<AgentDefinition> {
        vec![
            make_agent("plan", "Plan", "Plan", false),
            make_agent("builder", "Builder", "Build", false),
            make_agent("audit", "Audit", "Audit", true),
        ]
    }

    #[test]
    fn parse_trims_fields() {
        let s = parse(
            &json!({
                "agent": " builder ",
                "reason": " Why ",
                "brief": " Brief "
            }),
            &agents(),
            "plan",
        )
        .unwrap();
        assert_eq!(s.agent, "builder");
        assert_eq!(s.reason, "Why");
        assert_eq!(s.brief, "Brief");
    }

    #[test]
    fn parse_unknown_agent_lists_available_ids_and_excludes_subagents() {
        let err = parse(
            &json!({
                "agent": "nope",
                "reason": "r",
                "brief": "b"
            }),
            &agents(),
            "plan",
        )
        .err()
        .unwrap();
        assert!(err.contains("Available: plan, builder"));
        assert!(!err.contains("audit"));
    }

    #[test]
    fn parse_refuses_a_subagent() {
        let err = parse(
            &json!({
                "agent": "audit",
                "reason": "r",
                "brief": "b"
            }),
            &agents(),
            "plan",
        )
        .err()
        .unwrap();
        assert!(err.contains("Unknown agent 'audit'"));
    }

    #[test]
    fn parse_refuses_the_current_agent() {
        let err = parse(
            &json!({
                "agent": "plan",
                "reason": "r",
                "brief": "b"
            }),
            &agents(),
            "plan",
        )
        .err()
        .unwrap();
        assert!(err.contains("already 'plan'"));
    }

    #[test]
    fn parse_names_the_missing_field() {
        let err = parse(
            &json!({
                "agent": "builder",
                "brief": "b"
            }),
            &agents(),
            "plan",
        )
        .err()
        .unwrap();
        assert!(err.contains("'reason'"));
    }

    #[test]
    fn definition_enum_lists_only_non_subagents() {
        let def = definition(&agents());
        let enum_values = def["function"]["parameters"]["properties"]["agent"]["enum"]
            .as_array()
            .unwrap();
        let ids: Vec<&str> = enum_values.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(ids, vec!["plan", "builder"]);
    }
}
