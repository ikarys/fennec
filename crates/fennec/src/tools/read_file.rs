//! Port of openfox `tools/read.ts` (text files only).

use super::args;
use super::path_guard::resolve_in_workdir;
use crate::domain::event::ToolResult;
use serde_json::{Value, json};
use std::path::Path;

const READ_MAX_LINES: i64 = 2000;
const READ_MAX_BYTES: usize = 100_000;
const READ_MAX_FILE_BYTES: u64 = 20_971_520;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a text file. Returns its lines numbered, from `offset` (1-based) for at most `limit` lines.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file (relative to workdir or absolute)"
                    },
                    "offset": {
                        "type": "number",
                        "description": "Line number to start from (1-indexed). Default: 1. Only applies to text files."
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of lines to read. Default: 2000. Only applies to text files."
                    }
                },
                "required": ["path"]
            }
        }
    })
}

pub(super) fn run(args: &Value, workdir: &Path) -> ToolResult {
    let path = match args::string(args, "path") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let offset = match args::number(args, "offset", 1) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let limit = match args::number(args, "limit", READ_MAX_LINES) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let full_path = match resolve_in_workdir(workdir, path) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let metadata = match std::fs::metadata(&full_path) {
        Ok(value) => value,
        Err(_) => return ToolResult::failure(format!("File not found: {path}")),
    };

    if metadata.is_dir() {
        return ToolResult::failure(format!("Path is a directory: {path}"));
    }

    if metadata.len() > READ_MAX_FILE_BYTES {
        return ToolResult::failure(format!(
            "File size ({} bytes) exceeds maximum file size (20MB). Use shell command to process large files.",
            metadata.len()
        ));
    }

    let bytes = match std::fs::read(&full_path) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(format!("Failed to read file: {error}")),
    };

    let content = match String::from_utf8(bytes) {
        Ok(value) => value,
        Err(_) => return ToolResult::failure("File is not valid UTF-8".to_string()),
    };

    let lines: Vec<&str> = content.split('\n').collect();
    let total_lines = lines.len() as i64;
    let start_line = offset.max(1);
    let limit = limit.clamp(0, READ_MAX_LINES);

    let start_idx = ((start_line - 1).max(0)) as usize;
    let start_idx = start_idx.min(lines.len());

    let (end_line, end_idx) = if limit == 0 {
        (start_line - 1, start_idx)
    } else {
        let end_line = start_line.saturating_add(limit - 1).min(total_lines);
        let end_idx = end_line as usize;
        (end_line, end_idx.max(start_idx))
    };

    let selected = &lines[start_idx..end_idx];
    let mut numbered = Vec::with_capacity(selected.len());

    for (index, line) in selected.iter().enumerate() {
        let line_number = start_line + index as i64;
        numbered.push(format!("{line_number:>6}\t{line}"));
    }

    let mut output = numbered.join("\n");
    let mut truncated = end_line < total_lines;

    if truncated {
        output.push_str(&format!(
            "\n\n[Showing lines {start_line}-{end_line} of {total_lines} total. Use offset to read more.]"
        ));
    }

    if output.len() > READ_MAX_BYTES {
        let mut end = READ_MAX_BYTES;
        while end > 0 && !output.is_char_boundary(end) {
            end -= 1;
        }
        output.truncate(end);
        output.push_str("\n\n[Output truncated due to size limit]");
        truncated = true;
    }

    ToolResult::success(output, truncated)
}

#[cfg(test)]
mod tests {
    use crate::tools::test_support::{no_output, run_tool, temp_dir};

    #[test]
    fn write_then_read_with_line_numbers() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let write_args = serde_json::json!({
            "path": "file.txt",
            "content": "hello\nworld\n"
        });
        let write_result = run_tool("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let read_args = serde_json::json!({
            "path": "file.txt"
        });
        let read_result = run_tool("read_file", &read_args, &dir, &mut on_output);
        assert!(read_result.success);
        let output = read_result.output.unwrap();
        assert!(output.contains("1\thello"));
        assert!(output.contains("2\tworld"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
