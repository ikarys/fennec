//! fennec's own (openfox has no integration step): the review gate, what the
//! human must knowingly accept before integrating.

use serde::Serialize;

use super::architecture::{Rules, Violation};
use super::bypass::{self, Suppression, TestsWeakened};
use super::criteria::{CriterionState, CriterionStatus};
use super::diff::ChangedLines;

/// What the branch's diff tells the gate, checked on its lines.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiffChecks {
    pub violations: Vec<Violation>,
    pub suppressions: Vec<Suppression>,
    pub weakened: Option<TestsWeakened>,
}

impl DiffChecks {
    /// `files`: (path relative to the project, its changed lines).
    pub fn of(rules: &Rules, files: &[(String, ChangedLines)]) -> DiffChecks {
        let mut violations = Vec::new();
        let mut suppressions = Vec::new();

        for (path, lines) in files {
            violations.extend(rules.check(path, &lines.added));
            suppressions.extend(bypass::suppressions(path, &lines.added));
        }

        DiffChecks {
            violations,
            suppressions,
            weakened: bypass::tests_weakened(files),
        }
    }
}

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
    /// Uncommitted changes in the human's tree that the cherry-pick would
    /// overwrite; git would refuse the pick.
    LocalChanges { paths: Vec<String> },
    /// The run's criteria that did not pass (failed or still pending); ids.
    CriteriaNotPassed { criteria: Vec<String> },
    /// The run has no criteria at all: nothing was checked.
    NoCriteria,
    /// Protected files the branch changes (a command can create one: the file
    /// tools cannot).
    ProtectedTouched { paths: Vec<String> },
    /// Added lines that break the project's `.agents/architecture` rules.
    ArchitectureViolated { violations: Vec<Violation> },
    /// Added lines that silence a lint, a type check or a test.
    SuppressionsAdded { suppressions: Vec<Suppression> },
    /// The branch removes more tests or assertions than it adds.
    TestsWeakened {
        tests_removed: usize,
        assertions_removed: usize,
        paths: Vec<String>,
    },
}

impl Blocker {
    /// The key the human acknowledges in the UI.
    pub fn key(&self) -> &'static str {
        match self {
            Blocker::NoCommits => "noCommits",
            Blocker::BranchChanged { .. } => "branchChanged",
            Blocker::LocalChanges { .. } => "localChanges",
            Blocker::CriteriaNotPassed { .. } => "criteriaNotPassed",
            Blocker::NoCriteria => "noCriteria",
            Blocker::ProtectedTouched { .. } => "protectedTouched",
            Blocker::ArchitectureViolated { .. } => "architectureViolated",
            Blocker::SuppressionsAdded { .. } => "suppressionsAdded",
            Blocker::TestsWeakened { .. } => "testsWeakened",
        }
    }
}

/// True when the human's branch is not the one the run started from.
pub fn branch_changed(origin_branch: Option<&str>, human_branch: Option<&str>) -> bool {
    matches!(origin_branch, Some(origin) if human_branch != Some(origin))
}

/// The blockers of an integration, in the order the UI shows them:
/// `NoCommits` first, then the branch, then the human's uncommitted changes,
/// then the criteria, then the protected files, then the architecture rules,
/// then the added suppressions, then the weakened tests.
pub fn blockers(
    commits: usize,
    criteria: &[CriterionState],
    protected_touched: &[String],
    checks: &DiffChecks,
    origin_branch: Option<&str>,
    human_branch: Option<&str>,
    local_changes: &[String],
) -> Vec<Blocker> {
    let mut out = Vec::new();

    if commits == 0 {
        out.push(Blocker::NoCommits);
    }

    if let Some(origin) = origin_branch
        && branch_changed(Some(origin), human_branch)
    {
        out.push(Blocker::BranchChanged {
            from: origin.to_string(),
            to: human_branch.map(str::to_string),
        });
    }

    if !local_changes.is_empty() {
        out.push(Blocker::LocalChanges {
            paths: local_changes.to_vec(),
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

    if !checks.violations.is_empty() {
        out.push(Blocker::ArchitectureViolated {
            violations: checks.violations.clone(),
        });
    }

    if !checks.suppressions.is_empty() {
        out.push(Blocker::SuppressionsAdded {
            suppressions: checks.suppressions.clone(),
        });
    }

    if let Some(weakened) = &checks.weakened {
        out.push(Blocker::TestsWeakened {
            tests_removed: weakened.tests_removed,
            assertions_removed: weakened.assertions_removed,
            paths: weakened.paths.clone(),
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
    // Framing sessions are titled "Plan: …", and the integration commit
    // inherited "feat: plan: …".
    let title = match title.split_once(':') {
        Some((head, rest)) if head.trim().eq_ignore_ascii_case("plan") => rest.trim(),
        _ => title,
    };
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
            Blocker::LocalChanges {
                paths: vec!["a.rs".into()]
            }
            .key(),
            "localChanges"
        );
        assert_eq!(
            Blocker::ProtectedTouched {
                paths: vec!["AGENTS.md".into()]
            }
            .key(),
            "protectedTouched"
        );
        assert_eq!(
            Blocker::SuppressionsAdded {
                suppressions: vec![Suppression {
                    path: "a.rs".into(),
                    line: "#[allow(x)]".into(),
                    marker: "#[allow(".into(),
                }]
            }
            .key(),
            "suppressionsAdded"
        );
        assert_eq!(
            Blocker::TestsWeakened {
                tests_removed: 1,
                assertions_removed: 0,
                paths: vec!["a.rs".into()],
            }
            .key(),
            "testsWeakened"
        );
    }

    #[test]
    fn architecture_violations_block_after_protected_files() {
        let violation = Violation {
            path: "a.rs".into(),
            line: "use std::fs;".into(),
            prefix: "".into(),
            forbidden: "std::fs".into(),
        };
        let checks = DiffChecks {
            violations: vec![violation.clone()],
            ..Default::default()
        };

        let found = blockers(1, &[], &[], &checks, None, None, &[]);

        assert_eq!(
            found,
            vec![
                Blocker::NoCriteria,
                Blocker::ArchitectureViolated {
                    violations: vec![violation]
                }
            ]
        );
    }

    #[test]
    fn suppressions_and_weakened_tests_block_after_architecture() {
        let violation = Violation {
            path: "a.rs".into(),
            line: "use std::fs;".into(),
            prefix: "".into(),
            forbidden: "std::fs".into(),
        };
        let suppression = Suppression {
            path: "a.rs".into(),
            line: "#[allow(x)]".into(),
            marker: "#[allow(".into(),
        };
        let checks = DiffChecks {
            violations: vec![violation.clone()],
            suppressions: vec![suppression.clone()],
            weakened: Some(TestsWeakened {
                tests_removed: 1,
                assertions_removed: 0,
                paths: vec!["a.rs".into()],
            }),
        };

        let found = blockers(1, &[], &[], &checks, None, None, &[]);

        assert_eq!(
            found,
            vec![
                Blocker::NoCriteria,
                Blocker::ArchitectureViolated {
                    violations: vec![violation]
                },
                Blocker::SuppressionsAdded {
                    suppressions: vec![suppression]
                },
                Blocker::TestsWeakened {
                    tests_removed: 1,
                    assertions_removed: 0,
                    paths: vec!["a.rs".into()],
                },
            ]
        );
    }

    #[test]
    fn diff_checks_of_collects_violations_suppressions_and_weakened() {
        let rules = Rules::parse("src/ forbids std::fs");
        let files = vec![(
            "src/a.rs".to_string(),
            ChangedLines {
                added: vec![
                    String::from("use std::fs;"),
                    String::from("#[allow(unused)]"),
                ],
                removed: vec![String::from("#[test]")],
            },
        )];

        let checks = DiffChecks::of(&rules, &files);

        assert_eq!(checks.violations.len(), 1);
        assert_eq!(checks.violations[0].forbidden, "std::fs");
        assert_eq!(checks.suppressions.len(), 1);
        assert_eq!(checks.suppressions[0].marker, "#[allow(");
        let weakened = checks.weakened.unwrap();
        assert_eq!(weakened.tests_removed, 1);
        assert_eq!(weakened.paths, vec!["src/a.rs".to_string()]);
    }

    #[test]
    fn blockers_are_ordered_no_commits_then_criteria_then_protected() {
        let criteria = vec![state("C1", verified_failed())];
        let protected = vec!["AGENTS.md".to_string()];

        let blockers = blockers(
            0,
            &criteria,
            &protected,
            &DiffChecks::default(),
            None,
            None,
            &[],
        );

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
        assert_eq!(
            blockers(1, &[], &[], &DiffChecks::default(), None, None, &[]),
            vec![Blocker::NoCriteria]
        );
    }

    #[test]
    fn uncommitted_changes_block_after_the_branch_and_before_the_criteria() {
        let local = vec!["a.rs".to_string()];
        let criteria = vec![state("C1", verified_failed())];

        let found = blockers(
            1,
            &criteria,
            &[],
            &DiffChecks::default(),
            Some("main"),
            Some("main"),
            &local,
        );
        assert_eq!(
            found,
            vec![
                Blocker::LocalChanges {
                    paths: vec!["a.rs".to_string()]
                },
                Blocker::CriteriaNotPassed {
                    criteria: vec!["C1".to_string()]
                },
            ]
        );

        let found = blockers(
            1,
            &[],
            &[],
            &DiffChecks::default(),
            Some("main"),
            Some("feature"),
            &local,
        );
        assert_eq!(
            found[0],
            Blocker::BranchChanged {
                from: "main".into(),
                to: Some("feature".into())
            }
        );
        assert_eq!(found[1], Blocker::LocalChanges { paths: local });
    }

    #[test]
    fn an_acknowledged_local_changes_blocker_is_dropped() {
        let blockers = vec![Blocker::LocalChanges {
            paths: vec!["a.rs".to_string()],
        }];

        assert!(unacknowledged(&blockers, &["localChanges".to_string()]).is_empty());
        assert_eq!(unacknowledged(&blockers, &[]).len(), 1);
    }

    #[test]
    fn indicative_passes_count_as_passed() {
        let criteria = vec![
            state("C1", indicative_passed()),
            state("C2", verified_failed()),
        ];

        let blockers = blockers(1, &criteria, &[], &DiffChecks::default(), None, None, &[]);

        assert_eq!(
            blockers,
            vec![Blocker::CriteriaNotPassed {
                criteria: vec!["C2".into()]
            }]
        );
    }

    /// The branch blocker alone: the other blockers are tested above.
    fn branch_blocker(origin: Option<&str>, human: Option<&str>) -> Option<Blocker> {
        blockers(1, &[], &[], &DiffChecks::default(), origin, human, &[])
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
    fn branch_changed_is_true_only_when_the_human_branch_differs() {
        assert!(!branch_changed(None, Some("main")));
        assert!(!branch_changed(Some("main"), Some("main")));
        assert!(branch_changed(Some("main"), Some("feature")));
        assert!(branch_changed(Some("main"), None));
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
    fn default_message_drops_the_plan_prefix() {
        assert_eq!(
            default_message("Plan: Project pages vertical tabs"),
            "feat: project pages vertical tabs"
        );
        assert_eq!(default_message("plan : x"), "feat: x");
        assert_eq!(default_message("Plan:"), "feat: integrate the agent's work");
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
