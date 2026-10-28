//! Facts about a finished run, computed from the run's events.

use crate::domain::criteria::{CriterionState, CriterionStatus, Provenance};
use crate::domain::event::Event;
use crate::domain::stats::NodeTokens;
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
    /// Model calls (`message.done` with stats) in the session during the run and in its child sessions.
    pub calls: u32,
    pub prefill_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub generation_tokens: u64,
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

/// `run`: the run's events in the session (from its `workflow.started`); `children`: the
/// events of each child session the run created; `criteria`: the run's folded criteria.
pub fn run_stats(
    run: &[Event],
    children: &[Vec<Event>],
    criteria: &[CriterionState],
    files_changed: usize,
) -> RunStats {
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

    let mut calls = 0u32;
    let mut prefill_tokens = 0u64;
    let mut cached_prompt_tokens = 0u64;
    let mut generation_tokens = 0u64;

    let mut count = |events: &[Event]| {
        for event in events {
            if let Event::MessageDone {
                stats: Some(stats), ..
            } = event
            {
                calls += 1;
                prefill_tokens += stats.prefill_tokens;
                cached_prompt_tokens += stats.cached_prompt_tokens;
                generation_tokens += stats.generation_tokens;
            }
        }
    };
    count(run);
    for child in children {
        count(child);
    }

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
        calls,
        prefill_tokens,
        cached_prompt_tokens,
        generation_tokens,
        files_changed: files_changed as u32,
        criteria_total,
        criteria_passed,
        criteria_verified,
    }
}

/// The model cost of a node's run: the sum of its calls' stats; None when the
/// node made no model call (shell, condition, criteria, human).
pub fn node_tokens(events: &[Event]) -> Option<NodeTokens> {
    let mut prefill_tokens = 0u64;
    let mut generation_tokens = 0u64;
    let mut weighted_speed = 0.0f64;
    let mut any = false;

    for event in events {
        if let Event::MessageDone {
            stats: Some(stats), ..
        } = event
        {
            any = true;
            prefill_tokens += stats.prefill_tokens;
            generation_tokens += stats.generation_tokens;
            weighted_speed += stats.generation_speed * stats.generation_tokens as f64;
        }
    }

    if !any {
        return None;
    }
    Some(NodeTokens {
        prefill_tokens,
        generation_tokens,
        generation_speed: if generation_tokens > 0 {
            weighted_speed / generation_tokens as f64
        } else {
            0.0
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::criteria::Criterion;
    use crate::domain::stats::MessageStats;

    fn stats(prefill: u64, cached: u64, generation: u64) -> MessageStats {
        MessageStats {
            model: "m".into(),
            total_time: 0.0,
            prefill_tokens: prefill,
            cached_prompt_tokens: cached,
            generation_tokens: generation,
            prefill_speed: 0.0,
            generation_speed: 0.0,
        }
    }

    #[test]
    fn run_stats_counts_runs_time_calls_and_criteria() {
        let run = vec![
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 1,
                via: None,
                child_session_id: Some("child".into()),
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "a".into(),
                port: "done".into(),
                output: String::new(),
                exit_code: None,
                comment: None,
                duration_ms: Some(100),
                stats: None,
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "b".into(),
                pass: 1,
                via: None,
                child_session_id: None,
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "b".into(),
                port: "yes".into(),
                output: "yes".into(),
                exit_code: None,
                comment: None,
                duration_ms: None,
                stats: None,
            },
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 2,
                via: None,
                child_session_id: None,
            },
            Event::WorkflowNodeFinished {
                run_id: "r".into(),
                node_id: "a".into(),
                port: "done".into(),
                output: String::new(),
                exit_code: None,
                comment: None,
                duration_ms: Some(50),
                stats: None,
            },
        ];
        let child = vec![Event::MessageDone {
            message_id: "m".into(),
            stats: Some(stats(10, 4, 2)),
            partial: false,
        }];
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

        let stats = run_stats(&run, &[child], &criteria, 2);

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
        assert_eq!(stats.calls, 1);
        assert_eq!(stats.prefill_tokens, 10);
        assert_eq!(stats.cached_prompt_tokens, 4);
        assert_eq!(stats.generation_tokens, 2);
        assert_eq!(stats.files_changed, 2);
        assert_eq!(stats.criteria_total, 3);
        assert_eq!(stats.criteria_passed, 2);
        assert_eq!(stats.criteria_verified, 1);
    }

    #[test]
    fn node_tokens_sums_the_node_calls_and_weights_the_speed() {
        use crate::domain::stats::NodeTokens;

        let call = |prefill: u64, generation: u64, speed: f64| Event::MessageDone {
            message_id: "m".into(),
            stats: Some(stats_with_speed(prefill, generation, speed)),
            partial: false,
        };
        let events = vec![
            Event::WorkflowNodeStarted {
                run_id: "r".into(),
                node_id: "a".into(),
                pass: 1,
                via: None,
                child_session_id: None,
            },
            call(100, 10, 20.0),
            call(50, 30, 40.0),
        ];

        assert_eq!(
            node_tokens(&events),
            Some(NodeTokens {
                prefill_tokens: 150,
                generation_tokens: 40,
                // (10*20 + 30*40) / 40
                generation_speed: 35.0,
            })
        );
        assert_eq!(node_tokens(&[]), None);
    }

    fn stats_with_speed(prefill: u64, generation: u64, speed: f64) -> MessageStats {
        MessageStats {
            model: "m".into(),
            total_time: 0.0,
            prefill_tokens: prefill,
            cached_prompt_tokens: 0,
            generation_tokens: generation,
            prefill_speed: 0.0,
            generation_speed: speed,
        }
    }
}
