//! Which files are tests, and what the human's `.agents/protected` gains when
//! they protect the tests an integration brings (they are validated by then).

use super::compaction::files_changed;
use super::event::Event;
use super::protection::Protection;

/// The comment that heads the lines an integration appends.
pub const HEADER: &str = "# tests validated by integration";

/// `relative`: `/`-separated, relative to the project root.
pub fn is_test_path(relative: &str) -> bool {
    let mut segments: Vec<&str> = relative.split('/').collect();
    let Some(name) = segments.pop() else {
        return false;
    };
    if segments
        .iter()
        .any(|dir| matches!(*dir, "tests" | "test" | "__tests__"))
    {
        return true;
    }
    let stem_of = |exts: &[&str]| exts.iter().any(|ext| name.ends_with(ext));
    // `tests.rs` is the file of a `#[cfg(test)] mod tests;`, Rust's convention.
    name == "tests.rs"
        || stem_of(&["_test.rs", "_test.go", "_test.py"])
        || (name.starts_with("test_") && name.ends_with(".py"))
        || ["test", "spec"].iter().any(|kind| {
            ["ts", "tsx", "js", "jsx"]
                .iter()
                .any(|ext| name.ends_with(&format!(".{kind}.{ext}")))
        })
}

/// The test files written so far in a session's log: what the human validates at the test gate.
pub fn written(events: &[Event]) -> Vec<String> {
    files_changed(events)
        .into_iter()
        .filter(|file| is_test_path(file))
        .collect()
}

/// The test files the human validated in this session, protected from then on.
pub fn approved(events: &[Event]) -> Vec<String> {
    let mut files = Vec::new();
    for event in events {
        if let Event::TestsApproved {
            files: approved, ..
        } = event
        {
            for file in approved {
                if !files.contains(file) {
                    files.push(file.clone());
                }
            }
        }
    }
    files
}

/// The test files among `files` (relative to the project root) that the
/// protection does not already cover.
pub fn new_test_patterns(files: &[String], protection: &Protection) -> Vec<String> {
    files
        .iter()
        .filter(|file| is_test_path(file) && !protection.is_protected(file))
        .cloned()
        .collect()
}

/// The new content of `.agents/protected`: the existing lines kept, then the
/// header and one anchored path per line. Paths are prefixed with `/` because
/// a pattern without `/` would match that name in every directory. Unchanged
/// when there is nothing to add.
pub fn append_to(existing: &str, paths: &[String]) -> String {
    if paths.is_empty() {
        return existing.to_string();
    }
    let mut content = existing.to_string();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(HEADER);
    content.push('\n');
    for path in paths {
        content.push('/');
        content.push_str(path);
        content.push('\n');
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_filters_the_test_files_from_the_log() {
        let events = vec![
            Event::ToolCall {
                message_id: "m".into(),
                tool_call: crate::domain::event::ToolCall {
                    id: "c1".into(),
                    name: "write_file".into(),
                    arguments: serde_json::json!({"path": "tests/a.rs"}),
                    parse_error: None,
                    raw_arguments: None,
                },
            },
            Event::ToolResult {
                message_id: "m".into(),
                tool_call_id: "c1".into(),
                result: crate::domain::event::ToolResult::success("ok".into(), false),
            },
            Event::ToolCall {
                message_id: "m".into(),
                tool_call: crate::domain::event::ToolCall {
                    id: "c2".into(),
                    name: "write_file".into(),
                    arguments: serde_json::json!({"path": "src/lib.rs"}),
                    parse_error: None,
                    raw_arguments: None,
                },
            },
            Event::ToolResult {
                message_id: "m".into(),
                tool_call_id: "c2".into(),
                result: crate::domain::event::ToolResult::success("ok".into(), false),
            },
        ];
        assert_eq!(written(&events), vec!["tests/a.rs".to_string()]);
    }

    #[test]
    fn approved_unions_test_files_in_first_seen_order() {
        let events = vec![
            Event::TestsApproved {
                run_id: "r".into(),
                files: vec!["tests/a.rs".into(), "tests/b.rs".into()],
            },
            Event::TestsApproved {
                run_id: "r".into(),
                files: vec!["tests/b.rs".into(), "tests/c.rs".into()],
            },
        ];
        assert_eq!(
            approved(&events),
            vec![
                "tests/a.rs".to_string(),
                "tests/b.rs".to_string(),
                "tests/c.rs".to_string()
            ]
        );
        assert_eq!(approved(&[]), Vec::<String>::new());
    }

    #[test]
    fn recognises_test_paths() {
        for yes in [
            "tests/a.rs",
            "src/test/x.java",
            "web/__tests__/a.ts",
            "src/foo_test.rs",
            "src/agent/workflow/tests.rs",
            "pkg/foo_test.go",
            "test_foo.py",
            "a/foo_test.py",
            "web/a.test.ts",
            "web/a.test.tsx",
            "a.test.js",
            "a.test.jsx",
            "a.spec.ts",
            "a.spec.tsx",
            "a.spec.js",
            "a.spec.jsx",
        ] {
            assert!(is_test_path(yes), "{yes}");
        }
        for no in [
            "src/main.rs",
            "src/testing.rs",
            "latest/a.rs",
            "a.test.rs",
            "contest.py",
            "test.ts",
            "",
        ] {
            assert!(!is_test_path(no), "{no}");
        }
    }

    #[test]
    fn skips_already_protected_and_non_tests() {
        let protection = Protection::new(Some("tests/old.rs\n"));
        let files = vec![
            "tests/old.rs".to_string(),
            "tests/new.rs".to_string(),
            "src/lib.rs".to_string(),
        ];
        assert_eq!(new_test_patterns(&files, &protection), ["tests/new.rs"]);
    }

    #[test]
    fn appends_under_header_keeping_existing_lines() {
        let paths = vec!["a_test.go".to_string()];
        assert_eq!(
            append_to("x\n# mine", &paths),
            format!("x\n# mine\n{HEADER}\n/a_test.go\n")
        );
        assert_eq!(append_to("", &paths), format!("{HEADER}\n/a_test.go\n"));
        assert_eq!(append_to("x", &[]), "x");

        let protected = Protection::new(Some(&append_to("", &["a.test.ts".to_string()])));
        assert!(protected.is_protected("a.test.ts"));
        assert!(!protected.is_protected("web/a.test.ts"));
    }
}
