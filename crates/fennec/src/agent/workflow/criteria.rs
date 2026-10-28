//! The criteria node: records the plan's "Done when" list and runs its checks.

use super::WorkflowEnv;
use super::nodes::{NodeEnd, NodeInput};
use super::outcome;
use crate::agent::turn::Turn;
use crate::domain::criteria::{
    Criterion, CriterionState, CriterionStatus, Provenance, failures_summary, parse_done_when,
    parse_verdicts,
};
use crate::domain::event::Event;
use crate::domain::workflow::definition::{DEFAULT_SHELL_TIMEOUT_SECS, NodeKind};
use crate::domain::workflow::run::{self as run, RunOutcome};
use std::time::Duration;

pub(super) fn run(
    turn: &mut Turn,
    env: &WorkflowEnv,
    input: &NodeInput,
    plan_node: &str,
) -> Result<NodeEnd, String> {
    let label = input.node.label();
    let Some(plan_output) = input.state.results.get(plan_node).map(|r| r.output.clone()) else {
        return Ok(NodeEnd::EndRun {
            outcome: RunOutcome::Blocked,
            reason: Some(format!(
                "\"{label}\": the plan node {plan_node} has no output"
            )),
        });
    };

    // Once recorded, only the human changes the criteria (an accepted
    // proposal); re-reading the plan would undo that.
    let recorded: Vec<_> = input
        .state
        .criteria
        .iter()
        .map(|state| state.criterion.clone())
        .collect();
    let criteria = if recorded.is_empty() {
        let criteria = parse_done_when(&plan_output);
        if criteria.is_empty() {
            return Ok(NodeEnd::EndRun {
                outcome: RunOutcome::Blocked,
                reason: Some("the plan has no \"Done when\" criteria".into()),
            });
        }
        turn.append(Event::CriteriaSet {
            criteria: criteria.clone(),
        })?;
        criteria
    } else {
        recorded
    };

    let mut states: Vec<CriterionState> = criteria
        .iter()
        .map(|criterion| CriterionState {
            criterion: criterion.clone(),
            status: CriterionStatus::Pending,
        })
        .collect();

    for state in &mut states {
        let Some(command) = state.criterion.check.clone() else {
            continue;
        };

        let result = env.commands.run(
            &command,
            Duration::from_secs(DEFAULT_SHELL_TIMEOUT_SECS),
            input.stop,
        );
        if result.stopped {
            return Ok(NodeEnd::EndRun {
                outcome: RunOutcome::Stopped,
                reason: None,
            });
        }

        let output = outcome::last_bytes(&result.output, 4 * 1024);
        let status = if result.exit_code == Some(0) {
            CriterionStatus::Passed {
                provenance: Provenance::Verified {
                    command: command.clone(),
                    exit_code: result.exit_code,
                    output,
                },
            }
        } else {
            CriterionStatus::Failed {
                provenance: Provenance::Verified {
                    command: command.clone(),
                    exit_code: result.exit_code,
                    output,
                },
            }
        };

        turn.append(Event::CriterionUpdated {
            criterion_id: state.criterion.id.clone(),
            status: status.clone(),
        })?;
        state.status = status;
    }

    let all_checked_passed = states
        .iter()
        .filter(|state| state.criterion.check.is_some())
        .all(|state| matches!(state.status, CriterionStatus::Passed { .. }));

    Ok(NodeEnd::Finished {
        port: if all_checked_passed {
            "pass".into()
        } else {
            "fail".into()
        },
        output: failures_summary(&states),
        exit_code: None,
    })
}

/// The run's criteria from the plan node its criteria node names, parsed:
/// what a plan approval records.
pub(super) fn plan_criteria(state: &run::RunState) -> Option<Vec<Criterion>> {
    let plan = state
        .workflow
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            NodeKind::Criteria { plan } => Some(plan.clone()),
            _ => None,
        })?;
    let output = state.results.get(&plan)?.output.clone();
    let criteria = parse_done_when(&output);
    if criteria.is_empty() {
        None
    } else {
        Some(criteria)
    }
}

/// The human's approval of the plan is what records its criteria: from then
/// on, a proposal is a diff against the approved contract, not an empty list.
pub(super) fn record_from_plan(turn: &mut Turn, state: &run::RunState) -> Result<(), String> {
    if let Some(criteria) = plan_criteria(state) {
        turn.append(Event::CriteriaSet { criteria })?;
    }
    Ok(())
}

pub(super) fn record_verdicts(
    turn: &mut Turn,
    state: &run::RunState,
    agent: &str,
    output: &str,
) -> Result<(), String> {
    for verdict in parse_verdicts(output) {
        let Some(criterion) = state
            .criteria
            .iter()
            .find(|state| state.criterion.id == verdict.id)
        else {
            continue;
        };

        if criterion.criterion.check.is_some() {
            continue;
        }

        let status = if verdict.passed {
            CriterionStatus::Passed {
                provenance: Provenance::Indicative {
                    agent: agent.to_string(),
                    reason: verdict.reason,
                },
            }
        } else {
            CriterionStatus::Failed {
                provenance: Provenance::Indicative {
                    agent: agent.to_string(),
                    reason: verdict.reason,
                },
            }
        };

        turn.append(Event::CriterionUpdated {
            criterion_id: verdict.id,
            status,
        })?;
    }

    Ok(())
}
