//! fennec's own (openfox has no integration step): the review gate, what the
//! human must knowingly accept before integrating.

use serde::Serialize;

use super::criteria::{CriterionState, CriterionStatus};

/// What blocks an integration, shown to the human before they confirm it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Blocker {
    /// Nothing to integrate.
    NoCommits,
    /// The run's criteria that did not pass (failed or still pending); ids.
    CriteriaNotPassed { criteria: Vec<String> },
    /// The run has no criteria at all: nothing was checked.
    NoCriteria,
    /// Protected files the branch changes (a command can create one: the file
    /// tools cannot).
    ProtectedTouched { paths: Vec<String> },
}

impl Blocker {
    /// The key the human acknowledges in the UI.
    pub fn key(&self) -> &'static str {
        match self {
            Blocker::NoCommits => "noCommits",
            Blocker::CriteriaNotPassed { .. } => "criteriaNotPassed",
            Blocker::NoCriteria => "noCriteria",
            Blocker::ProtectedTouched { .. } => "protectedTouched",
        }
    }
}

/// The blockers of an integration, in the order the UI shows them:
/// `NoCommits` first, then the criteria, then the protected files.
pub fn blockers(
    commits: usize,
    criteria: &[CriterionState],
    protected_touched: &[String],
) -> Vec<Blocker> {
    let mut out = Vec::new();

    if commits == 0 {
        out.push(Blocker::NoCommits);
    }

    if criteria.is_empty() {
        out.push(Blocker::NoCriteria);
    } else {
        let not_passed: Vec<String> = criteria
            .iter()
            .filter(|state| !matches!(state.status, CriterionStatus::Passed { .. }))
            .map(|state| state.criterion.id.clone())
            .collect();
        if !not_passed.is_empty() {
            out.push(Blocker::CriteriaNotPassed {
                criteria: not_passed,
            });
        }
    }

    if !protected_touched.is_empty() {
        out.push(Blocker::ProtectedTouched {
            paths: protected_touched.to_vec(),
        });
    }

    out
}

/// The blockers the human did not acknowledge (by key). `NoCommits` can
/// never be acknowledged: there is nothing to integrate.
pub fn unacknowledged<'a>(blockers: &'a [Blocker], acknowledged: &[String]) -> Vec<&'a Blocker> {
    blockers
        .iter()
        .filter(|blocker| match blocker {
            Blocker::NoCommits => true,
            _ => !acknowledged.contains(&blocker.key().to_string()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::criteria::{Criterion, Provenance};

    fn state(id: &str, status: CriterionStatus) -> CriterionState {
        CriterionState {
            criterion: Criterion {
                id: id.to_string(),
                description: "description".to_string(),
                check: None,
            },
            status,
        }
    }

    fn indicative_passed() -> CriterionStatus {
        CriterionStatus::Passed {
            provenance: Provenance::Indicative {
                agent: "builder".into(),
                reason: "looks right".into(),
            },
        }
    }

    fn verified_failed() -> CriterionStatus {
        CriterionStatus::Failed {
            provenance: Provenance::Verified {
                command: "just test".into(),
                exit_code: Some(1),
                output: "boom".into(),
            },
        }
    }

    #[test]
    fn each_blocker_has_its_key() {
        assert_eq!(Blocker::NoCommits.key(), "noCommits");
        assert_eq!(
            Blocker::CriteriaNotPassed {
                criteria: vec!["C1".into()]
            }
            .key(),
            "criteriaNotPassed"
        );
        assert_eq!(Blocker::NoCriteria.key(), "noCriteria");
        assert_eq!(
            Blocker::ProtectedTouched {
                paths: vec!["AGENTS.md".into()]
            }
            .key(),
            "protectedTouched"
        );
    }

    #[test]
    fn blockers_are_ordered_no_commits_then_criteria_then_protected() {
        let criteria = vec![state("C1", verified_failed())];
        let protected = vec!["AGENTS.md".to_string()];

        let blockers = blockers(0, &criteria, &protected);

        assert_eq!(
            blockers,
            vec![
                Blocker::NoCommits,
                Blocker::CriteriaNotPassed {
                    criteria: vec!["C1".into()]
                },
                Blocker::ProtectedTouched {
                    paths: vec!["AGENTS.md".into()]
                },
            ]
        );
    }

    #[test]
    fn no_criteria_is_a_blocker() {
        assert_eq!(blockers(1, &[], &[]), vec![Blocker::NoCriteria]);
    }

    #[test]
    fn indicative_passes_count_as_passed() {
        let criteria = vec![
            state("C1", indicative_passed()),
            state("C2", verified_failed()),
        ];

        let blockers = blockers(1, &criteria, &[]);

        assert_eq!(
            blockers,
            vec![Blocker::CriteriaNotPassed {
                criteria: vec!["C2".into()]
            }]
        );
    }

    #[test]
    fn unacknowledged_keeps_everything_when_nothing_is_acknowledged() {
        let blockers = vec![
            Blocker::NoCommits,
            Blocker::CriteriaNotPassed {
                criteria: vec!["C1".into()],
            },
            Blocker::ProtectedTouched {
                paths: vec!["AGENTS.md".into()],
            },
        ];

        assert_eq!(
            unacknowledged(&blockers, &[]),
            vec![
                &Blocker::NoCommits,
                &Blocker::CriteriaNotPassed {
                    criteria: vec!["C1".into()]
                },
                &Blocker::ProtectedTouched {
                    paths: vec!["AGENTS.md".into()]
                },
            ]
        );
    }

    #[test]
    fn no_commits_can_never_be_acknowledged() {
        let blockers = vec![
            Blocker::NoCommits,
            Blocker::CriteriaNotPassed {
                criteria: vec!["C1".into()],
            },
            Blocker::ProtectedTouched {
                paths: vec!["AGENTS.md".into()],
            },
        ];
        let acknowledged = vec![
            "noCommits".to_string(),
            "criteriaNotPassed".to_string(),
            "protectedTouched".to_string(),
        ];

        assert_eq!(
            unacknowledged(&blockers, &acknowledged),
            vec![&Blocker::NoCommits]
        );
    }

    #[test]
    fn acknowledged_blockers_are_dropped() {
        let blockers = vec![
            Blocker::NoCommits,
            Blocker::CriteriaNotPassed {
                criteria: vec!["C1".into()],
            },
            Blocker::ProtectedTouched {
                paths: vec!["AGENTS.md".into()],
            },
        ];
        let acknowledged = vec!["criteriaNotPassed".to_string()];

        assert_eq!(
            unacknowledged(&blockers, &acknowledged),
            vec![
                &Blocker::NoCommits,
                &Blocker::ProtectedTouched {
                    paths: vec!["AGENTS.md".into()]
                }
            ]
        );
    }
}
