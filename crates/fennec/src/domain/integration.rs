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
    /// The human's checked-out branch is not the one the run started from
    /// (`to` None: a detached HEAD); the commits would land elsewhere.
    BranchChanged { from: String, to: Option<String> },
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
            Blocker::BranchChanged { .. } => "branchChanged",
            Blocker::CriteriaNotPassed { .. } => "criteriaNotPassed",
            Blocker::NoCriteria => "noCriteria",
            Blocker::ProtectedTouched { .. } => "protectedTouched",
        }
    }
}

/// The blockers of an integration, in the order the UI shows them:
/// `NoCommits` first, then the branch, then the criteria, then the protected
/// files.
pub fn blockers(
    commits: usize,
    criteria: &[CriterionState],
    protected_touched: &[String],
    origin_branch: Option<&str>,
    human_branch: Option<&str>,
) -> Vec<Blocker> {
    let mut out = Vec::new();

    if commits == 0 {
        out.push(Blocker::NoCommits);
    }

    if let Some(origin) = origin_branch
        && human_branch != Some(origin)
    {
        out.push(Blocker::BranchChanged {
            from: origin.to_string(),
            to: human_branch.map(str::to_string),
        });
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

/// A starting commit message for the integration: `feat: <title>`. Fennec
/// cannot tell a `feat` from a `fix`; the human edits it before confirming.
pub fn default_message(session_title: &str) -> String {
    let title = session_title.trim();
    if title.is_empty() {
        return "feat: integrate the agent's work".to_string();
    }
    let mut chars = title.chars();
    let first = chars.next().unwrap().to_lowercase().to_string();
    format!("feat: {first}{}", chars.collect::<String>())
}

/// The integration commit message: the human's message plus the session's
/// provenance trailer.
pub fn commit_message(message: &str, session_id: &str) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("empty commit message".to_string());
    }
    Ok(format!("{message}\n\nFennec-Session: {session_id}"))
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
            Blocker::BranchChanged {
                from: "main".into(),
                to: Some("feature".into())
            }
            .key(),
            "branchChanged"
        );
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

        let blockers = blockers(0, &criteria, &protected, None, None);

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
        assert_eq!(blockers(1, &[], &[], None, None), vec![Blocker::NoCriteria]);
    }

    #[test]
    fn indicative_passes_count_as_passed() {
        let criteria = vec![
            state("C1", indicative_passed()),
            state("C2", verified_failed()),
        ];

        let blockers = blockers(1, &criteria, &[], None, None);

        assert_eq!(
            blockers,
            vec![Blocker::CriteriaNotPassed {
                criteria: vec!["C2".into()]
            }]
        );
    }

    /// The branch blocker alone: the other blockers are tested above.
    fn branch_blocker(origin: Option<&str>, human: Option<&str>) -> Option<Blocker> {
        blockers(1, &[], &[], origin, human)
            .into_iter()
            .find(|blocker| matches!(blocker, Blocker::BranchChanged { .. }))
    }

    #[test]
    fn a_changed_branch_blocks() {
        let blocker = branch_blocker(Some("main"), Some("feature"));

        assert_eq!(
            blocker,
            Some(Blocker::BranchChanged {
                from: "main".into(),
                to: Some("feature".into())
            })
        );
        let blockers = vec![blocker.unwrap()];
        assert!(unacknowledged(&blockers, &["branchChanged".to_string()]).is_empty());
    }

    #[test]
    fn the_same_branch_does_not_block() {
        assert_eq!(branch_blocker(Some("main"), Some("main")), None);
    }

    #[test]
    fn no_origin_does_not_block() {
        assert_eq!(branch_blocker(None, Some("main")), None);
    }

    #[test]
    fn a_detached_head_blocks() {
        assert_eq!(
            branch_blocker(Some("main"), None),
            Some(Blocker::BranchChanged {
                from: "main".into(),
                to: None
            })
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

    #[test]
    fn default_message_uses_the_session_title() {
        assert_eq!(default_message("  Fix login "), "feat: fix login");
    }

    #[test]
    fn default_message_for_an_empty_title_is_a_plain_integrate() {
        assert_eq!(default_message("   "), "feat: integrate the agent's work");
    }

    #[test]
    fn commit_message_adds_the_session_trailer() {
        assert_eq!(
            commit_message("  Fix login \n", "s1"),
            Ok("Fix login\n\nFennec-Session: s1".to_string())
        );
    }

    #[test]
    fn commit_message_refuses_a_blank_message() {
        assert_eq!(
            commit_message("   ", "s1"),
            Err("empty commit message".to_string())
        );
    }
}
