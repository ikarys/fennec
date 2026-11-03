//! What silences a gate. An agent can make checks pass by silencing them
//! instead of fixing the code; these are text tripwires shown to the human,
//! not proofs (like the architecture rules).

use super::diff::ChangedLines;

/// An added line that silences a check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suppression {
    pub path: String,
    pub line: String,
    pub marker: String,
}

/// Markers that silence a lint, a type check or a test. Comment lines are not
/// skipped: the markers live in comments.
const MARKERS: &[&str] = &[
    "#[allow(",
    "#![allow(",
    "#[ignore",
    "#[cfg(any())]",
    "eslint-disable",
    "@ts-ignore",
    "@ts-expect-error",
    "@ts-nocheck",
    "biome-ignore",
    "noqa",
    "type: ignore",
    "pylint: disable",
    "nolint",
    "istanbul ignore",
    "c8 ignore",
    "it.skip(",
    "describe.skip(",
    "test.skip(",
    "xit(",
    "xdescribe(",
    "pytest.mark.skip",
    "pytest.mark.xfail",
    "@Disabled",
    "@Ignore",
];

/// One per added line that contains a marker (the first marker of MARKERS that
/// matches), trimmed line.
pub fn suppressions(path: &str, added: &[String]) -> Vec<Suppression> {
    let mut out = Vec::new();

    for line in added {
        let line = line.trim();
        if let Some(marker) = MARKERS.iter().find(|&&marker| line.contains(marker)) {
            out.push(Suppression {
                path: path.to_string(),
                line: line.to_string(),
                marker: marker.to_string(),
            });
        }
    }

    out
}

/// The tests and assertions a branch removes on balance.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestsWeakened {
    pub tests_removed: usize,
    pub assertions_removed: usize,
    /// The files that lost some, in input order.
    pub paths: Vec<String>,
}

/// `files`: (path relative to the project, its changed lines). The balance is
/// summed over all files first, so tests moved from one file to another do
/// not count as removed.
pub fn tests_weakened(files: &[(String, ChangedLines)]) -> Option<TestsWeakened> {
    let (mut tests, mut assertions) = (0i64, 0i64);
    let mut paths = Vec::new();
    for (path, lines) in files {
        let (removed_tests, removed_assertions) = counts(&lines.removed);
        let (added_tests, added_assertions) = counts(&lines.added);
        if removed_tests > added_tests || removed_assertions > added_assertions {
            paths.push(path.clone());
        }
        tests += removed_tests - added_tests;
        assertions += removed_assertions - added_assertions;
    }
    if tests <= 0 && assertions <= 0 {
        return None;
    }
    Some(TestsWeakened {
        tests_removed: tests.max(0) as usize,
        assertions_removed: assertions.max(0) as usize,
        paths,
    })
}

/// (test declarations, assertions) among `lines`.
fn counts(lines: &[String]) -> (i64, i64) {
    let tests = lines
        .iter()
        .filter(|line| is_test_declaration(line))
        .count();
    let assertions = lines.iter().filter(|line| is_assertion(line)).count();
    (tests as i64, assertions as i64)
}

fn is_test_declaration(line: &str) -> bool {
    let line = line.trim();
    line.contains("#[test]")
        || line.contains("#[tokio::test")
        || line.starts_with("def test_")
        || line.starts_with("async def test_")
        || line.starts_with("it(")
        || line.starts_with("test(")
        || line.starts_with("it.only(")
        || line.starts_with("test.only(")
}

fn is_assertion(line: &str) -> bool {
    let line = line.trim();
    line.contains("assert") || (line.contains("expect(") && !line.contains(".expect("))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppressions_flag_the_marker_families() {
        let added = vec![
            String::from("#[allow(dead_code)] fn f() {}"),
            String::from("/* eslint-disable */"),
            String::from("// @ts-ignore"),
            String::from("# noqa"),
            String::from("it.skip('x')"),
            String::from("pytest.mark.skip"),
        ];

        let found = suppressions("a.rs", &added);
        let markers: Vec<&str> = found.iter().map(|s| s.marker.as_str()).collect();

        assert_eq!(
            markers,
            vec![
                "#[allow(",
                "eslint-disable",
                "@ts-ignore",
                "noqa",
                "it.skip(",
                "pytest.mark.skip",
            ]
        );
    }

    #[test]
    fn suppressions_use_the_first_marker_and_one_per_line() {
        let added = vec![String::from("#[allow(clippy::all)] // noqa")];

        let found = suppressions("a.rs", &added);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].marker, "#[allow(");
        assert_eq!(found[0].line, "#[allow(clippy::all)] // noqa");
    }

    #[test]
    fn iter_skip_is_not_a_suppression() {
        let added = vec![String::from("let x = iter().skip(1);")];

        assert!(suppressions("a.rs", &added).is_empty());
    }

    #[test]
    fn suppressions_trim_the_line() {
        let added = vec![String::from("   #[allow(unused)]   ")];

        let found = suppressions("a.rs", &added);
        assert_eq!(
            found,
            vec![Suppression {
                path: "a.rs".into(),
                line: "#[allow(unused)]".into(),
                marker: "#[allow(".into(),
            }]
        );
    }

    #[test]
    fn a_deleted_test_is_flagged_with_its_path() {
        let files = vec![(
            "a.rs".to_string(),
            ChangedLines {
                added: vec![],
                removed: vec![String::from("#[test]"), String::from("fn old() {")],
            },
        )];

        let found = tests_weakened(&files).unwrap();

        assert_eq!(found.tests_removed, 1);
        assert_eq!(found.assertions_removed, 0);
        assert_eq!(found.paths, vec!["a.rs".to_string()]);
    }

    #[test]
    fn tests_moved_between_files_are_not_a_blocker() {
        let files = vec![
            (
                "a.rs".to_string(),
                ChangedLines {
                    added: vec![],
                    removed: vec![String::from("#[test]")],
                },
            ),
            (
                "b.rs".to_string(),
                ChangedLines {
                    added: vec![String::from("#[test]")],
                    removed: vec![],
                },
            ),
        ];

        assert!(tests_weakened(&files).is_none());
    }

    #[test]
    fn option_expect_is_not_an_assertion() {
        let files = vec![(
            "a.rs".to_string(),
            ChangedLines {
                added: vec![],
                removed: vec![String::from("let x = y.expect(\"x\");")],
            },
        )];

        assert!(tests_weakened(&files).is_none());
    }

    #[test]
    fn split_and_exit_are_not_test_declarations() {
        let files = vec![(
            "a.rs".to_string(),
            ChangedLines {
                added: vec![],
                removed: vec![
                    String::from("let a = s.split(' ');"),
                    String::from("exit(1);"),
                ],
            },
        )];

        assert!(tests_weakened(&files).is_none());
    }

    #[test]
    fn an_assertion_removed_with_code_added_elsewhere_blocks() {
        let files = vec![(
            "a.rs".to_string(),
            ChangedLines {
                added: vec![String::from("let x = 1;")],
                removed: vec![String::from("assert_eq!(x, 1);")],
            },
        )];

        let found = tests_weakened(&files).unwrap();

        assert_eq!(found.tests_removed, 0);
        assert_eq!(found.assertions_removed, 1);
        assert_eq!(found.paths, vec!["a.rs".to_string()]);
    }
}
