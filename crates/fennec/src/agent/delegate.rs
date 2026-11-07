//! The `delegate` tool, owned by the agent loop: it hands a self-contained, read-only task to an agent in a fresh child session and returns only that agent's final answer.
//! The child has its own context, so broad exploration does not fill the parent's context; it cannot write, ask the human, or delegate further, so delegations do not nest.

use crate::agent::ports::AgentRun;
use crate::domain::agent::AgentDefinition;
use crate::domain::event::{DoneReason, ToolResult};
use serde_json::{Value, json};

use super::tool_args::required;
use super::turn::Turn;

pub const NAME: &str = "delegate";

/// The OpenAI tool definition; the `agent` enum lists the delegable agents (`a.delegable()`), in the given order.
pub fn definition(agents: &[AgentDefinition]) -> Value {
    let delegable: Vec<&AgentDefinition> = agents.iter().filter(|a| a.delegable()).collect();
    let enum_values: Vec<String> = delegable.iter().map(|a| a.id.clone()).collect();
    let agent_list = delegable
        .iter()
        .map(|a| format!("{}: {}", a.id, a.description))
        .collect::<Vec<_>>()
        .join("; ");

    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": format!(
                "Delegate a self-contained, read-only task (exploring the code, finding where something is done, an audit of a part) to an agent that runs in a fresh context and returns only its final answer; use it when the work would read many files you do not need to keep; the agent sees nothing of this conversation, so the task must carry every path, name and question it needs; it cannot ask the human anything, change files or delegate further; one delegation at a time. Agents: {agent_list}"
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "agent": {
                        "type": "string",
                        "enum": enum_values,
                        "description": "Id of the delegable agent",
                    },
                    "task": {
                        "type": "string",
                        "description": "The self-contained task: every path, name and question the agent needs",
                    },
                },
                "required": ["agent", "task"],
            },
        },
    })
}

#[derive(Debug)]
pub struct Delegation {
    pub agent: String,
    pub task: String,
}

/// Validates the arguments. Errors are tool results the model reads: say what to fix.
pub fn parse(args: &Value, agents: &[AgentDefinition]) -> Result<Delegation, String> {
    let agent = required(NAME, args, "agent")?;
    let task = required(NAME, args, "task")?;

    let delegable_ids: Vec<&str> = agents
        .iter()
        .filter(|a| a.delegable())
        .map(|a| a.id.as_str())
        .collect();

    if !delegable_ids.contains(&agent.as_str()) {
        let list = if delegable_ids.is_empty() {
            "none".to_string()
        } else {
            delegable_ids.join(", ")
        };
        return Err(format!(
            "Agent '{agent}' cannot be delegated to. Delegable: {list}"
        ));
    }

    if task.chars().count() > 8000 {
        return Err("Task must be at most 8000 characters.".to_string());
    }

    Ok(Delegation { agent, task })
}

/// The child session's title: "<agent id>: <task's first line, cut at 60 chars with '…' when longer>".
pub fn child_title(d: &Delegation) -> String {
    let line = d.task.lines().next().unwrap_or("").trim();
    let summary = if line.chars().count() > 60 {
        let mut cut: String = line.chars().take(60).collect();
        cut.push('…');
        cut
    } else {
        line.to_string()
    };

    format!("{}: {}", d.agent, summary)
}

/// The tool result from the child's run.
pub fn result(child_session_id: &str, run: AgentRun) -> ToolResult {
    let reason = run.reason;
    let mut text = run.text.trim().to_string();

    if text.is_empty() {
        text = "(the agent gave no answer)".to_string();
    }

    let truncated = if text.chars().count() > 20_000 {
        let cut: String = text.chars().take(20_000).collect();
        text = format!("{cut}\n\n[answer cut at 20000 characters]");
        true
    } else {
        false
    };

    let prefix = match reason {
        DoneReason::Complete => "",
        DoneReason::Stopped => "The human stopped the delegated agent. Partial answer:\n\n",
        DoneReason::Error => "The delegated agent failed before finishing. Partial answer:\n\n",
        DoneReason::Truncated => "The delegated agent's answer was truncated:\n\n",
        DoneReason::WaitingForUser => {
            "The delegated agent ended on a question it could not ask:\n\n"
        }
    };

    let output = format!("{prefix}{text}\n\n(Delegated session: {child_session_id})");
    ToolResult::success(output, truncated)
}

/// Runs the delegation in the turn: called by the loop's tool dispatcher.
pub(super) fn run(turn: &Turn, args: &Value) -> ToolResult {
    let Some(children) = turn.children else {
        return ToolResult::failure(
            "delegate is not available here: only a session's top-level agent in the web app can delegate."
                .into(),
        );
    };
    let delegation = match parse(args, turn.agents) {
        Ok(delegation) => delegation,
        Err(e) => return ToolResult::failure(e),
    };
    // The child is another session: the model gate must not keep it waiting
    // behind its own parent, which is blocked on this call.
    turn.model.release(turn.session_id);
    let ran = children
        .create(turn.session_id, &child_title(&delegation))
        .and_then(|child| {
            children
                .send(&child, &delegation.agent, &delegation.task, turn.stop)
                .map(|run| (child, run))
        });
    match ran {
        Ok((child, run)) => result(&child, run),
        Err(e) => ToolResult::failure(format!("delegate failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::{AgentOrigin, CommandAccess, WriteScope};

    fn agent(id: &str, commands: CommandAccess, tools: &[&str]) -> AgentDefinition {
        AgentDefinition {
            id: id.into(),
            name: id.into(),
            description: format!("{id} agent"),
            subagent: false,
            composer: true,
            allowed_tools: tools.iter().map(|t| t.to_string()).collect(),
            commands,
            writes: WriteScope::Any,
            reasoning_effort: None,
            color: None,
            phase: None,
            workflow: None,
            prompt: "prompt".into(),
            origin: AgentOrigin::Default,
            path: None,
        }
    }

    #[test]
    fn definition_lists_only_delegable_agents() {
        let reader = agent("reader", CommandAccess::ReadOnly, &["read_file"]);
        let writer = agent(
            "writer",
            CommandAccess::ReadWrite,
            &["read_file", "write_file"],
        );

        let definition = definition(&[reader, writer]);
        let enum_values = definition["function"]["parameters"]["properties"]["agent"]["enum"]
            .as_array()
            .unwrap();

        assert_eq!(enum_values, &[Value::String("reader".to_string())]);
    }

    #[test]
    fn parse_trims_fields() {
        let reader = agent("reader", CommandAccess::ReadOnly, &["read_file"]);
        let args = json!({
            "agent": " reader ",
            "task": "  look at src/main.rs  "
        });

        let d = parse(&args, &[reader]).unwrap();

        assert_eq!(d.agent, "reader");
        assert_eq!(d.task, "look at src/main.rs");
    }

    #[test]
    fn parse_refuses_non_delegable_agent_and_lists_delegable_ones() {
        let reader = agent("reader", CommandAccess::ReadOnly, &["read_file"]);
        let writer = agent(
            "writer",
            CommandAccess::ReadWrite,
            &["read_file", "write_file"],
        );
        let args = json!({
            "agent": "writer",
            "task": "do something"
        });

        let err = parse(&args, &[reader, writer]).unwrap_err();

        assert!(err.contains("Agent 'writer' cannot be delegated to"));
        assert!(err.contains("Delegable: reader"));
    }

    #[test]
    fn parse_names_missing_field() {
        let reader = agent("reader", CommandAccess::ReadOnly, &["read_file"]);
        let args = json!({ "agent": "reader" });

        let err = parse(&args, &[reader]).unwrap_err();

        assert!(err.contains("task"));
        assert!(err.contains("delegate"));
    }

    #[test]
    fn parse_rejects_task_that_is_too_long() {
        let reader = agent("reader", CommandAccess::ReadOnly, &["read_file"]);
        let task = "a".repeat(8001);
        let args = json!({
            "agent": "reader",
            "task": task
        });

        let err = parse(&args, &[reader]).unwrap_err();

        assert!(err.contains("8000"));
    }

    #[test]
    fn child_title_uses_first_line_and_cuts_at_sixty_chars() {
        let long_line = "x".repeat(61);
        let d = Delegation {
            agent: "reader".to_string(),
            task: format!("{long_line}\nsecond line"),
        };

        assert_eq!(child_title(&d), format!("reader: {}…", "x".repeat(60)));

        let exact_line = "y".repeat(60);
        let d = Delegation {
            agent: "reader".to_string(),
            task: exact_line.clone(),
        };

        assert_eq!(child_title(&d), format!("reader: {exact_line}"));
    }

    #[test]
    fn result_complete_is_plain_with_session_line() {
        let r = result(
            "session-1",
            AgentRun {
                reason: DoneReason::Complete,
                text: "answer".to_string(),
            },
        );

        assert!(r.success);
        assert!(!r.truncated);
        assert_eq!(
            r.output.as_deref(),
            Some("answer\n\n(Delegated session: session-1)")
        );
    }

    #[test]
    fn result_stopped_adds_prefix() {
        let r = result(
            "session-1",
            AgentRun {
                reason: DoneReason::Stopped,
                text: "partial".to_string(),
            },
        );

        assert!(r.success);
        assert_eq!(
            r.output.as_deref(),
            Some(
                "The human stopped the delegated agent. Partial answer:\n\npartial\n\n(Delegated session: session-1)"
            )
        );
    }

    #[test]
    fn result_empty_text_has_placeholder() {
        let r = result(
            "session-1",
            AgentRun {
                reason: DoneReason::Complete,
                text: "   ".to_string(),
            },
        );

        assert_eq!(
            r.output.as_deref(),
            Some("(the agent gave no answer)\n\n(Delegated session: session-1)")
        );
    }

    #[test]
    fn result_caps_at_twenty_thousand_chars_and_marks_truncated() {
        let big = "a".repeat(20_001);
        let r = result(
            "session-1",
            AgentRun {
                reason: DoneReason::Complete,
                text: big,
            },
        );

        assert!(r.truncated);
        let output = r.output.as_deref().unwrap();
        assert!(output.starts_with(&"a".repeat(20_000)));
        assert!(output.contains("[answer cut at 20000 characters]"));
        assert!(output.ends_with("(Delegated session: session-1)"));
    }
}
