//! Runs a plan's `check:` commands before the builder starts, so a check that
//! cannot run or already passes is fixed by the planner in the same turn.

use super::turn::Turn;
use crate::domain::agent::{AgentDefinition, CommandAccess};
use crate::domain::criteria::Criterion;
use crate::domain::event::Event;
use crate::domain::preflight::{self, PreflightResult};
use serde_json::json;
use std::sync::atomic::Ordering;

const TWO_KIB: usize = 2048;

pub(super) fn run(
    turn: &mut Turn,
    agent: &AgentDefinition,
    message_id: &str,
    criteria: &[Criterion],
) -> Result<Option<String>, String> {
    // Only the read-only planner writes a plan; its checks cannot change the
    // workdir, so running them now is a fair test of the builder's sandbox.
    if agent.commands != CommandAccess::ReadOnly {
        return Ok(None);
    }

    let checks: Vec<(&Criterion, &String)> = criteria
        .iter()
        .filter_map(|criterion| criterion.check.as_ref().map(|check| (criterion, check)))
        .collect();
    if checks.is_empty() {
        return Ok(None);
    }

    let mut results = Vec::with_capacity(checks.len());
    for (criterion, check) in &checks {
        if turn.stop.load(Ordering::Relaxed) {
            return Ok(None);
        }

        let result = turn.tools.execute(
            "run_command",
            &json!({ "command": check }),
            agent.commands,
            turn.stop,
            &mut |_, _| {},
        );
        if turn.stop.load(Ordering::Relaxed) {
            return Ok(None);
        }

        let output = result
            .output
            .clone()
            .or_else(|| result.error.clone())
            .unwrap_or_default();
        let exit_code = preflight::exit_code(&output);
        // No exit code: the tool refused the command (the sandbox is turned
        // off), so nothing was tested and the planner has nothing to fix.
        if exit_code.is_none() {
            return Ok(None);
        }
        results.push(PreflightResult {
            criterion_id: criterion.id.clone(),
            command: (*check).clone(),
            verdict: preflight::classify(exit_code, &output),
            exit_code,
            output: last_two_kib(&output),
            existence_only: crate::domain::check_kind::is_existence_only(check),
        });
    }

    let feedback = preflight::feedback(&results);
    turn.append(Event::CriteriaPreflight {
        message_id: message_id.to_string(),
        results,
    })?;
    Ok(feedback)
}

fn last_two_kib(output: &str) -> String {
    if output.len() <= TWO_KIB {
        return output.to_string();
    }
    let mut start = output.len() - TWO_KIB;
    while !output.is_char_boundary(start) {
        start += 1;
    }
    output[start..].to_string()
}
