//! The review gate: what the human must knowingly accept before
//! integrating.

use serde::Serialize;

use super::architecture::{Rules, Violation};
use super::bypass::{self, Suppression, TestsWeakened};
use super::criteria::{CriterionState, CriterionStatus};
use super::diff::ChangedLines;
use super::security_scan::SecurityFinding;

/// What the branch's diff tells the gate, checked on its lines.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiffChecks {
    pub violations: Vec<Violation>,
    pub suppressions: Vec<Suppression>,
    pub weakened: Option<TestsWeakened>,
    /// Files whose added lines still hold git conflict markers: a resolution
    /// left unfinished, which git no longer sees as a conflict.
    pub markers: Vec<String>,
    /// Changed files inside the human's sensitive zones.
    pub sensitive: Vec<String>,
    /// Changed files no step of the approved plan lists (set by the adapter,
    /// which reads the run's plan).
    pub outside_plan: Vec<String>,
    /// The security scanners' hits on the branch (filled by the adapter that
    /// ran them: the diff alone cannot tell).
    pub security: Vec<SecurityFinding>,
    /// A scanner still runs: `security` is not complete yet.
    pub scanning: bool,
    /// A comprehension dialogue was held on the branch's current head (set by
    /// the adapter, like `scanning`).
    pub understood: bool,
}

impl DiffChecks {
    /// `files`: (path relative to the project, its changed lines).
    pub fn of(rules: &Rules, files: &[(String, ChangedLines)]) -> DiffChecks {
        let mut violations = Vec::new();
        let mut suppressions = Vec::new();
        let mut markers = Vec::new();
        let mut sensitive = Vec::new();

        for (path, lines) in files {
            violations.extend(rules.check(path, &lines.added));
            suppressions.extend(bypass::suppressions(path, &lines.added));
            if lines.added.iter().any(|line| is_conflict_marker(line)) {
                markers.push(path.clone());
            }
            if rules.is_sensitive(path) {
                sensitive.push(path.clone());
            }
        }

        DiffChecks {
            violations,
            suppressions,
            weakened: bypass::tests_weakened(files),
            markers,
            sensitive,
            outside_plan: Vec::new(),
            security: Vec::new(),
            scanning: false,
            understood: false,
        }
    }
}

/// `<<<<<<< ours` or `>>>>>>> theirs` (or bare): the lines git writes around
/// a conflict. `=======` alone is left out: Markdown and RST use it.
fn is_conflict_marker(line: &str) -> bool {
    ["<<<<<<<", ">>>>>>>"].iter().any(|marker| {
        line.strip_prefix(marker)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
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
    /// The human's branch changed the same lines since the run started: the
    /// merge would fail.
    Conflict { files: Vec<String> },
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
    /// Files in the sensitive zones of `.agents/architecture`: the human's
    /// review is mandatory there.
    SensitiveTouched { paths: Vec<String> },
    /// Changed files no step of the approved plan lists: the human did not
    /// approve them in the plan.
    OutsidePlan { paths: Vec<String> },
    /// The branch changes a sensitive zone and the human held no
    /// comprehension dialogue on its current head: the definition makes it
    /// mandatory there.
    UnderstandingMissing,
    /// A security scanner still runs: its findings are unknown, so nothing
    /// may be integrated until it ends.
    SecurityScanRunning,
    /// What the security scanners found on the branch: secrets, vulnerable
    /// dependencies, the human's own Semgrep rules.
    SecurityFindings { findings: Vec<SecurityFinding> },
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
            Blocker::Conflict { .. } => "conflict",
            Blocker::BranchChanged { .. } => "branchChanged",
            Blocker::LocalChanges { .. } => "localChanges",
            Blocker::CriteriaNotPassed { .. } => "criteriaNotPassed",
            Blocker::NoCriteria => "noCriteria",
            Blocker::ProtectedTouched { .. } => "protectedTouched",
            Blocker::SensitiveTouched { .. } => "sensitiveTouched",
            Blocker::OutsidePlan { .. } => "outsidePlan",
            Blocker::UnderstandingMissing => "understandingMissing",
            Blocker::SecurityScanRunning => "securityScanRunning",
            Blocker::SecurityFindings { .. } => "securityFindings",
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

/// Where the work would land: the human's tree as git sees it at review time.
#[derive(Debug, Clone, Copy, Default)]
pub struct Landing<'a> {
    /// The branch the run started from, and the one checked out now (None:
    /// a detached HEAD).
    pub origin_branch: Option<&'a str>,
    pub human_branch: Option<&'a str>,
    /// Uncommitted changes in the human's tree the agent's work would overwrite.
    pub local_changes: &'a [String],
    /// Files the merge onto the human's HEAD would conflict on.
    pub conflicts: &'a [String],
}

/// The blockers of an integration, in the order the UI shows them:
/// `NoCommits` first, then a merge conflict, then the branch, then the
/// human's uncommitted changes, then the criteria, then the protected files,
/// then the sensitive zones, then files outside the plan, then a missing
/// comprehension dialogue, then a security scan still running, then its
/// findings, then the architecture rules, then the added suppressions, then
/// the weakened tests.
pub fn blockers(
    commits: usize,
    criteria: &[CriterionState],
    protected_touched: &[String],
    checks: &DiffChecks,
    landing: &Landing,
) -> Vec<Blocker> {
    let Landing {
        origin_branch,
        human_branch,
        local_changes,
        conflicts,
    } = *landing;
    let mut out = Vec::new();

    if commits == 0 {
        out.push(Blocker::NoCommits);
    }

    // Markers left in the files are a conflict too: integrating them would
    // commit a broken file, and only a resolution clears them.
    let mut conflicted = conflicts.to_vec();
    for file in &checks.markers {
        if !conflicted.contains(file) {
            conflicted.push(file.clone());
        }
    }
    if !conflicted.is_empty() {
        out.push(Blocker::Conflict { files: conflicted });
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

    if !checks.sensitive.is_empty() {
        out.push(Blocker::SensitiveTouched {
            paths: checks.sensitive.clone(),
        });
    }
    if !checks.outside_plan.is_empty() {
        out.push(Blocker::OutsidePlan {
            paths: checks.outside_plan.clone(),
        });
    }
    if !checks.sensitive.is_empty() && !checks.understood {
        out.push(Blocker::UnderstandingMissing);
    }

    if checks.scanning {
        out.push(Blocker::SecurityScanRunning);
    }

    if !checks.security.is_empty() {
        out.push(Blocker::SecurityFindings {
            findings: checks.security.clone(),
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

/// The blockers the human did not acknowledge (by key). `NoCommits`,
/// `Conflict` and `LocalChanges` can never be acknowledged: there is nothing
/// to integrate, accepting a conflict would not make the merge succeed, and
/// the pick refuses to overwrite uncommitted work.
pub fn unacknowledged<'a>(blockers: &'a [Blocker], acknowledged: &[String]) -> Vec<&'a Blocker> {
    blockers
        .iter()
        .filter(|blocker| match blocker {
            Blocker::NoCommits
            | Blocker::Conflict { .. }
            | Blocker::LocalChanges { .. }
            | Blocker::SecurityScanRunning => true,
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

/// The integration commit message: the human's message, the session's
/// provenance trailer, and one `Assisted-by: fennec:<model>` trailer per
/// answering model.
pub fn commit_message(
    message: &str,
    session_id: &str,
    models: &[String],
) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("empty commit message".to_string());
    }

    let mut out = format!("{message}\n\nFennec-Session: {session_id}");

    // Sanitize each model so an untrusted id cannot break the one-line trailer.
    for model in models {
        let model = sanitize_model(model);
        if model.is_empty() {
            continue;
        }
        out.push_str("\nAssisted-by: fennec:");
        out.push_str(&model);
    }

    Ok(out)
}

/// Reduce an untrusted model id to a safe trailer name.
///
/// It drops trailer-breaking characters and surrounding whitespace, and the
/// caller skips it if that leaves no name.
fn sanitize_model(model: &str) -> String {
    // Drop characters that would let an untrusted id change the trailer's shape.
    let cleaned: String = model
        .chars()
        .filter(|c| !matches!(c, '<' | '>' | '\r' | '\n'))
        .collect();
    cleaned.trim().to_string()
}

/// The prompt sent to the agent after `update_branch` left conflict markers.
pub fn conflict_prompt(human_branch: &str, files: &[String]) -> String {
    let list = files
        .iter()
        .map(|file| format!("- {file}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your branch was brought up to date with `{human_branch}`, the human's branch. The human changed the same lines as your work in these files, which now hold git conflict markers (<<<<<<<, =======, >>>>>>>):\n{list}\n\nResolve every conflict: keep the intent of both sides, remove every marker, then run the project's checks. Do not run git: fennec commits your work when the turn ends."
    )
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
            Blocker::SensitiveTouched {
                paths: vec!["src/auth/login.rs".into()]
            }
            .key(),
            "sensitiveTouched"
        );
        assert_eq!(Blocker::UnderstandingMissing.key(), "understandingMissing");
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

        let found = blockers(1, &[], &[], &checks, &Landing::default());

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
        let finding = SecurityFinding {
            scanner: "gitleaks".into(),
            rule: "github-pat".into(),
            path: "a.rs".into(),
            line: Some(3),
            detail: "GitHub token".into(),
        };
        let checks = DiffChecks {
            violations: vec![violation.clone()],
            suppressions: vec![suppression.clone()],
            weakened: Some(TestsWeakened {
                tests_removed: 1,
                assertions_removed: 0,
                paths: vec!["a.rs".into()],
            }),
            markers: vec![],
            sensitive: vec![],
            outside_plan: Vec::new(),
            security: vec![finding.clone()],
            scanning: true,
            understood: false,
        };

        let found = blockers(1, &[], &[], &checks, &Landing::default());

        assert_eq!(
            found,
            vec![
                Blocker::NoCriteria,
                Blocker::SecurityScanRunning,
                Blocker::SecurityFindings {
                    findings: vec![finding]
                },
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
    fn sensitive_files_block_and_are_listed() {
        let rules = Rules::parse("sensitive src/auth/");
        let files = vec![(
            "src/auth/login.rs".to_string(),
            ChangedLines {
                added: vec![String::from("fn login() {}")],
                removed: vec![],
            },
        )];

        let checks = DiffChecks::of(&rules, &files);
        assert_eq!(checks.sensitive, vec!["src/auth/login.rs".to_string()]);

        let found = blockers(1, &[], &[], &checks, &Landing::default());
        assert!(found.contains(&Blocker::SensitiveTouched {
            paths: vec!["src/auth/login.rs".to_string()],
        }));
    }

    #[test]
    fn a_sensitive_branch_without_a_dialogue_blocks() {
        let rules = Rules::parse("sensitive src/auth/");
        let files = vec![(
            "src/auth/login.rs".to_string(),
            ChangedLines {
                added: vec![String::from("fn login() {}")],
                removed: vec![],
            },
        )];
        let checks = DiffChecks::of(&rules, &files);

        let found = blockers(1, &[], &[], &checks, &Landing::default());

        assert!(found.contains(&Blocker::SensitiveTouched {
            paths: vec!["src/auth/login.rs".to_string()],
        }));
        assert!(found.contains(&Blocker::UnderstandingMissing));
    }

    #[test]
    fn a_sensitive_branch_with_a_dialogue_does_not_block_for_understanding() {
        let rules = Rules::parse("sensitive src/auth/");
        let files = vec![(
            "src/auth/login.rs".to_string(),
            ChangedLines {
                added: vec![String::from("fn login() {}")],
                removed: vec![],
            },
        )];
        let mut checks = DiffChecks::of(&rules, &files);
        checks.understood = true;

        let found = blockers(1, &[], &[], &checks, &Landing::default());

        assert!(found.contains(&Blocker::SensitiveTouched {
            paths: vec!["src/auth/login.rs".to_string()],
        }));
        assert!(!found.contains(&Blocker::UnderstandingMissing));
    }

    #[test]
    fn a_branch_with_no_sensitive_file_never_blocks_for_understanding() {
        let found = blockers(1, &[], &[], &DiffChecks::default(), &Landing::default());

        assert!(!found.contains(&Blocker::UnderstandingMissing));
    }

    #[test]
    fn outside_plan_blocker_is_pushed_when_non_empty() {
        let checks = DiffChecks {
            outside_plan: vec!["src/a.rs".into()],
            ..Default::default()
        };

        let found = blockers(1, &[], &[], &checks, &Landing::default());

        assert!(found.contains(&Blocker::OutsidePlan {
            paths: vec!["src/a.rs".into()],
        }));
    }

    #[test]
    fn outside_plan_blocker_is_absent_when_empty() {
        let found = blockers(1, &[], &[], &DiffChecks::default(), &Landing::default());

        assert!(!found.contains(&Blocker::OutsidePlan { paths: Vec::new() }));
    }

    #[test]
    fn outside_plan_has_its_key() {
        assert_eq!(
            Blocker::OutsidePlan {
                paths: vec!["a.rs".into()]
            }
            .key(),
            "outsidePlan"
        );
    }

    #[test]
    fn outside_plan_is_pushed_after_sensitive_and_before_understanding() {
        let rules = Rules::parse("sensitive src/auth/");
        let files = vec![(
            "src/auth/login.rs".to_string(),
            ChangedLines {
                added: vec![String::from("fn login() {}")],
                removed: vec![],
            },
        )];
        let mut checks = DiffChecks::of(&rules, &files);
        checks.outside_plan = vec!["src/extra.rs".into()];

        let found = blockers(1, &[], &[], &checks, &Landing::default());

        let sensitive_pos = found
            .iter()
            .position(|blocker| matches!(blocker, Blocker::SensitiveTouched { .. }));
        let outside_pos = found
            .iter()
            .position(|blocker| matches!(blocker, Blocker::OutsidePlan { .. }));
        let understanding_pos = found
            .iter()
            .position(|blocker| matches!(blocker, Blocker::UnderstandingMissing));

        assert!(sensitive_pos.is_some());
        assert!(outside_pos.is_some());
        assert!(understanding_pos.is_some());
        assert!(outside_pos > sensitive_pos);
        assert!(understanding_pos > outside_pos);
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
            &Landing::default(),
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
            blockers(1, &[], &[], &DiffChecks::default(), &Landing::default()),
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
            &Landing {
                origin_branch: Some("main"),
                human_branch: Some("main"),
                local_changes: &local,
                ..Landing::default()
            },
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
            &Landing {
                origin_branch: Some("main"),
                human_branch: Some("feature"),
                local_changes: &local,
                ..Landing::default()
            },
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
    fn a_local_changes_blocker_is_never_acknowledged() {
        let blockers = vec![Blocker::LocalChanges {
            paths: vec!["a.rs".to_string()],
        }];

        assert_eq!(
            unacknowledged(&blockers, &["localChanges".to_string()]).len(),
            1
        );
    }

    #[test]
    fn a_running_security_scan_is_never_acknowledged() {
        let found = [Blocker::SecurityScanRunning];

        assert_eq!(
            unacknowledged(&found, &["securityScanRunning".into()]),
            vec![&Blocker::SecurityScanRunning]
        );
    }

    #[test]
    fn indicative_passes_count_as_passed() {
        let criteria = vec![
            state("C1", indicative_passed()),
            state("C2", verified_failed()),
        ];

        let blockers = blockers(
            1,
            &criteria,
            &[],
            &DiffChecks::default(),
            &Landing::default(),
        );

        assert_eq!(
            blockers,
            vec![Blocker::CriteriaNotPassed {
                criteria: vec!["C2".into()]
            }]
        );
    }

    /// The branch blocker alone: the other blockers are tested above.
    fn branch_blocker(origin: Option<&str>, human: Option<&str>) -> Option<Blocker> {
        blockers(
            1,
            &[],
            &[],
            &DiffChecks::default(),
            &Landing {
                origin_branch: origin,
                human_branch: human,
                ..Landing::default()
            },
        )
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
    fn conflict_blocker_has_its_key() {
        assert_eq!(
            Blocker::Conflict {
                files: vec!["a.txt".into()]
            }
            .key(),
            "conflict"
        );
    }

    #[test]
    fn a_conflict_is_listed_first_after_no_commits() {
        let criteria = vec![state("C1", indicative_passed())];
        let conflicts = vec!["a.txt".to_string()];

        let found = blockers(
            0,
            &criteria,
            &[],
            &DiffChecks::default(),
            &Landing {
                origin_branch: Some("main"),
                human_branch: Some("feature"),
                conflicts: &conflicts,
                ..Landing::default()
            },
        );

        assert_eq!(
            found,
            vec![
                Blocker::NoCommits,
                Blocker::Conflict {
                    files: vec!["a.txt".into()]
                },
                Blocker::BranchChanged {
                    from: "main".into(),
                    to: Some("feature".into())
                },
            ]
        );
    }

    #[test]
    fn a_conflict_stays_unacknowledged_even_if_acknowledged() {
        let blockers = vec![Blocker::Conflict {
            files: vec!["a.txt".into()],
        }];

        assert_eq!(
            unacknowledged(&blockers, &["conflict".to_string()]),
            vec![&Blocker::Conflict {
                files: vec!["a.txt".into()]
            }]
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
            commit_message("  Fix login \n", "s1", &[]),
            Ok("Fix login\n\nFennec-Session: s1".to_string())
        );
    }

    #[test]
    fn commit_message_refuses_a_blank_message() {
        assert_eq!(
            commit_message("   ", "s1", &[]),
            Err("empty commit message".to_string())
        );
    }

    #[test]
    fn commit_message_adds_one_trailer_per_model_in_order() {
        let models = vec!["qwen3.8-27b".to_string(), "claude-sonnet".to_string()];
        let expected = [
            "Fix",
            "",
            "Fennec-Session: s1",
            "Assisted-by: fennec:qwen3.8-27b",
            "Assisted-by: fennec:claude-sonnet",
        ]
        .join("\n");
        assert_eq!(commit_message("Fix", "s1", &models), Ok(expected));
    }

    #[test]
    fn commit_message_sanitizes_a_model_with_newlines_and_angle_brackets() {
        let models = vec!["  <q\nwen>  ".to_string()];
        let expected = ["Fix", "", "Fennec-Session: s1", "Assisted-by: fennec:qwen"].join("\n");
        assert_eq!(commit_message("Fix", "s1", &models), Ok(expected));
    }

    #[test]
    fn commit_message_skips_a_model_blank_after_sanitizing() {
        let models = vec!["<>\r\n".to_string()];
        let expected = ["Fix", "", "Fennec-Session: s1"].join("\n");
        assert_eq!(commit_message("Fix", "s1", &models), Ok(expected));
    }

    #[test]
    fn conflict_markers_left_in_added_lines_block_as_a_conflict() {
        let lines = ChangedLines {
            added: vec!["<<<<<<< HEAD".into(), "a".into(), "=======".into()],
            removed: vec![],
        };
        let clean = ChangedLines {
            added: vec!["=======".into(), "<<<<<<<<< not a marker".into()],
            removed: vec![],
        };
        let checks = DiffChecks::of(
            &Rules::default(),
            &[("a.rs".into(), lines), ("doc.md".into(), clean)],
        );
        assert_eq!(checks.markers, vec!["a.rs".to_string()]);
        let found = blockers(1, &[], &[], &checks, &Landing::default());
        assert!(found.contains(&Blocker::Conflict {
            files: vec!["a.rs".into()]
        }));
    }

    #[test]
    fn conflict_prompt_names_the_branch_and_each_file() {
        let prompt = conflict_prompt("main", &["a.txt".into(), "b.rs".into()]);

        assert!(prompt.contains("`main`"));
        assert!(prompt.contains("- a.txt"));
        assert!(prompt.contains("- b.rs"));
    }
}
