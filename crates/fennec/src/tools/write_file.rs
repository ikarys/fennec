//! Port of openfox `tools/write.ts`.

use super::args;
use super::path_guard::resolve_in_workdir;
use crate::domain::event::ToolResult;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Write content to a file. Creates the file if it does not exist, or overwrites if it does. Creates parent directories as needed.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file (relative to workdir or absolute)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    },
                    "encoding": {
                        "type": "string",
                        "description": "Optional file encoding (e.g. \"ISO-8859-1\", \"windows-1252\", \"utf-16\"). Defaults to \"utf-8\". Use the encoding reported by read_file to match project conventions."
                    }
                },
                "required": ["path", "content"]
            }
        }
    })
}

pub(super) fn run(args: &Value, workdir: &Path) -> ToolResult {
    let path = match args::string(args, "path") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let content = match args::string(args, "content") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let full_path = match resolve_in_workdir(workdir, path) {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    if let Some(parent) = full_path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return ToolResult::failure(format!("Failed to create parent directories: {error}"));
    }

    if let Err(error) = std::fs::write(&full_path, content) {
        return ToolResult::failure(format!("Failed to write file: {error}"));
    }

    let line_count = content.split('\n').count();
    let byte_count = content.len();

    ToolResult::success(
        format!("Successfully wrote {line_count} lines ({byte_count} bytes) to {path}"),
        false,
    )
}
