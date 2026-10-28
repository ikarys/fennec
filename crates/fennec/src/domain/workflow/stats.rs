//! Facts about a finished run, computed from the run's events.

use crate::domain::criteria::{CriterionState, CriterionStatus, Provenance};
use crate::domain::event::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Facts about a finished run, computed by fennec from its log (verified, not a model's claim).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStats {
    /// The sum of the nodes' working time: the time waiting for the human is not counted.
    pub working_ms: u64,
    pub nodes: Vec<NodeStats>,
    /// Node runs beyond the first, summed: how much the run looped.
    pub reruns: u32,
    pub files_changed: u32,
    pub criteria_total: u32,
    pub criteria_passed: u32,
    pub criteria_verified: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStats {
    pub node_id: String,
    pub label: String,
    pub runs: u32,
    pub working_ms: u64,
}

/// `run`: the run's events in the session (from its `workflow.started`); `criteria`: the
/// run's folded criteria. Tokens are not here: the usage ledger holds them.
pub fn run_stats(run: &[Event], criteria: &[CriterionState], files_changed: usize) -> RunStats {
    let workflow = run.iter().find_map(|event| match event {
        Event::WorkflowStarted { workflow, .. } => Some(workflow),
        _ => None,
    });

    let mut nodes: Vec<NodeStats> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for event in run {
        match event {
            Event::WorkflowNodeStarted { node_id, .. } => {
                if let Some(i) = index.get(node_id) {
                    nodes[*i].runs += 1;
                } else {
                    let label = workflow
                        .and_then(|workflow| workflow.node(node_id))
                        .map(|node| node.label().to_string())
                        .unwrap_or_else(|| node_id.clone());
                    index.insert(node_id.clone(), nodes.len());
                    nodes.push(NodeStats {
                        node_id: node_id.clone(),
                        label,
                        runs: 1,
                        working_ms: 0,
                    });
                }
            }
            Event::WorkflowNodeFinished {
                node_id,
                duration_ms,
                ..
            } => {
                if let Some(ms) = duration_ms
                    && let Some(i) = index.get(node_id)
                {
                    nodes[*i].working_ms += ms;
                }
            }
            _ => {}
        }
    }

    let working_ms = nodes.iter().map(|node| node.working_ms).sum();
    let reruns = nodes.iter().map(|node| node.runs.saturating_sub(1)).sum();

    let mut criteria_total = 0u32;
    let mut criteria_passed = 0u32;
    let mut criteria_verified = 0u32;
    for state in criteria {
        criteria_total += 1;
        if let CriterionStatus::Passed { provenance } = &state.status {
            criteria_passed += 1;
            if matches!(provenance, Provenance::Verified { .. }) {
                criteria_verified += 1;
            }
        }
    }

    RunStats {
        working_ms,
        nodes,
        reruns,
        files_changed: files_changed as u32,
        criteria_total,
        criteria_passed,
        criteria_verified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::criteria::Criterion;

    #[test]
    fn run_stats_counts_runs_time_and_criteria() {
        let run = vec![
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 1,
                via: None,
                child_session_id: Some("child".into()),
                started_at: None,
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "a".into(),
                port: "done".into(),
                output: String::new(),
                exit_code: None,
                comment: None,
                duration_ms: Some(100),
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "b".into(),
                pass: 1,
                via: None,
                child_session_id: None,
                started_at: None,
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "b".into(),
                port: "yes".into(),
                output: "yes".into(),
                exit_code: None,
                comment: None,
                duration_ms: None,
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 2,
                via: None,
                child_session_id: None,
                started_at: None,
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "a".into(),
                port: "done".into(),
                output: String::new(),
                exit_code: None,
                comment: None,
                duration_ms: Some(50),
            },
        ];
        let criteria = vec![
            CriterionState {
                criterion: Criterion {
                    id: "C1".into(),
                    description: "one".into(),
                    check: None,
                },
                status: CriterionStatus::Passed {
                    provenance: Provenance::Verified {
                        command: "c".into(),
                        exit_code: Some(0),
                        output: String::new(),
                    },
                },
            },
            CriterionState {
                criterion: Criterion {
                    id: "C2".into(),
                    description: "two".into(),
                    check: None,
                },
                status: CriterionStatus::Passed {
                    provenance: Provenance::Indicative {
                        agent: "a".into(),
                        reason: String::new(),
                    },
                },
            },
            CriterionState {
                criterion: Criterion {
                    id: "C3".into(),
                    description: "three".into(),
                    check: None,
                },
                status: CriterionStatus::Failed {
                    provenance: Provenance::Verified {
                        command: "c".into(),
                        exit_code: Some(1),
                        output: String::new(),
                    },
                },
            },
        ];

        let stats = run_stats(&run, &criteria, 2);

        assert_eq!(stats.working_ms, 150);
        assert_eq!(
            stats.nodes,
            vec![
                NodeStats {
                    node_id: "a".into(),
                    label: "a".into(),
                    runs: 2,
                    working_ms: 150,
                },
                NodeStats {
                    node_id: "b".into(),
                    label: "b".into(),
                    runs: 1,
                    working_ms: 0,
                }
            ]
        );
        assert_eq!(stats.reruns, 1);
        assert_eq!(stats.files_changed, 2);
        assert_eq!(stats.criteria_total, 3);
        assert_eq!(stats.criteria_passed, 2);
        assert_eq!(stats.criteria_verified, 1);
    }
}
