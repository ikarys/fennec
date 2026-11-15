//! The tests gate, answered by fennec when it proves there is nothing to decide:
//! every criterion has a check, and each one runs, selects a test and fails before the work.

use super::WorkflowEnv;
use crate::domain::check_kind::is_existence_only;
use crate::domain::criteria::CriterionState;
use crate::domain::preflight::{self, PreflightVerdict};
use crate::domain::workflow::definition::DEFAULT_SHELL_TIMEOUT_SECS;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub(super) enum Proof {
    /// Every check fails before the work: the node's output, one line per check.
    Holds(String),
    /// Why the human must look, in one clause.
    Missing(String),
    Stopped,
}

pub(super) fn prove(env: &WorkflowEnv, criteria: &[CriterionState], stop: &AtomicBool) -> Proof {
    if criteria.is_empty() {
        return Proof::Missing("the plan has no criteria".into());
    }
    let mut lines = vec!["Validated by fennec: every check fails before the work.".to_string()];
    for state in criteria {
        let id = &state.criterion.id;
        let Some(command) = &state.criterion.check else {
            return Proof::Missing(format!("{id} has no check"));
        };
        // A search proves the code exists, not that it works: the human reads those tests.
        if is_existence_only(command) {
            return Proof::Missing(format!("{id}'s check only searches the code"));
        }
        let run = env.commands.run(
            command,
            Duration::from_secs(DEFAULT_SHELL_TIMEOUT_SECS),
            stop,
        );
        if run.stopped {
            return Proof::Stopped;
        }
        let missing = match preflight::classify(run.exit_code, &run.output) {
            PreflightVerdict::Fails => None,
            PreflightVerdict::PassesAlready => Some("passes before the work"),
            PreflightVerdict::CannotRun => Some("cannot run here"),
            PreflightVerdict::RanNothing => Some("runs no test"),
        };
        if let Some(missing) = missing {
            return Proof::Missing(format!("{id}'s check {missing}"));
        }
        let exit = run.exit_code.map_or("none".into(), |code| code.to_string());
        lines.push(format!("- {id} `{command}`: fails (exit {exit})"));
    }
    Proof::Holds(lines.join("\n"))
}
