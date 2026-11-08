//! Runs the model's tool calls and records their results. `ask_user` is
//! special: it pauses the turn, so the calls after it in the batch are not
//! run, and the turn ends waiting for the human. `propose_plan` and
//! `submit_report` end the turn the same way: the plan or report is
//! recorded, the calls after it in the batch are not run, and the turn ends
//! complete.

use super::own_tools::{self, Handled};
use super::rule_check;
use super::turn::Turn;
use crate::agent::ports::RuleCheck;
use crate::domain::agent::{AgentDefinition, WriteScope};
use crate::domain::event::{Event, ToolCall, ToolResult};
use crate::domain::named_paths::{human_texts, named_by_human};
use crate::domain::test_files;
use std::sync::atomic::Ordering;

/// `INTERRUPTED_ERROR`: a call the human stopped before it ran.
pub(super) const INTERRUPTED_ERROR: &str = "Tool execution was interrupted by user";
/// A call queued after an `ask_user` that paused the turn.
const NOT_RUN_AFTER_ASK: &str = "Not run: the turn paused for the human (a question or an approval). Call it again after the answer if it is still needed.";

/// A default read ask can be skipped only when the human already named the path.
fn named_by_the_human(turn: &Turn, check: &RuleCheck) -> Result<bool, String> {
    let Some(path) = rule_check::named_read(check) else {
        return Ok(false);
    };

    let events = turn.log.events(turn.session_id)?;
    let texts = human_texts(&events);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);

    Ok(named_by_human(path, &texts, home.as_deref()))
}

pub(super) enum Flow {
    Continue,
    WaitingForUser,
    /// `propose_plan` recorded the plan, or `submit_report` the report:
    /// the turn ends complete.
    Done,
}

pub(super) fn record(
    turn: &mut Turn,
    message_id: &str,
    call: &ToolCall,
    result: ToolResult,
) -> Result<(), String> {
    turn.append(Event::ToolResult {
        message_id: message_id.to_string(),
        tool_call_id: call.id.clone(),
        result,
    })
}

/// Runs one call, streaming its output to the log; `approved` is the human's
/// "once" for a call fennec paused.
pub(super) fn run_tool(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    call: &ToolCall,
    approved: bool,
) -> Result<ToolResult, String> {
    let tools = turn.tools;
    let stop = turn.stop;
    let mut log_error = None;
    let mut on_output = |stream, chunk: &str| {
        if log_error.is_none() {
            log_error = turn
                .append(Event::ToolOutput {
                    message_id: message_id.into(),
                    tool_call_id: call.id.clone(),
                    stream,
                    content: chunk.into(),
                })
                .err();
        }
    };
    let (name, args, access) = (&call.name, &call.arguments, agent.commands);
    if let Some(refusal) = scope_refusal(agent, call) {
        return Ok(ToolResult::failure(refusal));
    }
    let result = if approved {
        tools.execute_approved(name, args, access, stop, &mut on_output)
    } else {
        tools.execute(name, args, access, stop, &mut on_output)
    };
    match log_error {
        Some(e) => Err(e),
        None => Ok(result),
    }
}

/// The refusal for a file write outside an agent's `writes` scope, if any.
fn scope_refusal(agent: &AgentDefinition, call: &ToolCall) -> Option<String> {
    if agent.writes != WriteScope::Tests {
        return None;
    }
    if !matches!(call.name.as_str(), "write_file" | "edit_file") {
        return None;
    }
    let path = call
        .arguments
        .get("path")
        .and_then(|value| value.as_str())?;
    let path = path.strip_prefix("./").unwrap_or(path);
    if test_files::is_test_path(path) {
        None
    } else {
        Some(format!(
            "{path} is not a test file: this agent writes only the tests; the code comes after the human validates them."
        ))
    }
}

/// A Toolbox call: the human's rules first (the model's call never reaches a refused tool), then the run.
fn run_checked(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    call: &ToolCall,
) -> Result<Handled, String> {
    let tools = turn.tools;
    let check = tools.check_rules(&call.name, &call.arguments);
    if !turn.autonomous
        && let rules::Decision::Ask(rule) = check.decision
    {
        if rule.is_none() && named_by_the_human(turn, &check)? {
            let r = run_tool(turn, agent, message_id, call, true)?;
            return Ok(Handled::Result(r));
        }

        turn.append(rule_check::approval(&call.id, rule, &check))?;
        return Ok(Handled::Waiting);
    }
    if let Some(rule) = rule_check::refusal(&check) {
        turn.append(rule_check::event(&call.id, rule, &check.subject))?;
        return Ok(Handled::Result(ToolResult::failure(rule.refusal())));
    }
    let r = run_tool(turn, agent, message_id, call, false)?;
    Ok(Handled::Result(r))
}

/// Runs the round's tool calls, sequential for now.
pub(super) fn execute_tools(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    calls: &[ToolCall],
) -> Result<Flow, String> {
    let mut waiting = false;
    let mut done = false;
    // MCP tools are a human-configured capability, not part of the agent's
    // fixed allowed list: allow them here so a `{server}_{tool}` call runs.
    let mcp_tools = turn.tools.mcp_tool_names();
    for call in calls {
        // The human pressed stop: a `write_file` queued after the stopped
        // command must not run, so record a failure without running it.
        if turn.stop.load(Ordering::Relaxed) {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(INTERRUPTED_ERROR.into()),
            )?;
            continue;
        }
        // An `ask_user` earlier in this batch paused the turn, or a
        // `propose_plan` / `submit_report` ended it.
        if waiting || done {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(NOT_RUN_AFTER_ASK.into()),
            )?;
            continue;
        }
        if let Some(pe) = &call.parse_error {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(format!(
                    "Failed to parse tool call arguments: {pe}. Please ensure your JSON function call arguments are valid."
                )),
            )?;
            continue;
        }
        // The agent's allow-list check: the mode is
        // enforced here, at execution, so plan stays read-only.
        if !agent.allows(&call.name) && !mcp_tools.contains(&call.name) {
            record(
                turn,
                message_id,
                call,
                ToolResult::failure(agent.denied(&call.name)),
            )?;
            continue;
        }
        let handled = match own_tools::handle(turn, agent, message_id, call)? {
            Some(handled) => handled,
            None => run_checked(turn, agent, message_id, call)?,
        };
        match handled {
            Handled::Result(result) => record(turn, message_id, call, result)?,
            Handled::Waiting => waiting = true,
            Handled::Done(result) => {
                record(turn, message_id, call, result)?;
                done = true;
            }
        }
    }
    Ok(if waiting {
        Flow::WaitingForUser
    } else if done {
        Flow::Done
    } else {
        Flow::Continue
    })
}

#[cfg(test)]
mod tests {
    use super::{Turn, execute_tools};
    use crate::agent::ports::{RuleCheck, Toolbox};
    use crate::agent::test_support::{MemoryLog, Scripted, builder, plan};
    use crate::domain::agent::{AgentDefinition, AgentOrigin, CommandAccess, WriteScope};
    use crate::domain::event::{Event, OutputStream, ToolCall, ToolResult};
    use rules::{Action, Origin, Request, Rule, Rules, Scope, Target, TargetKind};
    use serde_json::{Value, json};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::AtomicBool;

    /// A toolbox that offers one MCP tool and records every call it runs.
    struct McpToolbox {
        executed: Rc<RefCell<Vec<String>>>,
    }

    impl Toolbox for McpToolbox {
        fn check_rules(&self, _name: &str, _args: &Value) -> RuleCheck<'_> {
            RuleCheck::allowed()
        }
        fn definitions(&self) -> Value {
            json!([])
        }

        fn execute(
            &self,
            name: &str,
            _args: &Value,
            _access: CommandAccess,
            _stop: &AtomicBool,
            _on_output: &mut dyn FnMut(OutputStream, &str),
        ) -> ToolResult {
            self.executed.borrow_mut().push(name.to_string());
            ToolResult::success("mcp ran".into(), false)
        }

        fn mcp_tool_names(&self) -> Vec<String> {
            vec!["fs_echo".into()]
        }
    }

    struct RecordingToolbox {
        executed: Rc<RefCell<Vec<String>>>,
        rules: Rules,
    }

    impl Toolbox for RecordingToolbox {
        fn definitions(&self) -> Value {
            json!([])
        }

        fn execute(
            &self,
            name: &str,
            _args: &Value,
            _access: CommandAccess,
            _stop: &AtomicBool,
            _on_output: &mut dyn FnMut(OutputStream, &str),
        ) -> ToolResult {
            self.executed.borrow_mut().push(name.to_string());
            ToolResult::success("ran".into(), false)
        }

        fn check_rules(&self, name: &str, _args: &Value) -> RuleCheck<'_> {
            RuleCheck {
                decision: self.rules.decide(&Request::Tool(name)),
                subject: name.to_string(),
                proposal: None,
            }
        }

        fn mcp_tool_names(&self) -> Vec<String> {
            Vec::new()
        }
    }

    struct Harness<'a> {
        log: MemoryLog,
        model: Scripted,
        tools: RecordingToolbox,
        stop: &'a AtomicBool,
        workdir: PathBuf,
        on_event: Box<dyn FnMut(&Event) + 'a>,
        agents: Vec<AgentDefinition>,
    }

    impl<'a> Harness<'a> {
        fn new(rules: Rules, stop: &'a AtomicBool, agents: Vec<AgentDefinition>) -> Self {
            Harness {
                log: MemoryLog::default(),
                model: Scripted(RefCell::new(vec![])),
                tools: RecordingToolbox {
                    executed: Rc::new(RefCell::new(Vec::new())),
                    rules,
                },
                stop,
                workdir: PathBuf::from("/tmp"),
                on_event: Box::new(|_: &Event| {}),
                agents,
            }
        }

        fn turn(&mut self, autonomous: bool) -> Turn<'_> {
            Turn {
                model: &self.model,
                log: &self.log,
                tools: &self.tools,
                session_id: "s",
                workdir: &self.workdir,
                instructions: "",
                skills: &[],
                always_on: &[],
                agents: &self.agents,
                max_tokens: 100,
                context_window: 1000,
                compact_at: 0.0,
                reasoning_effort: None,
                autonomous,
                lang: crate::domain::lang::Lang::Fr,
                preflight_sent_back: false,
                stop: self.stop,
                on_event: &mut self.on_event,
                children: None,
            }
        }

        fn turn_with_agent(&mut self, autonomous: bool) -> (Turn<'_>, AgentDefinition) {
            let agent = self.agents[0].clone();
            (self.turn(autonomous), agent)
        }
    }

    fn rule(id: i64, tool: &str, action: Action) -> Rule {
        Rule {
            id,
            scope: Scope::Global,
            target: Target::parse(TargetKind::Tool, tool, None).unwrap(),
            action,
            note: None,
            origin: Origin::Human,
        }
    }

    fn call(id: &str, name: &str, args: Value) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: args,
            parse_error: None,
            raw_arguments: None,
        }
    }

    fn tests_agent(writes: WriteScope) -> AgentDefinition {
        AgentDefinition {
            id: "tests".into(),
            name: "Tests".into(),
            description: String::new(),
            subagent: false,
            composer: true,
            allowed_tools: vec!["read_file".into(), "write_file".into(), "edit_file".into()],
            commands: CommandAccess::ReadWrite,
            writes,
            reasoning_effort: None,
            color: None,
            phase: None,
            workflow: None,
            prompt: "prompt".into(),
            origin: AgentOrigin::Default,
            path: None,
        }
    }

    fn results(log: &MemoryLog) -> Vec<(String, ToolResult)> {
        log.0
            .borrow()
            .iter()
            .filter_map(|event| match event {
                Event::ToolResult {
                    tool_call_id,
                    result,
                    ..
                } => Some((tool_call_id.clone(), result.clone())),
                _ => None,
            })
            .collect()
    }

    fn count(log: &MemoryLog, pred: impl Fn(&Event) -> bool) -> usize {
        log.0.borrow().iter().filter(|e| pred(e)).count()
    }

    #[test]
    fn an_mcp_tool_call_is_allowed_even_if_not_in_the_agents_list() {
        let executed = Rc::new(RefCell::new(Vec::new()));
        let tools = McpToolbox {
            executed: executed.clone(),
        };
        let log = MemoryLog::default();
        let model = Scripted(RefCell::new(vec![]));
        let stop = AtomicBool::new(false);
        let agents = builder();
        let workdir = PathBuf::from("/tmp");
        let mut on_event = |_: &Event| {};
        let mut turn = Turn {
            model: &model,
            log: &log,
            tools: &tools,
            session_id: "s",
            workdir: &workdir,
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            autonomous: false,
            lang: crate::domain::lang::Lang::Fr,
            preflight_sent_back: false,
            stop: &stop,
            on_event: &mut on_event,
            children: None,
        };
        let agent = &agents[0];
        // The builder does not list `fs_echo`; the MCP set is what allows it.
        assert!(!agent.allows("fs_echo"));
        let mcp_call = ToolCall {
            id: "c1".into(),
            name: "fs_echo".into(),
            arguments: json!({}),
            parse_error: None,
            raw_arguments: None,
        };
        execute_tools(&mut turn, agent, "m1", &[mcp_call]).unwrap();
        assert_eq!(*executed.borrow(), vec!["fs_echo".to_string()]);

        // A non-MCP tool not in the list is still refused and never runs.
        let refused = ToolCall {
            id: "c2".into(),
            name: "web_search".into(),
            arguments: json!({}),
            parse_error: None,
            raw_arguments: None,
        };
        execute_tools(&mut turn, agent, "m2", &[refused]).unwrap();
        assert_eq!(*executed.borrow(), vec!["fs_echo".to_string()]);
        assert!(
            log.0
                .borrow()
                .iter()
                .any(|e| matches!(e, Event::ToolResult { result, .. }
                if !result.success
                    && result.error.as_deref().is_some_and(|m| m.contains("not available"))))
        );
    }

    #[test]
    fn stop_true_records_interrupted_and_continues() {
        let stop = AtomicBool::new(true);
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, builder());
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "read_file", json!({})),
                call("c2", "write_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert!(h.tools.executed.borrow().is_empty());

        let results = results(&h.log);
        let ids = results
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["c1", "c2"]);
        for (_, result) in &results {
            assert!(!result.success);
            assert_eq!(result.error.as_deref(), Some(super::INTERRUPTED_ERROR));
        }
    }

    #[test]
    fn ask_user_pauses_later_calls_not_run() {
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, builder());
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "ask_user", json!({"question": "Which?"})),
                call("c2", "read_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::WaitingForUser));
        assert!(h.tools.executed.borrow().is_empty());
        assert_eq!(
            results(&h.log),
            vec![(
                "c2".to_string(),
                ToolResult::failure(super::NOT_RUN_AFTER_ASK.into())
            )]
        );
        assert_eq!(
            count(
                &h.log,
                |e| matches!(e, Event::ChatAskUser { call_id, .. } if call_id == "c1")
            ),
            1
        );
    }

    #[test]
    fn parse_error_failure_contains_message_and_does_not_run() {
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, builder());
        let (mut turn, agent) = h.turn_with_agent(false);
        let bad_call = ToolCall {
            id: "c1".into(),
            name: "read_file".into(),
            arguments: json!({}),
            parse_error: Some("bad json".into()),
            raw_arguments: None,
        };
        let flow = execute_tools(&mut turn, &agent, "m1", &[bad_call]).unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert!(h.tools.executed.borrow().is_empty());

        let results = results(&h.log);
        assert_eq!(results.len(), 1);
        let error = results[0].1.error.as_deref().unwrap();
        assert!(error.contains("Failed to parse tool call arguments"));
        assert!(error.contains("bad json"));
    }

    #[test]
    fn scope_refusal_keeps_tests_agent_to_test_files() {
        let tests = tests_agent(WriteScope::Tests);
        assert!(
            super::scope_refusal(
                &tests,
                &call("c1", "write_file", json!({"path": "tests/foo.rs"}))
            )
            .is_none()
        );
        assert!(
            super::scope_refusal(
                &tests,
                &call("c1", "write_file", json!({"path": "./tests/foo.rs"}))
            )
            .is_none()
        );
        assert!(
            super::scope_refusal(
                &tests,
                &call("c1", "write_file", json!({"path": "src/lib.rs"}))
            )
            .is_some()
        );
        assert!(
            super::scope_refusal(
                &tests,
                &call("c1", "edit_file", json!({"path": "src/lib.rs"}))
            )
            .is_some()
        );
        assert!(
            super::scope_refusal(
                &tests,
                &call("c1", "read_file", json!({"path": "src/lib.rs"}))
            )
            .is_none()
        );

        let any = tests_agent(WriteScope::Any);
        assert!(
            super::scope_refusal(
                &any,
                &call("c1", "write_file", json!({"path": "src/lib.rs"}))
            )
            .is_none()
        );
    }

    #[test]
    fn plan_agent_denies_write_file_allows_read_file() {
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, vec![plan()]);
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "write_file", json!({})),
                call("c2", "read_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert_eq!(*h.tools.executed.borrow(), vec!["read_file".to_string()]);

        let results = results(&h.log);
        assert_eq!(results.len(), 2);
        let denied = agent.denied("write_file");
        assert_eq!(results[0].0, "c1");
        assert!(!results[0].1.success);
        assert_eq!(results[0].1.error.as_deref(), Some(denied.as_str()));
        assert_eq!(results[1].0, "c2");
        assert!(results[1].1.success);
        assert_eq!(results[1].1.output.as_deref(), Some("ran"));
    }

    #[test]
    fn autonomous_away_results_for_special_tools() {
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, builder());
        let (mut turn, agent) = h.turn_with_agent(true);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "ask_user", json!({"question": "Which?"})),
                call(
                    "c2",
                    "request_waiver",
                    json!({"path": "AGENTS.md", "reason": "need it"}),
                ),
                call(
                    "c3",
                    "propose_criteria",
                    json!({
                        "criteria": [{"description": "tests pass"}],
                        "reason": "needed"
                    }),
                ),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert!(h.tools.executed.borrow().is_empty());
        assert_eq!(
            results(&h.log),
            vec![
                (
                    "c1".to_string(),
                    ToolResult::success(crate::agent::own_tools::AWAY_ANSWER.into(), false)
                ),
                (
                    "c2".to_string(),
                    ToolResult::failure(crate::agent::own_tools::AWAY_WAIVER.into())
                ),
                (
                    "c3".to_string(),
                    ToolResult::failure(crate::agent::own_tools::AWAY_CRITERIA.into())
                ),
            ]
        );
        assert_eq!(count(&h.log, |e| matches!(e, Event::ChatAskUser { .. })), 0);
        assert_eq!(
            count(&h.log, |e| matches!(e, Event::WaiverRequested { .. })),
            0
        );
        assert_eq!(
            count(&h.log, |e| matches!(e, Event::CriteriaProposed { .. })),
            0
        );
    }

    #[test]
    fn invalid_ask_user_arguments_are_failure_not_pause() {
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, builder());
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[call("c1", "ask_user", json!({}))],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert!(h.tools.executed.borrow().is_empty());

        let results = results(&h.log);
        assert_eq!(results.len(), 1);
        assert!(!results[0].1.success);
        assert!(results[0].1.error.is_some());
        assert_eq!(count(&h.log, |e| matches!(e, Event::ChatAskUser { .. })), 0);
    }

    #[test]
    fn block_rule_records_rule_blocked_and_failure() {
        let stop = AtomicBool::new(false);
        let rule = rule(7, "read_file", Action::Block);
        let refusal = rule.refusal();
        let mut h = Harness::new(Rules::new(vec![rule]), &stop, builder());
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[call("c1", "read_file", json!({}))],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert!(h.tools.executed.borrow().is_empty());

        let results = results(&h.log);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "c1");
        assert!(!results[0].1.success);
        assert_eq!(results[0].1.error.as_deref(), Some(refusal.as_str()));

        let events = h.log.0.borrow();
        let blocked = events.iter().position(|e| {
            matches!(e, Event::RuleBlocked { call_id, rule_id, .. }
                if call_id == "c1" && rule_id == "7")
        });
        let result = events.iter().position(|e| {
            matches!(e, Event::ToolResult { tool_call_id, .. }
                if tool_call_id == "c1")
        });
        assert!(blocked.is_some_and(|b| result.is_some_and(|r| b < r)));
    }

    #[test]
    fn ask_rule_with_human_requests_approval_and_not_run_after_ask() {
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(
            Rules::new(vec![rule(7, "read_file", Action::Ask)]),
            &stop,
            builder(),
        );
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "read_file", json!({})),
                call("c2", "write_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::WaitingForUser));
        assert!(h.tools.executed.borrow().is_empty());
        assert_eq!(
            results(&h.log),
            vec![(
                "c2".to_string(),
                ToolResult::failure(super::NOT_RUN_AFTER_ASK.into())
            )]
        );
        assert_eq!(
            count(
                &h.log,
                |e| matches!(e, Event::RuleApprovalRequested { call_id, rule_id, .. }
                    if call_id == "c1" && rule_id.as_deref() == Some("7"))
            ),
            1
        );
    }

    #[test]
    fn ask_rule_autonomous_records_rule_blocked_and_failure() {
        let stop = AtomicBool::new(false);
        let rule = rule(7, "read_file", Action::Ask);
        let refusal = rule.refusal();
        let mut h = Harness::new(Rules::new(vec![rule]), &stop, builder());
        let (mut turn, agent) = h.turn_with_agent(true);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[call("c1", "read_file", json!({}))],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert!(h.tools.executed.borrow().is_empty());

        let results = results(&h.log);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "c1");
        assert!(!results[0].1.success);
        assert_eq!(results[0].1.error.as_deref(), Some(refusal.as_str()));
        assert_eq!(
            count(&h.log, |e| matches!(e, Event::RuleApprovalRequested { .. })),
            0
        );
        assert_eq!(
            count(
                &h.log,
                |e| matches!(e, Event::RuleBlocked { call_id, rule_id, .. }
                    if call_id == "c1" && rule_id == "7")
            ),
            1
        );
    }

    #[test]
    fn record_decision_and_propose_task_do_not_pause() {
        let stop = AtomicBool::new(false);
        let mut agents = builder();
        agents[0].allowed_tools.push("propose_task".into());
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, agents);
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call(
                    "c1",
                    "record_decision",
                    json!({
                        "title": "use SQLite",
                        "choice": "use SQLite",
                        "reason": "bundled",
                        "alternatives": ["postgres"]
                    }),
                ),
                call(
                    "c2",
                    "propose_task",
                    json!({"title": "Add tests", "text": "cover the loop"}),
                ),
                call("c3", "read_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert_eq!(*h.tools.executed.borrow(), vec!["read_file".to_string()]);

        let results = results(&h.log);
        assert_eq!(results.len(), 3);
        assert!(results[0].1.success);
        assert!(results[1].1.success);
        assert!(results[2].1.success);
        assert_eq!(results[2].1.output.as_deref(), Some("ran"));
    }

    fn plan_args(check: Option<&str>) -> Value {
        let mut criterion = json!({"text": "The build passes"});
        if let Some(check) = check {
            criterion["check"] = json!(check);
        }
        json!({
            "title": "Add the plan tool",
            "goal": "The plan agent ends its turn with propose_plan",
            "steps": [
                {
                    "title": "Write the domain",
                    "files": ["src/domain/plan.rs"],
                    "change": "The plan's vocabulary"
                }
            ],
            "doneWhen": [criterion],
            "risks": []
        })
    }

    #[test]
    fn an_invalid_propose_plan_is_a_failure_and_the_turn_continues() {
        let stop = AtomicBool::new(false);
        let mut agents = vec![plan()];
        agents[0].allowed_tools.push("propose_plan".into());
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, agents);
        let (mut turn, agent) = h.turn_with_agent(false);
        let mut args = plan_args(None);
        args.as_object_mut().unwrap().remove("goal");
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "propose_plan", args),
                call("c2", "read_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert_eq!(*h.tools.executed.borrow(), vec!["read_file".to_string()]);
        let results = results(&h.log);
        assert_eq!(results.len(), 2);
        assert!(!results[0].1.success);
        assert!(results[0].1.error.as_deref().unwrap().contains("'goal'"));
        assert_eq!(
            count(&h.log, |e| matches!(e, Event::PlanProposed { .. })),
            0
        );
    }

    #[test]
    fn a_valid_propose_plan_records_the_plan_and_ends_the_turn() {
        let stop = AtomicBool::new(false);
        let mut agents = vec![plan()];
        agents[0].allowed_tools.push("propose_plan".into());
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, agents);
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "propose_plan", plan_args(None)),
                call("c2", "read_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Done));
        assert!(h.tools.executed.borrow().is_empty());
        let results = results(&h.log);
        assert_eq!(results.len(), 2);
        assert!(results[0].1.success);
        assert_eq!(
            results[0].1.output.as_deref(),
            Some(crate::agent::propose_plan::RECORDED)
        );
        assert_eq!(
            results[1],
            (
                "c2".to_string(),
                ToolResult::failure(super::NOT_RUN_AFTER_ASK.into())
            )
        );
        let planned = h
            .log
            .0
            .borrow()
            .iter()
            .find_map(|e| match e {
                Event::PlanProposed { call_id, plan, .. } if call_id == "c1" => Some(plan.clone()),
                _ => None,
            })
            .expect("the plan is recorded");
        assert_eq!(
            planned,
            crate::agent::propose_plan::parse(&plan_args(None)).unwrap()
        );
    }

    fn report_args() -> Value {
        json!({
            "title": "Add the report tool",
            "summary": "The builder ends its turn with submit_report",
            "steps": [
                {"title": "Write the domain", "done": true, "note": "The vocabulary"},
                {"title": "Wire the tool", "done": false, "note": "Not yet"}
            ],
            "doneWhen": [
                {"text": "The build passes", "met": true, "how": "cargo build"}
            ],
            "changed": [{"path": "src/domain/report.rs", "what": "The vocabulary"}],
            "test": "",
            "todo": ""
        })
    }

    #[test]
    fn an_invalid_submit_report_is_a_failure_and_the_turn_continues() {
        let stop = AtomicBool::new(false);
        let mut agents = builder();
        agents[0].allowed_tools.push("submit_report".into());
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, agents);
        let (mut turn, agent) = h.turn_with_agent(false);
        let mut args = report_args();
        args.as_object_mut().unwrap().remove("summary");
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "submit_report", args),
                call("c2", "read_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert_eq!(*h.tools.executed.borrow(), vec!["read_file".to_string()]);
        let results = results(&h.log);
        assert_eq!(results.len(), 2);
        assert!(!results[0].1.success);
        assert!(results[0].1.error.as_deref().unwrap().contains("'summary'"));
        assert_eq!(
            count(&h.log, |e| matches!(e, Event::ReportSubmitted { .. })),
            0
        );
    }

    #[test]
    fn a_valid_submit_report_records_the_report_and_ends_the_turn() {
        let stop = AtomicBool::new(false);
        let mut agents = builder();
        agents[0].allowed_tools.push("submit_report".into());
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, agents);
        let (mut turn, agent) = h.turn_with_agent(false);
        let flow = execute_tools(
            &mut turn,
            &agent,
            "m1",
            &[
                call("c1", "submit_report", report_args()),
                call("c2", "read_file", json!({})),
            ],
        )
        .unwrap();

        assert!(matches!(flow, super::Flow::Done));
        assert!(h.tools.executed.borrow().is_empty());
        let results = results(&h.log);
        assert_eq!(results.len(), 2);
        assert!(results[0].1.success);
        assert_eq!(
            results[0].1.output.as_deref(),
            Some(crate::agent::submit_report::RECORDED)
        );
        assert_eq!(
            results[1],
            (
                "c2".to_string(),
                ToolResult::failure(super::NOT_RUN_AFTER_ASK.into())
            )
        );
        let submitted = h
            .log
            .0
            .borrow()
            .iter()
            .find_map(|e| match e {
                Event::ReportSubmitted {
                    call_id, report, ..
                } if call_id == "c1" => Some(report.clone()),
                _ => None,
            })
            .expect("the report is recorded");
        assert_eq!(
            submitted,
            crate::agent::submit_report::parse(&report_args()).unwrap()
        );
    }

    /// `run_command` answers with a passing exit code, so the preflight
    /// classifies the check as passing already.
    struct PassingRunToolbox {
        executed: Rc<RefCell<Vec<String>>>,
    }

    impl Toolbox for PassingRunToolbox {
        fn check_rules(&self, _name: &str, _args: &Value) -> RuleCheck<'_> {
            RuleCheck::allowed()
        }
        fn definitions(&self) -> Value {
            json!([])
        }

        fn execute(
            &self,
            name: &str,
            args: &Value,
            _access: CommandAccess,
            _stop: &AtomicBool,
            _on_output: &mut dyn FnMut(OutputStream, &str),
        ) -> ToolResult {
            if name == "run_command" {
                let command = args
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.executed.borrow_mut().push(command.to_string());
                ToolResult::success(format!("$ {command}\n[Exit code: 0]"), false)
            } else {
                ToolResult::success("ran".into(), false)
            }
        }
    }

    #[test]
    fn the_preflight_feedback_is_sent_back_once_from_the_tool() {
        let stop = AtomicBool::new(false);
        let executed = Rc::new(RefCell::new(Vec::new()));
        let tools = PassingRunToolbox {
            executed: executed.clone(),
        };
        let log = MemoryLog::default();
        let model = Scripted(RefCell::new(vec![]));
        let mut agents = vec![plan()];
        agents[0].allowed_tools.push("propose_plan".into());
        let workdir = PathBuf::from("/tmp");
        let mut on_event = |_: &Event| {};
        let mut turn = Turn {
            model: &model,
            log: &log,
            tools: &tools,
            session_id: "s",
            workdir: &workdir,
            instructions: "",
            skills: &[],
            always_on: &[],
            agents: &agents,
            max_tokens: 100,
            context_window: 1000,
            compact_at: 0.0,
            reasoning_effort: None,
            autonomous: false,
            lang: crate::domain::lang::Lang::Fr,
            preflight_sent_back: false,
            stop: &stop,
            on_event: &mut on_event,
            children: None,
        };
        let agent = &agents[0];

        // The check passes already: the plan is sent back as the tool's
        // error, no event, the turn goes on.
        let flow = execute_tools(
            &mut turn,
            agent,
            "m1",
            &[call("c1", "propose_plan", plan_args(Some("true")))],
        )
        .unwrap();
        assert!(matches!(flow, super::Flow::Continue));
        assert_eq!(*executed.borrow(), vec!["true".to_string()]);
        assert!(
            log.0
                .borrow()
                .iter()
                .any(|e| matches!(e, Event::CriteriaPreflight { .. }))
        );
        assert_eq!(count(&log, |e| matches!(e, Event::PlanProposed { .. })), 0);
        let logged = results(&log);
        assert_eq!(logged.len(), 1);
        assert!(!logged[0].1.success);
        assert!(
            logged[0]
                .1
                .error
                .as_deref()
                .unwrap()
                .contains("passes already")
        );

        // The revised plan: the preflight is not run a second time, the plan
        // is recorded and the turn ends.
        let flow = execute_tools(
            &mut turn,
            agent,
            "m2",
            &[call("c2", "propose_plan", plan_args(Some("cargo test")))],
        )
        .unwrap();
        assert!(matches!(flow, super::Flow::Done));
        assert_eq!(*executed.borrow(), vec!["true".to_string()]);
        assert_eq!(count(&log, |e| matches!(e, Event::PlanProposed { .. })), 1);
        let logged = results(&log);
        assert!(
            logged
                .iter()
                .find(|(id, _)| id == "c2")
                .is_some_and(|(_, r)| r.success)
        );
    }
    /// Child sessions that record what they were asked and answer one text.
    struct FakeChildren {
        sent: std::sync::Mutex<Vec<(String, String, String)>>,
    }

    impl crate::agent::ports::ChildSessions for FakeChildren {
        fn create(&self, parent: &str, title: &str) -> Result<String, String> {
            assert_eq!(parent, "s");
            assert!(title.starts_with("plan: "), "{title}");
            Ok("child".into())
        }

        fn send(
            &self,
            child: &str,
            agent: &str,
            prompt: &str,
            _stop: &AtomicBool,
        ) -> Result<crate::agent::ports::AgentRun, String> {
            self.sent
                .lock()
                .unwrap()
                .push((child.into(), agent.into(), prompt.into()));
            Ok(crate::agent::ports::AgentRun {
                reason: crate::domain::event::DoneReason::Complete,
                text: "found it in store.rs:12".into(),
            })
        }
    }

    #[test]
    fn delegate_runs_a_read_only_agent_in_a_child_and_returns_its_answer() {
        let stop = AtomicBool::new(false);
        let children = FakeChildren {
            sent: std::sync::Mutex::new(Vec::new()),
        };
        let mut agents = builder();
        agents[0].allowed_tools.push("delegate".into());
        agents.push(plan());
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, agents);
        let (mut turn, agent) = h.turn_with_agent(false);
        turn.children = Some(&children);
        let args = json!({"agent": "plan", "task": "Where are sessions stored?"});
        let flow = execute_tools(&mut turn, &agent, "m1", &[call("c1", "delegate", args)]).unwrap();

        assert!(matches!(flow, super::Flow::Continue));
        assert_eq!(
            *children.sent.lock().unwrap(),
            vec![(
                "child".to_string(),
                "plan".to_string(),
                format!(
                    "{}Where are sessions stored?",
                    crate::agent::delegate::CHILD_BRIEF
                )
            )]
        );
        let (id, result) = &results(&h.log)[0];
        assert_eq!(id, "c1");
        assert!(result.success);
        assert!(
            result
                .output
                .as_deref()
                .is_some_and(|o| o.starts_with("found it in store.rs:12"))
        );
    }

    #[test]
    fn delegate_without_child_sessions_is_a_failure() {
        let stop = AtomicBool::new(false);
        let mut agents = builder();
        agents[0].allowed_tools.push("delegate".into());
        agents.push(plan());
        let mut h = Harness::new(Rules::new(Vec::new()), &stop, agents);
        let (mut turn, agent) = h.turn_with_agent(false);
        let args = json!({"agent": "plan", "task": "Where are sessions stored?"});
        execute_tools(&mut turn, &agent, "m1", &[call("c1", "delegate", args)]).unwrap();

        let (_, result) = &results(&h.log)[0];
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .is_some_and(|e| e.contains("not available"))
        );
    }
}
