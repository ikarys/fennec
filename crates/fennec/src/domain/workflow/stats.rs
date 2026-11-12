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
    /// The run's wall-clock start, read from its `workflow.started`. `None` in
    /// logs written before the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    /// The run's wall-clock end, set when it finishes. `None` in logs written
    /// before the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    pub nodes: Vec<NodeStats>,
    /// Node runs beyond the first, summed: how much the run looped.
    pub reruns: u32,
    pub files_changed: u32,
    pub criteria_total: u32,
    pub criteria_passed: u32,
    pub criteria_verified: u32,
}

/// One pass of a node on the run's timeline, offset from the run's start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSpan {
    pub offset_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStats {
    pub node_id: String,
    pub label: String,
    pub runs: u32,
    pub working_ms: u64,
    /// Passes on the run's timeline; empty in logs written before the field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<NodeSpan>,
}

/// `run`: the run's events in the session (from its `workflow.started`); `criteria`: the
/// run's folded criteria. Tokens are not here: the usage ledger holds them.
pub fn run_stats(run: &[Event], criteria: &[CriterionState], files_changed: usize) -> RunStats {
    let start = run.iter().find_map(|event| match event {
        Event::WorkflowStarted {
            workflow,
            started_at,
            ..
        } => Some((workflow, *started_at)),
        _ => None,
    });
    let workflow = start.map(|(workflow, _)| workflow);
    let started_at = start.and_then(|(_, started_at)| started_at);

    let mut nodes: Vec<NodeStats> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut pending_offsets: HashMap<String, u64> = HashMap::new();

    for event in run {
        match event {
            Event::WorkflowNodeStarted {
                node_id,
                started_at: node_started_at,
                ..
            } => {
                if let Some(run_started) = started_at
                    && let Some(node_started) = *node_started_at
                {
                    pending_offsets
                        .insert(node_id.clone(), node_started.saturating_sub(run_started));
                }

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
                        spans: Vec::new(),
                    });
                }
            }
            Event::WorkflowNodeFinished {
                node_id,
                duration_ms,
                ..
            } => {
                if let Some(i) = index.get(node_id) {
                    if let Some(ms) = duration_ms {
                        nodes[*i].working_ms += ms;
                    }
                    if let Some(offset_ms) = pending_offsets.remove(node_id) {
                        nodes[*i].spans.push(NodeSpan {
                            offset_ms,
                            duration_ms: duration_ms.unwrap_or(0),
                        });
                    }
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
        started_at,
        finished_at: None,
        nodes,
        reruns,
        files_changed: files_changed as u32,
        criteria_total,
        criteria_passed,
        criteria_verified,
    }
}

/// Fills the node spans of stats recorded before they existed, from the run's
/// events; stats that already carry spans are left as they are.
pub fn fill_missing_spans(stats: &mut RunStats, run: &[Event]) {
    if stats.nodes.iter().any(|node| !node.spans.is_empty()) {
        return;
    }

    let rebuilt = run_stats(run, &[], 0);
    for node in rebuilt.nodes {
        if let Some(existing) = stats
            .nodes
            .iter_mut()
            .find(|existing| existing.node_id == node.node_id)
        {
            existing.spans = node.spans;
        }
    }
}

/// A session node's working time: its agent turns' durations since its last
/// start, so the waits for the human's replies between them do not count.
pub fn node_turns_ms(events: &[Event], node_id: &str) -> u64 {
    let Some(start) = events.iter().rposition(
        |event| matches!(event, Event::WorkflowNodeStarted { node_id: id, .. } if id == node_id),
    ) else {
        return 0;
    };
    events[start + 1..]
        .iter()
        .filter_map(|event| match event {
            Event::ChatDone { duration_ms, .. } => *duration_ms,
            _ => None,
        })
        .sum()
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
                    spans: vec![],
                },
                NodeStats {
                    node_id: "b".into(),
                    label: "b".into(),
                    runs: 1,
                    working_ms: 0,
                    spans: vec![],
                }
            ]
        );
        assert_eq!(stats.reruns, 1);
        assert_eq!(stats.files_changed, 2);
        assert_eq!(stats.criteria_total, 3);
        assert_eq!(stats.criteria_passed, 2);
        assert_eq!(stats.criteria_verified, 1);
    }

    #[test]
    fn run_stats_carries_the_run_wall_clock() {
        let run = vec![Event::WorkflowStarted {
            run_id: "r".into(),
            workflow: crate::domain::workflow::definition::Workflow {
                id: "w".into(),
                name: "w".into(),
                description: String::new(),
                max_steps: 2,
                nodes: vec![],
                edges: vec![],
            },
            task: String::new(),
            step_by_step: false,
            started_at: Some(1_000),
        }];
        let stats = run_stats(&run, &[], 0);
        // The start is read from the run's `workflow.started`; the end is set
        // at finish time, not by the fold.
        assert_eq!(stats.started_at, Some(1_000));
        assert_eq!(stats.finished_at, None);
    }

    #[test]
    fn run_stats_places_node_passes_on_the_timeline() {
        let run = vec![
            Event::WorkflowStarted {
                run_id: "r".into(),
                workflow: crate::domain::workflow::definition::Workflow {
                    id: "w".into(),
                    name: "w".into(),
                    description: String::new(),
                    max_steps: 2,
                    nodes: vec![],
                    edges: vec![],
                },
                task: String::new(),
                step_by_step: false,
                started_at: Some(1_000),
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 1,
                via: None,
                child_session_id: None,
                started_at: Some(1_100),
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "a".into(),
                port: "done".into(),
                output: String::new(),
                exit_code: None,
                comment: None,
                duration_ms: Some(200),
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 2,
                via: None,
                child_session_id: None,
                started_at: Some(2_000),
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

        let stats = run_stats(&run, &[], 0);

        assert_eq!(stats.nodes.len(), 1);
        assert_eq!(
            stats.nodes[0].spans,
            vec![
                NodeSpan {
                    offset_ms: 100,
                    duration_ms: 200
                },
                NodeSpan {
                    offset_ms: 1_000,
                    duration_ms: 50
                }
            ]
        );
    }

    #[test]
    fn fill_missing_spans_rebuilds_the_timeline_of_an_old_run() {
        let run = vec![
            Event::WorkflowStarted {
                run_id: "r".into(),
                workflow: crate::domain::workflow::definition::Workflow {
                    id: "w".into(),
                    name: "w".into(),
                    description: String::new(),
                    max_steps: 2,
                    nodes: vec![],
                    edges: vec![],
                },
                task: String::new(),
                step_by_step: false,
                started_at: Some(1_000),
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 1,
                via: None,
                child_session_id: None,
                started_at: Some(1_100),
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "a".into(),
                port: "done".into(),
                output: String::new(),
                exit_code: None,
                comment: None,
                duration_ms: Some(200),
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 2,
                via: None,
                child_session_id: None,
                started_at: Some(2_000),
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

        let mut stats = RunStats {
            working_ms: 250,
            started_at: Some(1_000),
            finished_at: Some(2_050),
            nodes: vec![NodeStats {
                node_id: "a".into(),
                label: "a".into(),
                runs: 2,
                working_ms: 250,
                spans: vec![],
            }],
            reruns: 1,
            files_changed: 0,
            criteria_total: 0,
            criteria_passed: 0,
            criteria_verified: 0,
        };

        fill_missing_spans(&mut stats, &run);

        assert_eq!(
            stats.nodes[0].spans,
            vec![
                NodeSpan {
                    offset_ms: 100,
                    duration_ms: 200
                },
                NodeSpan {
                    offset_ms: 1_000,
                    duration_ms: 50
                }
            ]
        );

        let existing = NodeSpan {
            offset_ms: 7,
            duration_ms: 8,
        };
        let mut old = RunStats {
            working_ms: 8,
            started_at: Some(1_000),
            finished_at: Some(1_008),
            nodes: vec![NodeStats {
                node_id: "a".into(),
                label: "a".into(),
                runs: 1,
                working_ms: 8,
                spans: vec![existing.clone()],
            }],
            reruns: 0,
            files_changed: 0,
            criteria_total: 0,
            criteria_passed: 0,
            criteria_verified: 0,
        };

        fill_missing_spans(&mut old, &run);

        assert_eq!(old.nodes[0].spans, vec![existing]);
    }

    #[test]
    fn node_turns_ms_sums_the_chat_turns_after_the_last_node_start() {
        let events = vec![
            chat_done(Some(999)),
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 1,
                via: None,
                child_session_id: None,
                started_at: None,
            },
            chat_done(Some(100)),
            chat_done(Some(50)),
            chat_done(None),
        ];
        assert_eq!(node_turns_ms(&events, "a"), 150);
        assert_eq!(node_turns_ms(&events, "b"), 0);
    }

    fn chat_done(duration_ms: Option<u64>) -> Event {
        Event::ChatDone {
            message_id: String::new(),
            reason: crate::domain::event::DoneReason::Complete,
            duration_ms,
        }
    }
}
