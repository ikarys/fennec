//! The run's verification block: what was verified, folded from the log.

use crate::domain::criteria::{CriterionStatus, Provenance};
use crate::domain::event::Event;
use crate::domain::plan_scope;
use crate::domain::test_files;
use crate::domain::workflow::run;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    /// The run these figures are for: the session's latest; None before any run.
    pub run_id: Option<String>,
    pub criteria: Vec<CheckedCriterion>,
    /// The test files the human validated (protected from then on).
    pub tests_protected: usize,
    pub waivers_asked: usize,
    pub waivers_granted: usize,
    /// The changed files the plan did not name; None when the branch is not known (no worktree).
    pub outside_plan: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckedCriterion {
    pub id: String,
    pub description: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Outcome {
    /// fennec ran the check.
    Verified {
        command: String,
        exit_code: Option<i32>,
        passed: bool,
    },
    /// A model judged it: indicative.
    Judged {
        passed: bool,
    },
    Pending,
}

/// The current run's verification, folded from the session's events; `changed` = the branch's changed paths, project-relative, when known.
pub fn of(events: &[Event], changed: Option<&[String]>) -> Verification {
    let run = run::current_run(events);
    let run_id = run.as_ref().map(|state| state.run_id.clone());
    let criteria = run
        .map(|state| state.criteria)
        .unwrap_or_default()
        .into_iter()
        .map(|state| CheckedCriterion {
            id: state.criterion.id,
            description: state.criterion.description,
            outcome: outcome_of(&state.status),
        })
        .collect();
    let tests_protected = test_files::approved(events).len();
    let waivers_asked = events
        .iter()
        .filter(|event| matches!(event, Event::WaiverRequested { .. }))
        .count();
    let waivers_granted = events
        .iter()
        .filter(|event| matches!(event, Event::WaiverAnswered { granted: true, .. }))
        .count();
    let outside_plan = changed.map(|changed| outside_plan(events, changed));

    Verification {
        run_id,
        criteria,
        tests_protected,
        waivers_asked,
        waivers_granted,
        outside_plan,
    }
}

/// The changed paths outside the plan's contract (plan files + validated tests), as the review gate computes them.
pub fn outside_plan(events: &[Event], changed: &[String]) -> Vec<String> {
    let plan = run::current_run(events)
        .as_ref()
        .and_then(run::plan_output)
        .map(plan_scope::plan_files)
        .unwrap_or_default();
    let validated: Vec<String> = test_files::approved(events)
        .iter()
        .map(|path| path.trim_start_matches("./").to_string())
        .collect();
    let contract = plan_scope::contract(plan, &validated);
    plan_scope::outside(&contract, changed)
}

impl Verification {
    /// The block for the commit body and the Copy button; None when the run has no criteria.
    pub fn block(&self) -> Option<String> {
        if self.criteria.is_empty() {
            return None;
        }

        let mut lines = vec!["Verified by fennec:".to_string()];

        if self.criteria.len() > 8 {
            for criterion in &self.criteria[..7] {
                lines.push(criterion_line(criterion));
            }
            let (passed, failed, pending) = outcome_counts(&self.criteria[7..]);
            lines.push(format!(
                "- … and {} more criteria: {}",
                self.criteria.len() - 7,
                summary_parts(passed, failed, pending)
            ));
        } else {
            for criterion in &self.criteria {
                lines.push(criterion_line(criterion));
            }
        }

        lines.push(self.last_line());
        Some(lines.join("\n"))
    }

    fn last_line(&self) -> String {
        let waivers = if self.waivers_asked == 0 || self.waivers_granted == self.waivers_asked {
            format!("waivers: {}", self.waivers_asked)
        } else {
            format!(
                "waivers: {} of {} granted",
                self.waivers_granted, self.waivers_asked
            )
        };

        let mut line = format!(
            "Tests approved in the run: {}, {waivers}.",
            self.tests_protected
        );

        if let Some(outside) = &self.outside_plan {
            line.push_str(&format!(
                " Files outside the plan: {}.",
                outside_text(outside)
            ));
        }

        line
    }
}

/// `message`, a blank line, then the block; the message alone without a block.
pub fn with_block(message: &str, block: Option<&str>) -> String {
    match block {
        Some(block) => format!("{message}\n\n{block}"),
        None => message.to_string(),
    }
}

fn outcome_of(status: &CriterionStatus) -> Outcome {
    match status {
        CriterionStatus::Pending => Outcome::Pending,
        CriterionStatus::Passed { provenance } => match provenance {
            Provenance::Verified {
                command, exit_code, ..
            } => Outcome::Verified {
                command: command.clone(),
                exit_code: *exit_code,
                passed: true,
            },
            Provenance::Indicative { .. } => Outcome::Judged { passed: true },
        },
        CriterionStatus::Failed { provenance } => match provenance {
            Provenance::Verified {
                command, exit_code, ..
            } => Outcome::Verified {
                command: command.clone(),
                exit_code: *exit_code,
                passed: false,
            },
            Provenance::Indicative { .. } => Outcome::Judged { passed: false },
        },
    }
}

fn criterion_line(criterion: &CheckedCriterion) -> String {
    let id = one_line(&criterion.id);
    let description = one_line(&criterion.description);
    let outcome = match &criterion.outcome {
        Outcome::Verified {
            command,
            exit_code,
            passed,
        } => {
            let command = one_line(command);
            if *passed {
                format!("`{command}` passed")
            } else {
                match exit_code {
                    Some(code) => format!("`{command}` failed (exit {code})"),
                    None => format!("`{command}` failed"),
                }
            }
        }
        Outcome::Judged { passed } => {
            if *passed {
                "judged by the model, passed".to_string()
            } else {
                "judged by the model, failed".to_string()
            }
        }
        Outcome::Pending => "not checked".to_string(),
    };
    format!("- {id} {description}: {outcome}")
}

fn outcome_counts(criteria: &[CheckedCriterion]) -> (usize, usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    let mut pending = 0;

    for criterion in criteria {
        match &criterion.outcome {
            Outcome::Verified { passed: true, .. } | Outcome::Judged { passed: true } => {
                passed += 1;
            }
            Outcome::Verified { passed: false, .. } | Outcome::Judged { passed: false } => {
                failed += 1;
            }
            Outcome::Pending => pending += 1,
        }
    }

    (passed, failed, pending)
}

fn summary_parts(passed: usize, failed: usize, pending: usize) -> String {
    let mut parts = Vec::new();
    if passed > 0 {
        parts.push(format!("{passed} passed"));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    if pending > 0 {
        parts.push(format!("{pending} not checked"));
    }
    parts.join(", ")
}

fn outside_text(outside: &[String]) -> String {
    if outside.is_empty() {
        "none".to_string()
    } else if outside.len() <= 3 {
        outside.join(", ")
    } else {
        format!("{} and {} more", outside[..3].join(", "), outside.len() - 3)
    }
}

/// One line of untrusted model text.
///
/// Line-injection: a model-written criterion forging extra lines in the human's commit.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_renders_the_example() {
        let verification = Verification {
            run_id: None,
            criteria: vec![
                CheckedCriterion {
                    id: "C1".into(),
                    description: "add_todo stores the item".into(),
                    outcome: Outcome::Verified {
                        command: "cargo test add_todo_stores".into(),
                        exit_code: Some(0),
                        passed: true,
                    },
                },
                CheckedCriterion {
                    id: "C2".into(),
                    description: "list_todos returns the list".into(),
                    outcome: Outcome::Verified {
                        command: "cargo test list_todos_returns".into(),
                        exit_code: Some(1),
                        passed: false,
                    },
                },
                CheckedCriterion {
                    id: "C3".into(),
                    description: "docs updated".into(),
                    outcome: Outcome::Judged { passed: true },
                },
                CheckedCriterion {
                    id: "C4".into(),
                    description: "something".into(),
                    outcome: Outcome::Pending,
                },
            ],
            tests_protected: 2,
            waivers_asked: 0,
            waivers_granted: 0,
            outside_plan: Some(Vec::new()),
        };

        assert_eq!(
            verification.block(),
            Some(
                "Verified by fennec:\n\
                 - C1 add_todo stores the item: `cargo test add_todo_stores` passed\n\
                 - C2 list_todos returns the list: `cargo test list_todos_returns` failed (exit 1)\n\
                 - C3 docs updated: judged by the model, passed\n\
                 - C4 something: not checked\n\
                 Tests approved in the run: 2, waivers: 0. Files outside the plan: none."
                    .to_string()
            )
        );
    }

    #[test]
    fn block_is_none_without_criteria() {
        let verification = Verification {
            run_id: None,
            criteria: Vec::new(),
            tests_protected: 1,
            waivers_asked: 0,
            waivers_granted: 0,
            outside_plan: Some(Vec::new()),
        };

        assert_eq!(verification.block(), None);
    }

    #[test]
    fn block_waivers_wording() {
        let partial = Verification {
            run_id: None,
            criteria: vec![CheckedCriterion {
                id: "C1".into(),
                description: "x".into(),
                outcome: Outcome::Pending,
            }],
            tests_protected: 0,
            waivers_asked: 3,
            waivers_granted: 1,
            outside_plan: Some(Vec::new()),
        };
        assert!(partial.block().unwrap().contains("waivers: 1 of 3 granted"));

        let all_granted = Verification {
            waivers_asked: 2,
            waivers_granted: 2,
            ..partial.clone()
        };
        assert!(all_granted.block().unwrap().contains("waivers: 2"));

        let none = Verification {
            waivers_asked: 0,
            waivers_granted: 0,
            ..partial
        };
        assert!(none.block().unwrap().contains("waivers: 0"));
    }

    #[test]
    fn block_outside_list_over_three() {
        let verification = Verification {
            run_id: None,
            criteria: vec![CheckedCriterion {
                id: "C1".into(),
                description: "x".into(),
                outcome: Outcome::Pending,
            }],
            tests_protected: 0,
            waivers_asked: 0,
            waivers_granted: 0,
            outside_plan: Some(vec![
                "a".into(),
                "b".into(),
                "c".into(),
                "d".into(),
                "e".into(),
            ]),
        };

        assert!(
            verification
                .block()
                .unwrap()
                .contains("Files outside the plan: a, b, c and 2 more.")
        );
    }

    #[test]
    fn block_without_outside_plan_ends_after_waivers() {
        let verification = Verification {
            run_id: None,
            criteria: vec![CheckedCriterion {
                id: "C1".into(),
                description: "x".into(),
                outcome: Outcome::Pending,
            }],
            tests_protected: 2,
            waivers_asked: 0,
            waivers_granted: 0,
            outside_plan: None,
        };

        assert!(
            verification
                .block()
                .unwrap()
                .ends_with("Tests approved in the run: 2, waivers: 0.")
        );
    }

    #[test]
    fn block_caps_at_ten_lines_with_twelve_criteria() {
        let criteria = (0..12)
            .map(|index| CheckedCriterion {
                id: format!("C{}", index + 1),
                description: format!("c{index}"),
                outcome: Outcome::Pending,
            })
            .collect();
        let verification = Verification {
            run_id: None,
            criteria,
            tests_protected: 0,
            waivers_asked: 0,
            waivers_granted: 0,
            outside_plan: Some(Vec::new()),
        };

        let block = verification.block().unwrap();
        assert_eq!(block.lines().count(), 10);
        assert!(block.contains("- … and 5 more criteria: 5 not checked"));
    }

    #[test]
    fn block_keeps_a_newline_description_on_one_line() {
        let verification = Verification {
            run_id: None,
            criteria: vec![CheckedCriterion {
                id: "C1".into(),
                description: "line1\nline2".into(),
                outcome: Outcome::Verified {
                    command: "cmd\nx".into(),
                    exit_code: None,
                    passed: true,
                },
            }],
            tests_protected: 0,
            waivers_asked: 0,
            waivers_granted: 0,
            outside_plan: Some(Vec::new()),
        };

        let block = verification.block().unwrap();
        assert_eq!(block.lines().count(), 3);
        assert!(block.contains("- C1 line1 line2: `cmd x` passed"));
    }

    #[test]
    fn with_block_appends_a_blank_line_and_block() {
        assert_eq!(with_block("message", Some("block")), "message\n\nblock");
        assert_eq!(with_block("message", None), "message");
    }

    #[test]
    fn the_integration_commit_keeps_the_block_before_the_trailers() {
        let message = with_block(
            "feat: add todos",
            Some("Verified by fennec:\n- C1 x: not checked"),
        );
        let commit =
            crate::domain::integration::commit_message(&message, "s1", &["qwen".into()]).unwrap();
        assert_eq!(
            commit,
            "feat: add todos\n\nVerified by fennec:\n- C1 x: not checked\n\nFennec-Session: s1\nAssisted-by: fennec:qwen"
        );
    }

    #[test]
    fn of_counts_waivers_and_keeps_changed_unknown() {
        let events = vec![
            Event::WaiverRequested {
                call_id: "c".into(),
                path: "p".into(),
                reason: "r".into(),
            },
            Event::WaiverAnswered {
                call_id: "c".into(),
                granted: true,
                expires_at: None,
                adr: None,
            },
        ];

        let verification = of(&events, Some(&[] as &[String]));
        assert_eq!(verification.waivers_asked, 1);
        assert_eq!(verification.waivers_granted, 1);
        assert!(verification.criteria.is_empty());
        assert_eq!(verification.outside_plan, Some(Vec::new()));
        assert_eq!(verification.block(), None);
    }
}
