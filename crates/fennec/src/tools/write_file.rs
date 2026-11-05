//! `write_file` tool.

use super::args;
use super::path_guard::{Outside, resolve_for_writing};
use crate::domain::event::ToolResult;
use crate::domain::protection::Protection;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Write content to a file. Creates the file if it does not exist, or overwrites if it does. Creates parent directories as needed. For a change to an existing file, use edit_file instead: rewriting the whole file costs more and can drop content.",
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

pub(super) fn run(
    args: &Value,
    workdir: &Path,
    protection: &Protection,
    outside: &Outside,
) -> ToolResult {
    try_run(args, workdir, protection, outside).unwrap_or_else(ToolResult::failure)
}

fn try_run(
    args: &Value,
    workdir: &Path,
    protection: &Protection,
    outside: &Outside,
) -> Result<ToolResult, String> {
    let path = args::string(args, "path")?;
    let content = args::string(args, "content")?;
    let full_path = resolve_for_writing(workdir, protection, outside, path)?;

    if let Some(parent) = full_path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return Err(format!("Failed to create parent directories: {error}"));
    }

    if let Err(error) = std::fs::write(&full_path, content) {
        return Err(format!("Failed to write file: {error}"));
    }

    let line_count = content.split('\n').count();
    let byte_count = content.len();

    Ok(ToolResult::success(
        format!("Successfully wrote {line_count} lines ({byte_count} bytes) to {path}"),
        false,
    ))
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::domain::protection::Protection;
    use crate::tools::path_guard::Outside;
    use crate::tools::test_support::temp_dir;

    #[test]
    fn write_to_protected_file_is_refused_and_file_unchanged() {
        let dir = temp_dir();
        std::fs::write(dir.join("Cargo.lock"), "original").unwrap();

        let args = serde_json::json!({
            "path": "Cargo.lock",
            "content": "changed"
        });
        let result = run(&args, &dir, &Protection::new(None), &Outside::none());

        assert!(!result.success);
        assert!(result.error.unwrap().contains("is protected"));
        assert_eq!(
            std::fs::read_to_string(dir.join("Cargo.lock")).unwrap(),
            "original"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
