//! Port of openfox `tools/edit.ts`: exact string replacement that keeps the
//! file's line endings.
//! The guidance in its errors (where to look, what to run) is fennec's own.

use super::args;
use super::path_guard::resolve_for_writing;
use crate::domain::event::ToolResult;
use crate::domain::protection::Protection;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "edit_file",
            "description": "Replace specific text in a file. Use this for surgical edits; prefer it to write_file for an existing file. Read the file first. The old_string must match exactly (including whitespace and indentation) and be unique: include enough surrounding lines. If the edit fails, read the region again before retrying.",
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

pub(super) fn run(args: &Value, workdir: &Path, protection: &Protection) -> ToolResult {
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

    let full_path = match resolve_for_writing(workdir, protection, path) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let bytes = match std::fs::read(&full_path) {
        Ok(value) => value,
        Err(_) => {
            let name = Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string());
            return ToolResult::failure(format!(
                "File not found: {path}. Find it with: rg --files | rg '{name}'"
            ));
        }
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
        let hits = first_line_hits(&normalized_content, &normalized_old);
        let message = if let Some(first) = hits.first().copied() {
            let list = hits
                .iter()
                .take(3)
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let offset = first.saturating_sub(5).max(1);
            format!(
                "old_string not found in file.\n\nSearched for:\n{preview}\n\nIts first line appears at line(s) {list}, with different surrounding text or indentation: read_file that range (offset {offset}, limit 20) and retry with the exact text."
            )
        } else {
            format!(
                "old_string not found in file.\n\nSearched for:\n{preview}\n\nIts first line does not appear in the file either: read the file again (rg -n to locate) before retrying."
            )
        };
        return ToolResult::failure(message);
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

fn first_line_hits(content: &str, old: &str) -> Vec<usize> {
    let target = old
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if target.is_empty() {
        return Vec::new();
    }

    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| (line.trim() == target).then_some(index + 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::domain::protection::Protection;
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
    fn edit_to_protected_file_is_refused_and_file_unchanged() {
        let dir = temp_dir();
        std::fs::write(dir.join("Cargo.lock"), "original").unwrap();

        let args = serde_json::json!({
            "path": "Cargo.lock",
            "old_string": "original",
            "new_string": "changed"
        });
        let result = run(&args, &dir, &Protection::new(None));

        assert!(!result.success);
        assert!(result.error.unwrap().contains("is protected"));
        assert_eq!(
            std::fs::read_to_string(dir.join("Cargo.lock")).unwrap(),
            "original"
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

    #[test]
    fn edit_not_found_reports_first_line_hits_when_indented_differently() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let write_args = serde_json::json!({
            "path": "indent.txt",
            "content": "hello world\n  hello world\n"
        });
        let write_result = run_tool("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let edit_args = serde_json::json!({
            "path": "indent.txt",
            "old_string": "    hello world",
            "new_string": "value"
        });
        let edit_result = run_tool("edit_file", &edit_args, &dir, &mut on_output);
        assert!(!edit_result.success);
        let error = edit_result.error.unwrap();
        assert!(error.contains("old_string not found"));
        assert!(error.contains("line(s) 1, 2"));
        assert!(error.contains("offset 1"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_not_found_says_first_line_missing_when_absent() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let write_args = serde_json::json!({
            "path": "absent.txt",
            "content": "hello\nworld\n"
        });
        let write_result = run_tool("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let edit_args = serde_json::json!({
            "path": "absent.txt",
            "old_string": "missing",
            "new_string": "value"
        });
        let edit_result = run_tool("edit_file", &edit_args, &dir, &mut on_output);
        assert!(!edit_result.success);
        let error = edit_result.error.unwrap();
        assert!(error.contains("old_string not found"));
        assert!(error.contains("Its first line does not appear in the file either"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
