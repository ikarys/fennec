//! Port of openfox's `Criterion` / `CriterionStatus` (shared/types.ts)
//! reshaped for fennec: a criterion's status carries its provenance; not
//! ported: in_progress/completed statuses, attempts history.

use super::event::Event;
use serde::{Deserialize, Serialize};

/// The choice that accepts a proposal; anything else leaves the criteria as they are.
pub const ACCEPT_PROPOSAL: &str = "Accept";
pub const REFUSE_PROPOSAL: &str = "Refuse";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Criterion {
    /// `C1`, `C2`… in the plan's order.
    pub id: String,
    pub description: String,
    /// The command that proves it (exit code 0); None when an agent judges it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CriterionStatus {
    Pending,
    Passed { provenance: Provenance },
    Failed { provenance: Provenance },
}

/// Where a status comes from: shown as verified or indicative (AGENTS.md rule 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum Provenance {
    /// fennec ran the check: a deterministic fact.
    Verified {
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        output: String,
    },
    /// An agent's judgement.
    Indicative { agent: String, reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CriterionState {
    pub criterion: Criterion,
    pub status: CriterionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub id: String,
    pub passed: bool,
    pub reason: String,
}

/// The "Done when" bullets of a plan, in order.
pub fn parse_done_when(plan: &str) -> Vec<Criterion> {
    let mut criteria = Vec::new();
    let mut in_section = false;

    for line in plan.lines() {
        let trimmed = line.trim();

        if let Some(level) = heading_level(trimmed) {
            let title = trimmed[level..].trim();
            let title = title.to_lowercase();
            in_section =
                (level == 2 || level == 3) && (title == "done when" || title == "done when:");
            continue;
        }

        if !in_section || trimmed.is_empty() {
            continue;
        }

        let Some(bullet) = trimmed
            .strip_prefix('-')
            .or_else(|| trimmed.strip_prefix('*'))
        else {
            continue;
        };

        let mut text = bullet.trim();
        if let Some(rest) = text.strip_prefix('[')
            && let Some(close) = rest.find(']')
        {
            let box_content = &rest[..close];
            if box_content == " " || box_content == "x" || box_content == "X" {
                text = rest[close + 1..].trim();
            }
        }

        if text.is_empty() {
            continue;
        }

        let (description, check) = match last_check_index(text) {
            Some(index) => {
                let description = trim_trailing_separators(&text[..index]);
                let command = text[index + "check:".len()..]
                    .trim()
                    .trim_matches('`')
                    .trim();
                if command.is_empty() {
                    (description.to_string(), None)
                } else if is_manual_check(command) {
                    // Not a command: the whole text goes to the judging agent.
                    (text.to_string(), None)
                } else {
                    (description.to_string(), Some(command.to_string()))
                }
            }
            None => (text.to_string(), None),
        };

        criteria.push(Criterion {
            id: format!("C{}", criteria.len() + 1),
            description,
            check,
        });
    }

    criteria
}

/// The criteria a proposal sets when the human's `answer` to its question
/// accepts it: the latest `criteria.proposed` with this `call_id`.
pub fn accepted_proposal(events: &[Event], call_id: &str, answer: &str) -> Option<Vec<Criterion>> {
    let accepted = answer.trim().eq_ignore_ascii_case(ACCEPT_PROPOSAL);
    if !accepted {
        return None;
    }

    events.iter().rev().find_map(|event| match event {
        Event::CriteriaProposed {
            call_id: proposed_call_id,
            criteria,
            ..
        } if proposed_call_id == call_id => Some(criteria.clone()),
        _ => None,
    })
}

/// The question shown to the human (the UI speaks French) for a proposed criteria list.
pub fn proposal_question(current: &[Criterion], proposed: &[Criterion], reason: &str) -> String {
    if current.is_empty() {
        let mut lines = vec![
            "L'agent propose de remplacer les critères :".to_string(),
            String::new(),
            reason.to_string(),
            String::new(),
        ];

        for criterion in proposed {
            lines.push(format!(
                "{}: {}",
                criterion.id,
                criterion_description_and_check(criterion)
            ));
        }

        return lines.join("\n");
    }

    let mut changes = Vec::new();
    let mut unchanged = Vec::new();

    for criterion in proposed {
        match current.iter().find(|c| c.id == criterion.id) {
            None => {
                changes.push(format!(
                    "{} ajouté : {}",
                    criterion.id,
                    criterion_description_and_check(criterion)
                ));
            }
            Some(old) if old != criterion => {
                changes.push(format!(
                    "{} modifié : {}",
                    criterion.id,
                    criterion_description_and_check(criterion)
                ));
                changes.push(format!(
                    "  avant : {}",
                    criterion_description_and_check(old)
                ));
            }
            Some(_) => unchanged.push(criterion.id.clone()),
        }
    }

    for criterion in current {
        if !proposed
            .iter()
            .any(|candidate| candidate.id == criterion.id)
        {
            changes.push(format!(
                "{} retiré : {}",
                criterion.id,
                criterion_description_and_check(criterion)
            ));
        }
    }

    let mut lines = vec![
        "L'agent propose de modifier les critères :".to_string(),
        String::new(),
        reason.to_string(),
        String::new(),
    ];

    if changes.is_empty() {
        lines.push("Aucun changement par rapport aux critères en vigueur.".to_string());
    } else {
        lines.extend(changes);
        if !unchanged.is_empty() {
            lines.push(String::new());
            lines.push(format!("Inchangés : {}.", unchanged.join(", ")));
        }
    }

    lines.join("\n")
}

fn criterion_description_and_check(criterion: &Criterion) -> String {
    let check = match &criterion.check {
        Some(command) => format!("check `{command}`"),
        None => "sans check, à juger".to_string(),
    };
    format!("{} — {}", criterion.description, check)
}

/// The agent's judgement lines: `C<n>: pass|fail — <reason>`.
pub fn parse_verdicts(text: &str) -> Vec<Verdict> {
    let mut verdicts = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("- ").unwrap_or(line);

        let Some(colon) = line.find(':') else {
            continue;
        };

        let id = line[..colon].trim();
        if !is_criterion_id(id) {
            continue;
        }

        let rest = line[colon + 1..].trim();
        let Some((passed, reason)) = verdict_status(rest) else {
            continue;
        };

        verdicts.push(Verdict {
            id: id.to_string(),
            passed,
            reason,
        });
    }

    verdicts
}

/// The `{{criteria}}` template value: one line per criterion.
pub fn render_for_prompt(criteria: &[CriterionState]) -> String {
    criteria
        .iter()
        .map(|state| {
            let status = match &state.status {
                CriterionStatus::Pending => "pending",
                CriterionStatus::Passed { provenance } => match provenance {
                    Provenance::Verified { .. } => "passed (verified)",
                    Provenance::Indicative { .. } => "passed (indicative)",
                },
                CriterionStatus::Failed { provenance } => match provenance {
                    Provenance::Verified { .. } => "failed (verified)",
                    Provenance::Indicative { .. } => "failed (indicative)",
                },
            };

            match &state.criterion.check {
                Some(command) => format!(
                    "{}: {} — check `{command}`: {status}",
                    state.criterion.id, state.criterion.description
                ),
                None => format!(
                    "{}: {} — no check, to judge: {status}",
                    state.criterion.id, state.criterion.description
                ),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The criteria node's output: the failed checked criteria, if any.
pub fn failures_summary(criteria: &[CriterionState]) -> String {
    let mut failures = Vec::new();

    for state in criteria {
        let (Some(command), CriterionStatus::Failed { provenance }) =
            (&state.criterion.check, &state.status)
        else {
            continue;
        };

        let Provenance::Verified {
            exit_code, output, ..
        } = provenance
        else {
            continue;
        };

        let exit_code = match exit_code {
            Some(code) => code.to_string(),
            None => "unknown".to_string(),
        };

        failures.push(format!(
            "{}: {} — check `{command}` failed (exit {exit_code})\n{output}",
            state.criterion.id, state.criterion.description
        ));
    }

    if failures.is_empty() {
        "All checks passed.".to_string()
    } else {
        failures.join("\n\n")
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level == 0 { None } else { Some(level) }
}

fn is_criterion_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix('C') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

/// A model that has no command often writes `check: manual (…)` anyway;
/// running that in a shell can only fail.
fn is_manual_check(command: &str) -> bool {
    let first = command
        .split(|c: char| c.is_whitespace() || c == '(' || c == ':' || c == ',')
        .next()
        .unwrap_or("")
        .to_lowercase();
    matches!(
        first.as_str(),
        "manual"
            | "manually"
            | "manuel"
            | "manuelle"
            | "manuellement"
            | "none"
            | "aucun"
            | "aucune"
            | "n/a"
            | "-"
    )
}

fn last_check_index(text: &str) -> Option<usize> {
    let mut last = None;
    let mut start = 0;

    while let Some(offset) = text[start..].find_ci("check:") {
        last = Some(start + offset);
        start = start + offset + 1;
    }

    last
}

fn verdict_status(rest: &str) -> Option<(bool, String)> {
    let lower = rest.to_lowercase();

    if lower == "pass" {
        return Some((true, String::new()));
    }
    if lower == "fail" {
        return Some((false, String::new()));
    }

    for (word, passed) in [("pass", true), ("fail", false)] {
        let Some(after) = lower.strip_prefix(word) else {
            continue;
        };

        let after = after.trim_start();
        if after.is_empty() || after.starts_with('—') || after.starts_with('-') {
            let reason = rest[word.len()..]
                .trim_start()
                .trim_start_matches('—')
                .trim_start_matches('-')
                .trim_start()
                .to_string();
            return Some((passed, reason));
        }
    }

    None
}

fn trim_trailing_separators(text: &str) -> &str {
    text.trim_end()
        .trim_end_matches([' ', '\t', '—', '–', '-'])
        .trim_end()
}

trait FindCi {
    fn find_ci(&self, needle: &str) -> Option<usize>;
}

impl FindCi for str {
    fn find_ci(&self, needle: &str) -> Option<usize> {
        let needle = needle.to_lowercase();
        self.to_lowercase().find(&needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_done_when_reads_bullets_with_checks() {
        let plan = "\
# Plan

## Done when
- [ ] The build passes — check: `just build`
- [x] The tests pass – check: just test
- The docs are updated - check: test -n docs
- A plain criterion
";

        let criteria = parse_done_when(plan);
        assert_eq!(
            criteria,
            vec![
                Criterion {
                    id: "C1".into(),
                    description: "The build passes".into(),
                    check: Some("just build".into()),
                },
                Criterion {
                    id: "C2".into(),
                    description: "The tests pass".into(),
                    check: Some("just test".into()),
                },
                Criterion {
                    id: "C3".into(),
                    description: "The docs are updated".into(),
                    check: Some("test -n docs".into()),
                },
                Criterion {
                    id: "C4".into(),
                    description: "A plain criterion".into(),
                    check: None,
                },
            ]
        );
    }

    #[test]
    fn parse_done_when_accepts_heading_variants() {
        let plan = "### DONE WHEN:\n- one";
        assert_eq!(parse_done_when(plan).len(), 1);

        let plan = "## done when\n- one";
        assert_eq!(parse_done_when(plan).len(), 1);

        let plan = "# Done when\n- one";
        assert!(parse_done_when(plan).is_empty());
    }

    #[test]
    fn parse_done_when_stops_at_the_next_heading() {
        let plan = "## Done when\n- one\n## Other\n- two";
        let criteria = parse_done_when(plan);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].description, "one");
    }

    #[test]
    fn parse_done_when_missing_section_is_empty() {
        assert!(parse_done_when("## Plan\n- one").is_empty());
        assert!(parse_done_when("").is_empty());
    }

    #[test]
    fn parse_done_when_last_check_wins() {
        let plan = "## Done when\n- check: first — check: `second`";
        let criteria = parse_done_when(plan);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].description, "check: first");
        assert_eq!(criteria[0].check.as_deref(), Some("second"));
    }

    #[test]
    fn parse_done_when_empty_command_means_no_check() {
        let plan = "## Done when\n- one — check: ``";
        let criteria = parse_done_when(plan);
        assert_eq!(criteria[0].description, "one");
        assert_eq!(criteria[0].check, None);
    }

    #[test]
    fn parse_done_when_manual_check_is_left_to_judge() {
        let plan = "## Done when\n- a — check: `manuel (ouvrir un onglet)`\n- b — check: manual\n- c — check: `manualtest.sh`";
        let criteria = parse_done_when(plan);
        assert_eq!(
            criteria[0].description,
            "a — check: `manuel (ouvrir un onglet)`"
        );
        assert_eq!(criteria[0].check, None);
        assert_eq!(criteria[1].check, None);
        assert_eq!(criteria[2].check.as_deref(), Some("manualtest.sh"));
    }

    #[test]
    fn parse_done_when_ignores_non_bullets() {
        let plan = "## Done when\nnot a bullet\n- real";
        let criteria = parse_done_when(plan);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].description, "real");
    }

    #[test]
    fn parse_verdicts_reads_pass_and_fail_lines() {
        let text = "\
C1: pass — because
- C2: FAIL — not yet
junk
C3: pass
";

        let verdicts = parse_verdicts(text);
        assert_eq!(
            verdicts,
            vec![
                Verdict {
                    id: "C1".into(),
                    passed: true,
                    reason: "because".into(),
                },
                Verdict {
                    id: "C2".into(),
                    passed: false,
                    reason: "not yet".into(),
                },
                Verdict {
                    id: "C3".into(),
                    passed: true,
                    reason: String::new(),
                },
            ]
        );
    }

    #[test]
    fn parse_verdicts_ignores_junk_and_non_ids() {
        let text = "1: pass — no\nC: pass — no\nC1 passed — no";
        assert!(parse_verdicts(text).is_empty());
    }

    #[test]
    fn render_for_prompt_renders_statuses() {
        let criteria = vec![
            CriterionState {
                criterion: Criterion {
                    id: "C1".into(),
                    description: "build".into(),
                    check: Some("just build".into()),
                },
                status: CriterionStatus::Passed {
                    provenance: Provenance::Verified {
                        command: "just build".into(),
                        exit_code: Some(0),
                        output: String::new(),
                    },
                },
            },
            CriterionState {
                criterion: Criterion {
                    id: "C2".into(),
                    description: "docs".into(),
                    check: None,
                },
                status: CriterionStatus::Pending,
            },
        ];

        assert_eq!(
            render_for_prompt(&criteria),
            "C1: build — check `just build`: passed (verified)\n\
             C2: docs — no check, to judge: pending"
        );
    }

    #[test]
    fn failures_summary_lists_failed_checked_criteria() {
        let criteria = vec![
            CriterionState {
                criterion: Criterion {
                    id: "C1".into(),
                    description: "build".into(),
                    check: Some("just build".into()),
                },
                status: CriterionStatus::Failed {
                    provenance: Provenance::Verified {
                        command: "just build".into(),
                        exit_code: Some(1),
                        output: "boom".into(),
                    },
                },
            },
            CriterionState {
                criterion: Criterion {
                    id: "C2".into(),
                    description: "docs".into(),
                    check: None,
                },
                status: CriterionStatus::Failed {
                    provenance: Provenance::Indicative {
                        agent: "a".into(),
                        reason: "no".into(),
                    },
                },
            },
        ];

        let summary = failures_summary(&criteria);
        assert!(summary.contains("C1"));
        assert!(summary.contains("just build"));
        assert!(summary.contains("exit 1"));
        assert!(summary.contains("boom"));
        assert!(!summary.contains("C2"));
    }

    #[test]
    fn failures_summary_is_all_passed_when_no_checked_failures() {
        let criteria = vec![CriterionState {
            criterion: Criterion {
                id: "C1".into(),
                description: "docs".into(),
                check: None,
            },
            status: CriterionStatus::Failed {
                provenance: Provenance::Indicative {
                    agent: "a".into(),
                    reason: "no".into(),
                },
            },
        }];

        assert_eq!(failures_summary(&criteria), "All checks passed.");
    }

    #[test]
    fn accepted_proposal_accepts_only_the_matching_call() {
        let criteria = vec![Criterion {
            id: "C1".into(),
            description: "builds".into(),
            check: Some("true".into()),
        }];
        let events = vec![Event::CriteriaProposed {
            call_id: "c1".into(),
            criteria: criteria.clone(),
            reason: "wrong".into(),
        }];

        assert_eq!(accepted_proposal(&events, "c1", "accept"), Some(criteria));
        assert_eq!(accepted_proposal(&events, "c1", "Refuse"), None);
        assert_eq!(accepted_proposal(&events, "c2", "accept"), None);
    }

    fn criterion(id: &str, description: &str, check: Option<&str>) -> Criterion {
        Criterion {
            id: id.into(),
            description: description.into(),
            check: check.map(str::to_string),
        }
    }

    #[test]
    fn proposal_question_empty_current_keeps_old_wording() {
        let proposed = vec![
            criterion("C1", "builds", Some("true")),
            criterion("C2", "docs", None),
        ];

        assert_eq!(
            proposal_question(&[], &proposed, "the old check cannot run"),
            "L'agent propose de remplacer les critères :\n\nthe old check cannot run\n\nC1: builds — check `true`\nC2: docs — sans check, à juger"
        );
    }

    #[test]
    fn proposal_question_shows_only_the_modified_criterion() {
        let current = vec![
            criterion("C1", "builds", Some("true")),
            criterion("C2", "docs", None),
            criterion("C3", "logs", Some("ls")),
        ];
        let proposed = vec![
            criterion("C1", "builds faster", Some("fast")),
            criterion("C2", "docs", None),
            criterion("C3", "logs", Some("ls")),
        ];

        assert_eq!(
            proposal_question(&current, &proposed, "the old check cannot run"),
            "L'agent propose de modifier les critères :\n\nthe old check cannot run\n\nC1 modifié : builds faster — check `fast`\n  avant : builds — check `true`\n\nInchangés : C2, C3."
        );
    }

    #[test]
    fn proposal_question_shows_added_criterion() {
        let current = vec![
            criterion("C1", "builds", Some("true")),
            criterion("C2", "docs", None),
        ];
        let proposed = vec![
            criterion("C1", "builds", Some("true")),
            criterion("C2", "docs", None),
            criterion("C3", "logs", Some("ls")),
        ];

        assert_eq!(
            proposal_question(&current, &proposed, "the old check cannot run"),
            "L'agent propose de modifier les critères :\n\nthe old check cannot run\n\nC3 ajouté : logs — check `ls`\n\nInchangés : C1, C2."
        );
    }

    #[test]
    fn proposal_question_shows_removed_criterion() {
        let current = vec![
            criterion("C1", "builds", Some("true")),
            criterion("C2", "docs", None),
            criterion("C3", "logs", Some("ls")),
        ];
        let proposed = vec![
            criterion("C1", "builds", Some("true")),
            criterion("C2", "docs", None),
        ];

        assert_eq!(
            proposal_question(&current, &proposed, "the old check cannot run"),
            "L'agent propose de modifier les critères :\n\nthe old check cannot run\n\nC3 retiré : logs — check `ls`\n\nInchangés : C1, C2."
        );
    }

    #[test]
    fn proposal_question_identical_lists_says_no_change() {
        let criteria = vec![
            criterion("C1", "builds", Some("true")),
            criterion("C2", "docs", None),
        ];

        assert_eq!(
            proposal_question(&criteria, &criteria, "the old check cannot run"),
            "L'agent propose de modifier les critères :\n\nthe old check cannot run\n\nAucun changement par rapport aux critères en vigueur."
        );
    }
}
