//! Port of openfox `tools/edit.ts`: exact string replacement that keeps the
//! file's line endings.

use super::args;
use super::path_guard::resolve_in_workdir;
use crate::domain::event::ToolResult;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "edit_file",
            "description": "Replace specific text in a file. Use this for surgical edits. The old_string must match exactly (including whitespace and indentation).",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file (relative to workdir or absolute)"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "Exact text to find and replace. Must match exactly including whitespace."
                    },
                    "new_string": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace all occurrences (default: false). If false and multiple matches found, the operation fails."
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }
        }
    })
}

pub(super) fn run(args: &Value, workdir: &Path) -> ToolResult {
    let path = match args::string(args, "path") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let old_string = match args::string(args, "old_string") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let new_string = match args::string(args, "new_string") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let replace_all = match args::boolean(args, "replace_all") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let full_path = match resolve_in_workdir(workdir, path) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let bytes = match std::fs::read(&full_path) {
        Ok(value) => value,
        Err(_) => return ToolResult::failure(format!("File not found: {path}")),
    };

    let content = match String::from_utf8(bytes) {
        Ok(value) => value,
        Err(_) => return ToolResult::failure("File is not valid UTF-8".to_string()),
    };

    let line_ending = detect_line_ending(&content);
    let normalized_content = normalize_to_lf(&content);
    let normalized_old = normalize_to_lf(old_string);
    let normalized_new = normalize_to_lf(new_string);

    if normalized_old.is_empty() {
        return ToolResult::failure("old_string must not be empty".to_string());
    }

    let occurrences = normalized_content.split(normalized_old.as_str()).count() - 1;

    if occurrences == 0 {
        let preview = preview_string(old_string);
        return ToolResult::failure(format!(
            "old_string not found in file.\n\nSearched for:\n{preview}\n\nMake sure whitespace and indentation match exactly."
        ));
    }

    if occurrences > 1 && !replace_all {
        return ToolResult::failure(format!(
            "Found {occurrences} matches for old_string. Use replace_all: true to replace all, or provide more context to make the match unique."
        ));
    }

    let replaced = if replace_all {
        normalized_content.replace(normalized_old.as_str(), &normalized_new)
    } else {
        match normalized_content.split_once(normalized_old.as_str()) {
            Some((before, after)) => format!("{before}{normalized_new}{after}"),
            None => {
                return ToolResult::failure(
                    "old_string not found in file (unexpected)".to_string(),
                );
            }
        }
    };

    let restored = match line_ending {
        LineEnding::Crlf => replaced.replace('\n', "\r\n"),
        LineEnding::Cr => replaced.replace('\n', "\r"),
        LineEnding::Lf => replaced,
    };

    if let Err(error) = std::fs::write(&full_path, restored) {
        return ToolResult::failure(format!("Failed to write file: {error}"));
    }

    let count = if replace_all { occurrences } else { 1 };
    ToolResult::success(
        format!("Successfully replaced {count} occurrence(s) in {path}"),
        false,
    )
}

#[derive(Clone, Copy)]
enum LineEnding {
    Crlf,
    Lf,
    Cr,
}

fn detect_line_ending(content: &str) -> LineEnding {
    if content.contains("\r\n") {
        LineEnding::Crlf
    } else if content.contains('\n') {
        LineEnding::Lf
    } else if content.contains('\r') {
        LineEnding::Cr
    } else {
        LineEnding::Lf
    }
}

fn normalize_to_lf(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

fn preview_string(value: &str) -> String {
    if value.chars().count() > 100 {
        format!("{}...", value.chars().take(100).collect::<String>())
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::tools::test_support::{no_output, run_tool, temp_dir};

    #[test]
    fn edit_single_no_multiple_and_replace_all() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let write_args = serde_json::json!({
            "path": "edit.txt",
            "content": "foo bar\nfoo baz\n"
        });
        let write_result = run_tool("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let single_args = serde_json::json!({
            "path": "edit.txt",
            "old_string": "foo bar",
            "new_string": "qux bar"
        });
        let single_result = run_tool("edit_file", &single_args, &dir, &mut on_output);
        assert!(single_result.success);
        assert!(single_result.output.unwrap().contains("1 occurrence"));

        let missing_args = serde_json::json!({
            "path": "edit.txt",
            "old_string": "missing",
            "new_string": "value"
        });
        let missing_result = run_tool("edit_file", &missing_args, &dir, &mut on_output);
        assert!(!missing_result.success);
        assert!(
            missing_result
                .error
                .unwrap()
                .contains("old_string not found")
        );

        let multi_write_args = serde_json::json!({
            "path": "multi.txt",
            "content": "abc\nabc\n"
        });
        let multi_write_result = run_tool("write_file", &multi_write_args, &dir, &mut on_output);
        assert!(multi_write_result.success);

        let multi_args = serde_json::json!({
            "path": "multi.txt",
            "old_string": "abc",
            "new_string": "x"
        });
        let multi_result = run_tool("edit_file", &multi_args, &dir, &mut on_output);
        assert!(!multi_result.success);
        assert!(multi_result.error.unwrap().contains("Found 2 matches"));

        let replace_all_args = serde_json::json!({
            "path": "multi.txt",
            "old_string": "abc",
            "new_string": "x",
            "replace_all": true
        });
        let replace_all_result = run_tool("edit_file", &replace_all_args, &dir, &mut on_output);
        assert!(replace_all_result.success);
        assert!(
            replace_all_result
                .output
                .unwrap()
                .contains("2 occurrence(s)")
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("multi.txt")).unwrap(),
            "x\nx\n"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_preserves_crlf() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let write_args = serde_json::json!({
            "path": "crlf.txt",
            "content": "line1\r\nline2\r\n"
        });
        let write_result = run_tool("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let edit_args = serde_json::json!({
            "path": "crlf.txt",
            "old_string": "line1",
            "new_string": "lineX"
        });
        let edit_result = run_tool("edit_file", &edit_args, &dir, &mut on_output);
        assert!(edit_result.success);

        let content = std::fs::read_to_string(dir.join("crlf.txt")).unwrap();
        assert_eq!(content, "lineX\r\nline2\r\n");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
