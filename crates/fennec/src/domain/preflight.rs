//! The plan's checks, tested before the builder changes anything: a check
//! that cannot run or already passes is a contract the builder cannot meet.

use super::event::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreflightVerdict {
    Fails,
    PassesAlready,
    CannotRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightResult {
    pub criterion_id: String,
    pub command: String,
    pub verdict: PreflightVerdict,
    pub exit_code: Option<i32>,
    pub output: String,
    #[serde(default)]
    pub existence_only: bool,
}

// ponytail: heuristic; extend it when a new environment failure shows up.
const CANNOT_RUN_MARKERS: &[&str] = &[
    "command not found",
    "could not resolve host",
    "enotfound",
    "eai_again",
    "network is unreachable",
    "temporary failure in name resolution",
    "cannot connect to the docker daemon",
    "connection refused",
    "read-only file system",
];

pub fn exit_code(tool_output: &str) -> Option<i32> {
    const MARKER: &str = "[Exit code: ";
    let start = tool_output.rfind(MARKER)?;
    let rest = &tool_output[start + MARKER.len()..];
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

pub fn classify(exit_code: Option<i32>, output: &str) -> PreflightVerdict {
    match exit_code {
        Some(0) => return PreflightVerdict::PassesAlready,
        None => return PreflightVerdict::CannotRun,
        Some(code) if [124, 126, 127].contains(&code) => return PreflightVerdict::CannotRun,
        _ => {}
    }

    let lowered = output.to_lowercase();
    if CANNOT_RUN_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
    {
        PreflightVerdict::CannotRun
    } else {
        PreflightVerdict::Fails
    }
}

/// The criteria whose check already passed before any change, from the
/// last preflight: an integrated run that still lists one proves nothing
/// by it, and the human should know.
pub fn passed_before(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::CriteriaPreflight { results, .. } => Some(results),
            _ => None,
        })
        .map(|results| {
            results
                .iter()
                .filter(|result| result.verdict == PreflightVerdict::PassesAlready)
                .map(|result| result.criterion_id.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub fn feedback(results: &[PreflightResult]) -> Option<String> {
    let mut lines = vec![
        "fennec ran the plan's checks before any change, in the sandbox the builder will use."
            .to_string(),
    ];

    for result in results {
        match result.verdict {
            PreflightVerdict::Fails => {}
            PreflightVerdict::PassesAlready => {
                lines.push(format!(
                    "- {} `{}`: passes already. A check that passes before the work proves nothing: make it fail until the step is done, or keep it only as a guard against a regression and say so in the criterion.",
                    result.criterion_id, result.command
                ));
            }
            PreflightVerdict::CannotRun => {
                let exit = result
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "none".to_string());
                lines.push(format!(
                    "- {} `{}`: cannot run here (exit {exit}): {}. Use a command that runs offline in the sandbox, or drop the check and leave the criterion to the reviewer.",
                    result.criterion_id,
                    result.command,
                    last_300_bytes_one_line(&result.output)
                ));
            }
        }
    }

    if lines.len() == 1 {
        return None;
    }

    lines.push("Fix these checks and end with the complete revised plan.".to_string());
    Some(lines.join("\n"))
}

fn last_300_bytes_one_line(output: &str) -> String {
    let mut start = 0;
    if output.len() > 300 {
        start = output.len() - 300;
        while !output.is_char_boundary(start) {
            start += 1;
        }
    }
    output[start..]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(
        criterion_id: &str,
        command: &str,
        verdict: PreflightVerdict,
        exit_code: Option<i32>,
        output: &str,
    ) -> PreflightResult {
        PreflightResult {
            criterion_id: criterion_id.into(),
            command: command.into(),
            verdict,
            exit_code,
            output: output.into(),
            existence_only: false,
        }
    }

    #[test]
    fn exit_code_reads_the_last_marker() {
        assert_eq!(exit_code("ok\n[Exit code: 0]"), Some(0));
        assert_eq!(exit_code("[Exit code: 1]\n[Exit code: 2]"), Some(2));
        assert_eq!(exit_code("[Exit code: -1]"), Some(-1));
        assert_eq!(exit_code("no marker"), None);
        assert_eq!(exit_code("[Exit code: ]"), None);
    }

    #[test]
    fn classify_uses_the_exit_code_first() {
        assert_eq!(
            classify(Some(0), "command not found"),
            PreflightVerdict::PassesAlready
        );
        assert_eq!(classify(None, ""), PreflightVerdict::CannotRun);
        assert_eq!(classify(Some(124), ""), PreflightVerdict::CannotRun);
        assert_eq!(classify(Some(126), ""), PreflightVerdict::CannotRun);
        assert_eq!(classify(Some(127), ""), PreflightVerdict::CannotRun);
    }

    #[test]
    fn classify_marks_environment_failures_as_cannot_run() {
        for marker in CANNOT_RUN_MARKERS {
            assert_eq!(classify(Some(1), marker), PreflightVerdict::CannotRun);
            assert_eq!(
                classify(Some(1), &marker.to_uppercase()),
                PreflightVerdict::CannotRun
            );
        }
    }

    #[test]
    fn classify_marks_everything_else_as_fails() {
        assert_eq!(classify(Some(1), "boom"), PreflightVerdict::Fails);
        assert_eq!(classify(Some(2), ""), PreflightVerdict::Fails);
    }

    #[test]
    fn feedback_is_none_when_every_check_fails() {
        let results = vec![result(
            "C1",
            "cargo test",
            PreflightVerdict::Fails,
            Some(1),
            "boom",
        )];
        assert_eq!(feedback(&results), None);
        assert_eq!(feedback(&[]), None);
    }

    #[test]
    fn feedback_lists_passes_already_and_cannot_run() {
        let results = vec![
            result("C2", "true", PreflightVerdict::PassesAlready, Some(0), ""),
            result(
                "C3",
                "curl example.com",
                PreflightVerdict::CannotRun,
                Some(127),
                "curl: command not found\n",
            ),
        ];
        let feedback = feedback(&results).expect("feedback");
        assert!(feedback.starts_with(
            "fennec ran the plan's checks before any change, in the sandbox the builder will use."
        ));
        assert!(feedback.contains(
            "- C2 `true`: passes already. A check that passes before the work proves nothing: make it fail until the step is done, or keep it only as a guard against a regression and say so in the criterion."
        ));
        assert!(feedback.contains(
            "- C3 `curl example.com`: cannot run here (exit 127): curl: command not found. Use a command that runs offline in the sandbox, or drop the check and leave the criterion to the reviewer."
        ));
        assert!(feedback.ends_with("Fix these checks and end with the complete revised plan."));
    }

    #[test]
    fn feedback_keeps_the_last_300_bytes_on_one_line() {
        let output = format!("{}\ntail", "x".repeat(400));
        let results = vec![result(
            "C1",
            "cmd",
            PreflightVerdict::CannotRun,
            None,
            &output,
        )];
        let feedback = feedback(&results).expect("feedback");
        assert!(feedback.contains("exit none): "));
        assert!(!feedback.contains("\n\n"));
        assert!(feedback.ends_with("Fix these checks and end with the complete revised plan."));
    }

    #[test]
    fn passed_before_reads_the_last_preflight() {
        let first = Event::CriteriaPreflight {
            message_id: "m".into(),
            results: vec![result(
                "C1",
                "true",
                PreflightVerdict::PassesAlready,
                Some(0),
                "",
            )],
        };
        let last = Event::CriteriaPreflight {
            message_id: "m".into(),
            results: vec![result(
                "C2",
                "true",
                PreflightVerdict::PassesAlready,
                Some(0),
                "",
            )],
        };
        assert_eq!(passed_before(&[first, last]), vec!["C2"]);
        assert_eq!(passed_before(&[]), Vec::<String>::new());
    }
}
