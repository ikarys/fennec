//! The human's answer to a `rule.approval_requested`: record it, run the
//! recorded call (or refuse it for "never"), then resume the loop with no new
//! user message.

use super::tool_calls;
use super::turn::{self, Turn};
use crate::domain::agent::resolve;
use crate::domain::event::{DoneReason, Event, ToolResult};
use crate::domain::rule_approval::Choice;

/// Records the answer, produces the recorded call's `tool.result`, and runs
/// the agent loop on.
pub fn approve_turn(
    turn: &mut Turn,
    call_id: &str,
    choice: Choice,
    rule_id: Option<i64>,
) -> Result<DoneReason, String> {
    turn::with_running(turn, |turn| {
        let events = turn.log.events(turn.session_id)?;
        let pending = crate::domain::rule_approval::pending(&events)
            .filter(|pending| pending.call_id == call_id)
            .ok_or_else(|| "no pending approval for this call".to_string())?;
        let agent =
            resolve(turn.agents, &events).ok_or_else(|| "no top-level agent".to_string())?;

        turn.append(Event::RuleApprovalAnswered {
            call_id: call_id.to_string(),
            choice: choice.as_str().to_string(),
            rule_id: rule_id.map(|id| id.to_string()),
        })?;

        let result = match choice {
            Choice::Never => ToolResult::failure(
                "Refused by the human. Do not retry it in another form.".to_string(),
            ),
            Choice::Once | Choice::Session | Choice::Always => {
                tool_calls::run_tool(turn, agent, &pending.message_id, &pending.call, true)?
            }
        };
        tool_calls::record(turn, &pending.message_id, &pending.call, result)?;

        turn::run_top_level_agent_loop(turn, agent)
    })
}

#[cfg(test)]
mod tests {
    use super::approve_turn;
    use crate::agent::ports::Model;
    use crate::agent::test_support::{MemoryLog, Scripted, builder, reply};
    use crate::agent::turn::{Turn, run_turn};
    use crate::domain::agent::AgentDefinition;
    use crate::domain::event::{DoneReason, Event, ToolCall};
    use crate::domain::rule_approval::Choice;
    use crate::tools::LocalTools;
    use serde_json::json;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    struct Harness<'a> {
        log: MemoryLog,
        model: Box<dyn Model>,
        tools: LocalTools,
        stop: &'a AtomicBool,
        workdir: PathBuf,
        on_event: Box<dyn FnMut(&Event) + 'a>,
        agents: Vec<AgentDefinition>,
    }

    impl<'a> Harness<'a> {
        fn new(
            model: Box<dyn Model>,
            workdir: PathBuf,
            rules: rules::Rules,
            stop: &'a AtomicBool,
        ) -> Self {
            Harness {
                log: MemoryLog::default(),
                model,
                tools: LocalTools::new(workdir.clone()).with_rules(rules),
                stop,
                workdir,
                on_event: Box::new(|_: &Event| {}),
                agents: builder(),
            }
        }

        fn turn(&mut self, autonomous: bool) -> Turn<'_> {
            Turn {
                model: &*self.model,
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
                session_effort: None,
                autonomous,
                lang: crate::domain::lang::Lang::Fr,
                preflight_sent_back: false,
                stop: self.stop,
                on_event: &mut self.on_event,
                children: None,
            }
        }
    }

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fennec-rule-approval-{}",
            crate::domain::id::new_id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ask_rule() -> rules::Rules {
        rules::Rules::new(vec![rules::Rule {
            id: 1,
            scope: rules::Scope::Global,
            target: rules::Target::parse(rules::TargetKind::Command, "touch", None).unwrap(),
            action: rules::Action::Ask,
            note: None,
            origin: rules::Origin::Human,
        }])
    }

    #[test]
    fn once_runs_the_recorded_call_and_resumes() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "run_command".into(),
            arguments: json!({"command": "touch made"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), ask_rule(), &stop);

        let reason = run_turn(&mut h.turn(false), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);
        let events = h.log.0.borrow().clone();
        assert!(
            events.iter().any(
                |e| matches!(e, Event::RuleApprovalRequested { call_id, .. } if call_id == "c1")
            )
        );
        assert!(
            !events.iter().any(
                |e| matches!(e, Event::ToolResult { tool_call_id, .. } if tool_call_id == "c1")
            )
        );
        assert!(!dir.join("made").exists());

        let reason = approve_turn(&mut h.turn(false), "c1", Choice::Once, None).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::RuleApprovalAnswered { call_id, choice, .. } if call_id == "c1" && choice == "once")));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.success)));
        assert!(dir.join("made").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn never_refuses_the_recorded_call() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "run_command".into(),
            arguments: json!({"command": "touch made"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), ask_rule(), &stop);

        let reason = run_turn(&mut h.turn(false), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);

        let reason = approve_turn(&mut h.turn(false), "c1", Choice::Never, Some(7)).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::RuleApprovalAnswered { call_id, choice, rule_id, .. } if call_id == "c1" && choice == "never" && rule_id.as_deref() == Some("7"))));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && !result.success && result.error.as_deref().is_some_and(|e| e.contains("Refused by the human")))));
        assert!(!dir.join("made").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn while_the_human_is_away_an_ask_rule_is_refused_not_asked() {
        let dir = scratch_dir();
        let call = ToolCall {
            id: "c1".into(),
            name: "run_command".into(),
            arguments: json!({"command": "touch made"}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), ask_rule(), &stop);

        let reason = run_turn(&mut h.turn(true), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::RuleBlocked { call_id, .. } if call_id == "c1"))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::RuleApprovalRequested { .. }))
        );
        assert!(!dir.join("made").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn once_reads_a_path_outside_the_workdir() {
        let dir = scratch_dir();
        let outside = scratch_dir();
        std::fs::write(outside.join("note.txt"), "outside text").unwrap();
        let call = ToolCall {
            id: "c1".into(),
            name: "read_file".into(),
            arguments: json!({"path": outside.join("note.txt").to_string_lossy()}),
            parse_error: None,
            raw_arguments: None,
        };
        let model = Scripted(RefCell::new(vec![
            reply("", vec![call]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), rules::Rules::default(), &stop);

        let reason = run_turn(&mut h.turn(false), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);
        let events = h.log.0.borrow().clone();
        assert!(
            events.iter().any(
                |e| matches!(e, Event::RuleApprovalRequested { call_id, .. } if call_id == "c1")
            )
        );

        let reason = approve_turn(&mut h.turn(false), "c1", Choice::Once, None).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.success && result.output.as_deref().is_some_and(|o| o.contains("outside text")))));

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
