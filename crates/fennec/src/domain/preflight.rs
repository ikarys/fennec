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
    /// The runner selected no test: the test the check names does not exist
    /// yet, which is expected before the work and a failure after it.
    RanNothing,
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
// "read-only file system" is not one: the preflight runs in the planner's
// read-only workdir, which the builder can write (`cargo test` writing its
// lockfile), so it says nothing about the builder's sandbox.
const CANNOT_RUN_MARKERS: &[&str] = &[
    "command not found",
    "could not resolve host",
    // Node's DNS codes, with their prefix: a bare "enotfound" is inside
    // Python's "ModuleNotFoundError" and "FileNotFoundError".
    "getaddrinfo enotfound",
    "getaddrinfo eai_again",
    "network is unreachable",
    "temporary failure in name resolution",
    "cannot connect to the docker daemon",
    "connection refused",
];

pub fn exit_code(tool_output: &str) -> Option<i32> {
    const MARKER: &str = "[Exit code: ";
    let start = tool_output.rfind(MARKER)?;
    let rest = &tool_output[start + MARKER.len()..];
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

/// `command` is the check itself: a Python runner it names with `-m` and
/// that is not installed (`No module named 'pytest'`) cannot run here, where
/// a module the work has yet to write is an expected failure.
pub fn classify(exit_code: Option<i32>, output: &str, command: &str) -> PreflightVerdict {
    match exit_code {
        None => return PreflightVerdict::CannotRun,
        Some(code) if [124, 126, 127].contains(&code) => return PreflightVerdict::CannotRun,
        _ if super::test_selection::ran_no_test(output) => return PreflightVerdict::RanNothing,
        Some(0) => return PreflightVerdict::PassesAlready,
        _ => {}
    }

    let lowered = output.to_lowercase();
    if CANNOT_RUN_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
        || missing_top_level_module(output).is_some_and(|module| runs_module(command, module))
    {
        PreflightVerdict::CannotRun
    } else {
        PreflightVerdict::Fails
    }
}

/// Whether `command` runs `module` as a Python runner: `python3 -m pytest …`.
fn runs_module(command: &str, module: &str) -> bool {
    command
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[0] == "-m" && pair[1] == module)
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
            PreflightVerdict::Fails | PreflightVerdict::RanNothing => {}
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

/// The module a Python check could not import, when it is a top-level
/// name (`No module named 'tomli'`): if the repository already holds it,
/// the check cannot import the project itself, which no change will fix.
pub fn missing_top_level_module(output: &str) -> Option<&str> {
    let start = output.rfind("No module named '")?;
    let name_start = start + "No module named '".len();
    let rest = &output[name_start..];
    let end = rest.find('\'')?;
    let name = &rest[..end];

    if name.is_empty()
        || name.contains('.')
        || name.starts_with(|c: char| c.is_ascii_digit())
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }

    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_top_level_module_reads_the_unimportable_package() {
        let output = r#"ImportError: Failed to import test module: tests
Traceback (most recent call last):
  File "/w/tests/__init__.py", line 9, in <module>
    import tomli as tomllib
ModuleNotFoundError: No module named 'tomli'

FAILED (errors=1)

[Exit code: 1]"#;
        assert_eq!(missing_top_level_module(output), Some("tomli"));
    }

    #[test]
    fn missing_top_level_module_ignores_a_dotted_module() {
        let output = "ModuleNotFoundError: No module named 'tests.test_flatten'";
        assert_eq!(missing_top_level_module(output), None);
    }

    #[test]
    fn missing_top_level_module_refuses_a_name_that_is_not_an_identifier() {
        assert_eq!(missing_top_level_module("No module named '../etc'"), None);
        assert_eq!(missing_top_level_module("No module named 'a/b'"), None);
        assert_eq!(missing_top_level_module("No module named ''"), None);
    }

    #[test]
    fn missing_top_level_module_is_none_without_the_message() {
        assert_eq!(
            missing_top_level_module("ModuleNotFoundError: No module named"),
            None
        );
    }

    #[test]
    fn missing_top_level_module_reads_the_last_message() {
        let output = "No module named 'tests.test_flatten'\nNo module named 'tomli'";
        assert_eq!(missing_top_level_module(output), Some("tomli"));
    }

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
            classify(Some(0), "command not found", ""),
            PreflightVerdict::PassesAlready
        );
        assert_eq!(classify(None, "", ""), PreflightVerdict::CannotRun);
        assert_eq!(classify(Some(124), "", ""), PreflightVerdict::CannotRun);
        assert_eq!(classify(Some(126), "", ""), PreflightVerdict::CannotRun);
        assert_eq!(classify(Some(127), "", ""), PreflightVerdict::CannotRun);
    }

    #[test]
    fn classify_marks_environment_failures_as_cannot_run() {
        for marker in CANNOT_RUN_MARKERS {
            assert_eq!(classify(Some(1), marker, ""), PreflightVerdict::CannotRun);
            assert_eq!(
                classify(Some(1), &marker.to_uppercase(), ""),
                PreflightVerdict::CannotRun
            );
        }
    }

    #[test]
    fn classify_marks_a_runner_that_selected_no_test() {
        let cargo = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 3 filtered out";
        assert_eq!(classify(Some(0), cargo, ""), PreflightVerdict::RanNothing);
        assert_eq!(
            classify(Some(5), "collected 0 items\nno tests ran", ""),
            PreflightVerdict::RanNothing
        );
        assert_eq!(
            classify(Some(0), "running 2 tests\ntest result: ok. 2 passed", ""),
            PreflightVerdict::PassesAlready
        );
    }

    #[test]
    fn classify_marks_everything_else_as_fails() {
        assert_eq!(classify(Some(1), "boom", ""), PreflightVerdict::Fails);
        assert_eq!(classify(Some(2), "", ""), PreflightVerdict::Fails);
    }

    #[test]
    fn classify_does_not_blame_the_planners_read_only_workdir() {
        let cargo = "error: failed to open: /work/Cargo.lock\n\nCaused by:\n  Read-only file system (os error 30)";
        assert_eq!(classify(Some(101), cargo, ""), PreflightVerdict::Fails);
    }

    #[test]
    fn classify_marks_a_python_runner_that_is_not_installed_as_cannot_run() {
        let output = "/usr/bin/python3: No module named 'pytest'\n[Exit code: 1]";
        assert_eq!(
            classify(Some(1), output, "python3 -m pytest test_wc.py -q"),
            PreflightVerdict::CannotRun
        );
        assert_eq!(
            classify(Some(1), output, "python3 test_wc.py"),
            PreflightVerdict::Fails,
            "the missing module is not the runner"
        );
    }

    #[test]
    fn classify_takes_a_missing_python_test_module_as_a_failure() {
        let unittest = "ImportError: Failed to import test module: test_flatten\nModuleNotFoundError: No module named 'tests.test_flatten'\nFAILED (errors=1)";
        assert_eq!(classify(Some(1), unittest, ""), PreflightVerdict::Fails);
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
