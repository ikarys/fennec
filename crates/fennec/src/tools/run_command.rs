//! Port of openfox `tools/shell.ts`: `run_command`.

pub(super) mod process;
mod report;

use super::args;
use super::path_guard::resolve_in_workdir;
use super::sandbox::Sandbox;
use crate::domain::event::{OutputStream, ToolResult};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const DEFAULT_TIMEOUT_MS: i64 = 120_000;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "run_command",
            "description": "Execute a shell command. Returns stdout, stderr, exit code, and elapsed duration. Does NOT support a trailing \"&\": background commands are not available, run commands in the foreground.\nCommands run from your working directory automatically, so prefer relative paths.\nSearch with `rg` (content) and `rg --files` (names) rather than grep or find. Keep output short: long output is truncated and fills your context, so pipe it through `tail`/`head` or filter it.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to execute"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory for the command (default: session workdir)"
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Timeout in milliseconds (default: 120000)"
                    }
                },
                "required": ["command"]
            }
        }
    })
}

pub(super) fn run(
    args: &Value,
    workdir: &Path,
    sandbox: &Sandbox,
    env: &[(String, PathBuf)],
    stop: &AtomicBool,
    on_output: &mut dyn FnMut(OutputStream, &str),
) -> ToolResult {
    let (command, cwd, timeout_ms) = match parse(args, workdir) {
        Ok(request) => request,
        Err(error) => return ToolResult::failure(error),
    };
    let timeout = Duration::from_millis(timeout_ms);
    match process::execute(
        process::Launch {
            command,
            workdir,
            cwd: &cwd,
            sandbox,
            env,
        },
        timeout,
        stop,
        on_output,
    ) {
        Ok(execution) => report::report(execution, timeout_ms),
        Err(error) => ToolResult::failure(error),
    }
}

/// The command, the directory it runs in and its timeout in milliseconds.
fn parse<'a>(
    args: &'a Value,
    workdir: &Path,
) -> Result<(&'a str, std::path::PathBuf, u64), String> {
    let command = args::string(args, "command")?;
    if has_background_ampersand(command) {
        return Err(
            "Background commands ('&') are not supported: run the command in the foreground, with a longer timeout if needed."
                .to_string(),
        );
    }
    let cwd = match args.get("cwd").filter(|v| !v.is_null()) {
        Some(_) => resolve_in_workdir(workdir, args::string(args, "cwd")?)?,
        None => workdir.to_path_buf(),
    };
    let timeout_ms = args::number(args, "timeout", DEFAULT_TIMEOUT_MS)?.max(0) as u64;
    Ok((command, cwd, timeout_ms))
}

fn has_background_ampersand(command: &str) -> bool {
    let mut processed = String::with_capacity(command.len());
    let mut chars = command.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                processed.push(' ');
                while let Some(&next) = chars.peek() {
                    if next == '\'' {
                        chars.next();
                        break;
                    }
                    processed.push(' ');
                    chars.next();
                }
            }
            '"' => {
                processed.push(' ');
                while let Some(&next) = chars.peek() {
                    if next == '"' {
                        chars.next();
                        break;
                    }
                    processed.push(' ');
                    chars.next();
                }
            }
            '\\' => {
                processed.push(' ');
                if chars.next().is_some() {
                    processed.push(' ');
                }
            }
            _ => processed.push(c),
        }
    }

    processed = processed.replace("&&", "  ");
    processed = processed.replace("|&", "   ");
    processed = processed.replace("&>", "  ");
    processed = replace_gt_amp_digit(&processed);
    processed = processed.replace(">&", "  ");

    processed.contains('&')
}

fn replace_gt_amp_digit(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '>' && chars.peek() == Some(&'&') {
            chars.next();
            if let Some(&digit) = chars.peek()
                && digit.is_ascii_digit()
            {
                chars.next();
                out.push_str("   ");
                continue;
            }
        }
        out.push(c);
    }

    out
}

#[cfg(test)]
mod tests {
    use crate::domain::event::OutputStream;
    use crate::tools::test_support::{no_output, run_tool, temp_dir};

    #[test]
    fn run_command_echo_stdout_and_exit_code() {
        let dir = temp_dir();
        let mut chunks = Vec::new();
        let mut on_output = |stream: OutputStream, chunk: &str| {
            chunks.push((stream, chunk.to_string()));
        };

        let args = serde_json::json!({
            "command": "echo hello"
        });
        let result = run_tool("run_command", &args, &dir, &mut on_output);
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("hello"));
        assert!(output.contains("[Exit code: 0]"));
        assert!(!chunks.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn execute_creates_the_workdir() {
        let dir = temp_dir();
        // A chat session's scratch did not exist: the first command creates it.
        let workdir = dir.join("scratch");
        assert!(!workdir.exists());
        let mut on_output = no_output();

        let args = serde_json::json!({ "command": "echo hello" });
        let result = run_tool("run_command", &args, &workdir, &mut on_output);
        assert!(result.success);
        assert!(result.output.as_deref().unwrap_or("").contains("hello"));
        assert!(workdir.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_command_timeout_kills() {
        let dir = temp_dir();
        let mut on_output = no_output();

        // `; true` makes sh fork instead of exec: sleep is a grandchild that
        // must die with the group, or its open pipes keep the call waiting.
        let args = serde_json::json!({
            "command": "sleep 30; true",
            "timeout": 100
        });
        let started = std::time::Instant::now();
        let result = run_tool("run_command", &args, &dir, &mut on_output);
        assert!(!result.success);
        let output = result.output.unwrap();
        assert!(output.contains("[Exit code: 124]"));
        assert!(output.contains("timed out"));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_command_trailing_ampersand_rejected() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let args = serde_json::json!({
            "command": "echo hi &"
        });
        let result = run_tool("run_command", &args, &dir, &mut on_output);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Background commands"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
