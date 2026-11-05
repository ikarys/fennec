//! The human's answer to a `waiver.requested`: record it, produce the
//! recorded call's `tool.result`, then resume the loop with no new user
//! message.

use super::tool_calls;
use super::turn::{self, Turn};
use crate::domain::agent::resolve;
use crate::domain::clock;
use crate::domain::event::{DoneReason, Event, ToolResult};
use crate::domain::waiver;

/// Produces the answered waiver call's `tool.result` (the answer itself was
/// recorded at the human's click), and runs the agent loop on.
pub fn answer_waiver(turn: &mut Turn, call_id: &str) -> Result<DoneReason, String> {
    turn::with_running(turn, |turn| {
        let events = turn.log.events(turn.session_id)?;
        let (path, answer) = waiver::answered(&events, call_id)
            .ok_or_else(|| "no answered waiver awaiting its result".to_string())?;
        let agent =
            resolve(turn.agents, &events).ok_or_else(|| "no top-level agent".to_string())?;
        let (message_id, call) = events
            .iter()
            .find_map(|event| match event {
                Event::ToolCall {
                    message_id,
                    tool_call,
                } if tool_call.id == call_id => Some((message_id.clone(), tool_call.clone())),
                _ => None,
            })
            .ok_or_else(|| "no tool call for this waiver".to_string())?;

        let result = match (answer.granted, answer.expires_at, answer.adr) {
            (true, Some(expires_at), Some(adr)) => ToolResult::success(
                format!(
                    "Granted by the human: you may write {path} until {} (UTC), in this session only. The waiver is recorded in {adr}.",
                    clock::format_utc(expires_at)
                ),
                false,
            ),
            _ => ToolResult::failure(
                "Refused by the human. Do not work around it: say in your report what should change there and why."
                    .to_string(),
            ),
        };
        tool_calls::record(turn, &message_id, &call, result)?;

        turn::run_top_level_agent_loop(turn, agent)
    })
}

#[cfg(test)]
mod tests {
    use super::answer_waiver;
    use crate::agent::ports::{EventLog, Model};
    use crate::agent::test_support::{MemoryLog, Scripted, builder, reply};
    use crate::agent::turn::{Turn, run_turn};
    use crate::domain::agent::AgentDefinition;
    use crate::domain::event::{DoneReason, Event, ToolCall};
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
        fn new(model: Box<dyn Model>, workdir: PathBuf, stop: &'a AtomicBool) -> Self {
            Harness {
                log: MemoryLog::default(),
                model,
                tools: LocalTools::new(workdir.clone()).with_rules(rules::Rules::default()),
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
                autonomous,
                lang: crate::domain::lang::Lang::Fr,
                preflight_sent_back: false,
                stop: self.stop,
                on_event: &mut self.on_event,
            }
        }
    }

    fn scratch_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fennec-waiver-{}", crate::domain::id::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn waiver_call() -> ToolCall {
        ToolCall {
            id: "c1".into(),
            name: "request_waiver".into(),
            arguments: json!({"path": "AGENTS.md", "reason": "why"}),
            parse_error: None,
            raw_arguments: None,
        }
    }

    /// What the route records at the human's click.
    fn answer(h: &Harness, granted: bool) {
        h.log
            .append(
                "s",
                &Event::WaiverAnswered {
                    call_id: "c1".into(),
                    granted,
                    expires_at: granted.then(|| crate::domain::clock::now_ms() + 3_600_000),
                    adr: granted.then(|| "docs/adr/0001-waiver-agents-md.md".into()),
                },
            )
            .unwrap();
    }

    #[test]
    fn without_an_answer_there_is_nothing_to_resume() {
        let dir = scratch_dir();
        let model = Scripted(RefCell::new(vec![reply("", vec![waiver_call()])]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);
        run_turn(&mut h.turn(false), "go", &[]).unwrap();
        assert!(answer_waiver(&mut h.turn(false), "c1").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn grant_records_the_answer_and_resumes() {
        let dir = scratch_dir();
        let model = Scripted(RefCell::new(vec![
            reply("", vec![waiver_call()]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);

        let reason = run_turn(&mut h.turn(false), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);
        let events = h.log.0.borrow().clone();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::WaiverRequested { call_id, .. } if call_id == "c1"))
        );
        assert!(
            !events.iter().any(
                |e| matches!(e, Event::ToolResult { tool_call_id, .. } if tool_call_id == "c1")
            )
        );

        answer(&h, true);
        let reason = answer_waiver(&mut h.turn(false), "c1").unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::WaiverAnswered { call_id, granted, .. } if call_id == "c1" && *granted)));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && result.success && result.output.as_deref().is_some_and(|o| o.contains("Granted by the human")))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refusal_records_the_answer_and_resumes() {
        let dir = scratch_dir();
        let model = Scripted(RefCell::new(vec![
            reply("", vec![waiver_call()]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);

        let reason = run_turn(&mut h.turn(false), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::WaitingForUser);

        answer(&h, false);
        let reason = answer_waiver(&mut h.turn(false), "c1").unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::WaiverAnswered { call_id, granted, .. } if call_id == "c1" && !*granted)));
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && !result.success && result.error.as_deref().is_some_and(|e| e.contains("Refused by the human")))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn while_the_human_is_away_a_waiver_is_refused_not_asked() {
        let dir = scratch_dir();
        let model = Scripted(RefCell::new(vec![
            reply("", vec![waiver_call()]),
            reply("done", vec![]),
        ]));
        let stop = AtomicBool::new(false);
        let mut h = Harness::new(Box::new(model), dir.clone(), &stop);

        let reason = run_turn(&mut h.turn(true), "go", &[]).unwrap();
        assert_eq!(reason, DoneReason::Complete);
        let events = h.log.0.borrow().clone();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::WaiverRequested { .. }))
        );
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::ToolResult { tool_call_id, result, .. } if tool_call_id == "c1" && !result.success && result.error.as_deref().is_some_and(|e| e.contains("The human is away")))));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
