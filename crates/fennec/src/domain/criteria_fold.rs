//! The criteria in force, folded from the log: the last `criteria.set` and the status updates since. Shared by a workflow run's state and the served thread view, so both read the same contract.

use super::criteria::{CriterionState, CriterionStatus, Provenance};
use super::event::Event;
use serde::Serialize;

/// How the criteria in force stand: the header's figure and its colour.
/// `indicative` counts the passes an agent judged rather than fennec checked.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tally {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub indicative: usize,
    /// The passes' share of the criteria, 0 to 100, the bar's width.
    pub percent: u8,
}

pub fn apply(criteria: &mut Vec<CriterionState>, event: &Event) {
    match event {
        // A new criteria.set is the source of truth; older statuses no longer apply.
        Event::CriteriaSet { criteria: set } => {
            *criteria = set
                .iter()
                .map(|criterion| CriterionState {
                    criterion: criterion.clone(),
                    status: CriterionStatus::Pending,
                })
                .collect();
        }
        // Status updates only affect criteria present in the current set.
        Event::CriterionUpdated {
            criterion_id,
            status,
        } => {
            if let Some(state) = criteria
                .iter_mut()
                .find(|state| state.criterion.id == *criterion_id)
            {
                state.status = status.clone();
            }
        }
        _ => {}
    }
}

pub fn in_force(events: &[Event]) -> Vec<CriterionState> {
    // Fold the log so the latest set and subsequent updates are the only state kept.
    events.iter().fold(Vec::new(), |mut criteria, event| {
        apply(&mut criteria, event);
        criteria
    })
}

pub fn tally(criteria: &[CriterionState]) -> Tally {
    let mut tally = criteria.iter().fold(Tally::default(), |mut tally, state| {
        tally.total += 1;
        match &state.status {
            CriterionStatus::Pending => {}
            CriterionStatus::Passed { provenance } => {
                tally.passed += 1;
                if matches!(provenance, Provenance::Indicative { .. }) {
                    tally.indicative += 1;
                }
            }
            CriterionStatus::Failed { .. } => tally.failed += 1,
        }
        tally
    });
    tally.percent = (tally.passed * 100)
        .checked_div(tally.total)
        .map_or(0, |share| share as u8);
    tally
}

#[cfg(test)]
mod tests {
    use super::{Tally, apply, in_force, tally};
    use crate::domain::criteria::{Criterion, CriterionState, CriterionStatus, Provenance};
    use crate::domain::event::Event;

    #[test]
    fn set_replaces_the_list_as_pending() {
        let mut criteria = vec![CriterionState {
            criterion: Criterion {
                id: "C1".into(),
                description: "old".into(),
                check: None,
            },
            status: CriterionStatus::Passed {
                provenance: Provenance::Indicative {
                    agent: "verify".into(),
                    reason: "ok".into(),
                },
            },
        }];

        apply(
            &mut criteria,
            &Event::CriteriaSet {
                criteria: vec![Criterion {
                    id: "C2".into(),
                    description: "new".into(),
                    check: None,
                }],
            },
        );

        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].criterion.id, "C2");
        assert_eq!(criteria[0].status, CriterionStatus::Pending);
    }

    #[test]
    fn updated_sets_the_status_of_the_matching_id() {
        let mut criteria = vec![CriterionState {
            criterion: Criterion {
                id: "C1".into(),
                description: "d".into(),
                check: None,
            },
            status: CriterionStatus::Pending,
        }];

        apply(
            &mut criteria,
            &Event::CriterionUpdated {
                criterion_id: "C1".into(),
                status: CriterionStatus::Passed {
                    provenance: Provenance::Indicative {
                        agent: "verify".into(),
                        reason: "ok".into(),
                    },
                },
            },
        );

        assert_eq!(
            criteria[0].status,
            CriterionStatus::Passed {
                provenance: Provenance::Indicative {
                    agent: "verify".into(),
                    reason: "ok".into(),
                },
            }
        );
    }

    #[test]
    fn updated_ignores_an_unknown_id() {
        let mut criteria = vec![CriterionState {
            criterion: Criterion {
                id: "C1".into(),
                description: "d".into(),
                check: None,
            },
            status: CriterionStatus::Pending,
        }];

        apply(
            &mut criteria,
            &Event::CriterionUpdated {
                criterion_id: "C2".into(),
                status: CriterionStatus::Passed {
                    provenance: Provenance::Indicative {
                        agent: "verify".into(),
                        reason: "ok".into(),
                    },
                },
            },
        );

        assert_eq!(criteria[0].status, CriterionStatus::Pending);
    }

    #[test]
    fn in_force_folds_the_whole_log() {
        let events = vec![
            Event::CriteriaSet {
                criteria: vec![Criterion {
                    id: "C1".into(),
                    description: "d".into(),
                    check: None,
                }],
            },
            Event::CriterionUpdated {
                criterion_id: "C1".into(),
                status: CriterionStatus::Passed {
                    provenance: Provenance::Indicative {
                        agent: "verify".into(),
                        reason: "ok".into(),
                    },
                },
            },
        ];

        let criteria = in_force(&events);

        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].criterion.id, "C1");
        assert_eq!(
            criteria[0].status,
            CriterionStatus::Passed {
                provenance: Provenance::Indicative {
                    agent: "verify".into(),
                    reason: "ok".into(),
                },
            }
        );
    }

    #[test]
    fn tally_counts_passes_failures_and_indicative_passes() {
        let state = |id: &str, status: CriterionStatus| CriterionState {
            criterion: Criterion {
                id: id.into(),
                description: "d".into(),
                check: None,
            },
            status,
        };
        let verified = Provenance::Verified {
            command: "true".into(),
            exit_code: Some(0),
            output: String::new(),
        };
        let judged = Provenance::Indicative {
            agent: "verify".into(),
            reason: "ok".into(),
        };
        let criteria = vec![
            state("C1", CriterionStatus::Pending),
            state(
                "C2",
                CriterionStatus::Passed {
                    provenance: verified.clone(),
                },
            ),
            state("C3", CriterionStatus::Passed { provenance: judged }),
            state(
                "C4",
                CriterionStatus::Failed {
                    provenance: verified,
                },
            ),
        ];

        assert_eq!(
            tally(&criteria),
            Tally {
                total: 4,
                passed: 2,
                failed: 1,
                indicative: 1,
                percent: 50,
            }
        );
    }
}
