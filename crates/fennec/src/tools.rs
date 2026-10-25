//! Port of openfox `src/server/tools/{read,write,edit,shell,path-security}.ts`
//! (simplified: paths outside the workdir are denied, not confirmed).

use serde_json::Value;
use std::io::BufRead;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub use crate::events::ToolResult;

const READ_MAX_LINES: i64 = 2000;
const READ_MAX_BYTES: usize = 100_000;
const READ_MAX_FILE_BYTES: u64 = 20_971_520;
const RUN_MAX_LINES: usize = 2000;
const RUN_MAX_BYTES: usize = 50_000;
const DEFAULT_RUN_TIMEOUT_MS: u64 = 120_000;

pub fn definitions() -> Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the contents of a file or list directory contents. For text files: returns file content. For images (PNG, JPEG, GIF, WebP, BMP, SVG): returns base64-encoded data with MIME type metadata. For PDF files: extracts text content page by page. For directories: returns a tree-formatted listing of entries with file sizes.",
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
        },
        {
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
        },
        {
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
        },
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Execute a shell command. Returns stdout, stderr, exit code, and elapsed duration. Does NOT support trailing \"&\" for backgrounding — use background_process tool instead.\nCommands run from your working directory automatically, so prefer relative paths.",
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
        }
    ])
}

pub fn execute(
    name: &str,
    args: &Value,
    workdir: &Path,
    on_output: &mut dyn FnMut(&str, &str),
) -> ToolResult {
    let start = Instant::now();
    let mut result = match name {
        "read_file" => read_file(args, workdir),
        "write_file" => write_file(args, workdir),
        "edit_file" => edit_file(args, workdir),
        "run_command" => run_command(args, workdir, on_output),
        _ => tool_error(format!("Unknown tool: {name}")),
    };
    result.duration_ms = elapsed_ms(start);
    result
}

fn tool_error(error: String) -> ToolResult {
    ToolResult {
        success: false,
        output: None,
        error: Some(error),
        duration_ms: 0,
        truncated: false,
    }
}

fn tool_success(output: String, truncated: bool) -> ToolResult {
    ToolResult {
        success: true,
        output: Some(output),
        error: None,
        duration_ms: 0,
        truncated,
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

fn arg_string<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match args.get(key) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(format!("Argument {key} must be a string")),
        None => Err(format!("Missing required argument: {key}")),
    }
}

fn arg_bool(args: &Value, key: &str) -> Result<bool, String> {
    match args.get(key) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("Argument {key} must be a boolean")),
        None => Ok(false),
    }
}

fn arg_number(args: &Value, key: &str, default: i64) -> Result<i64, String> {
    match args.get(key) {
        Some(Value::Number(value)) => value
            .as_i64()
            .ok_or_else(|| format!("Argument {key} must be an integer")),
        Some(_) => Err(format!("Argument {key} must be a number")),
        None => Ok(default),
    }
}

fn resolve_in_workdir(workdir: &Path, p: &str) -> Result<PathBuf, String> {
    let canonical_workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let normalized = lexical_normalize(workdir, p);

    let mut ancestor = normalized.clone();
    let mut remaining = Vec::new();

    loop {
        // symlink_metadata, not exists(): a dangling symlink must count as
        // existing so canonicalize below rejects it instead of letting a
        // write follow it out of the workdir.
        if ancestor.symlink_metadata().is_ok() {
            break;
        }

        if let Some(component) = ancestor.components().next_back() {
            remaining.push(component.as_os_str().to_owned());
        }

        if !ancestor.pop() {
            if ancestor.as_os_str().is_empty() {
                ancestor = PathBuf::from("/");
            }
            break;
        }
    }

    let canonical_ancestor = ancestor
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path {p}: {e}"))?;

    if canonical_ancestor != canonical_workdir
        && !canonical_ancestor.starts_with(&canonical_workdir)
    {
        return Err(format!("Path outside working directory: {p}"));
    }

    let mut resolved = canonical_ancestor;
    for component in remaining.iter().rev() {
        resolved.push(component);
    }

    Ok(resolved)
}

fn lexical_normalize(workdir: &Path, p: &str) -> PathBuf {
    let path = Path::new(p);
    let mut normalized = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        workdir.to_path_buf()
    };

    for component in path.components() {
        match component {
            Component::RootDir => {
                normalized = PathBuf::from("/");
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && normalized.as_os_str().is_empty() {
                    normalized = PathBuf::from("/");
                }
            }
            Component::Normal(_) => normalized.push(component.as_os_str()),
            Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        normalized
    }
}

fn read_file(args: &Value, workdir: &Path) -> ToolResult {
    let path = match arg_string(args, "path") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let offset = match arg_number(args, "offset", 1) {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let limit = match arg_number(args, "limit", READ_MAX_LINES) {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let full_path = match resolve_in_workdir(workdir, path) {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let metadata = match std::fs::metadata(&full_path) {
        Ok(value) => value,
        Err(_) => return tool_error(format!("File not found: {path}")),
    };

    if metadata.is_dir() {
        return tool_error(format!("Path is a directory: {path}"));
    }

    if metadata.len() > READ_MAX_FILE_BYTES {
        return tool_error(format!(
            "File size ({} bytes) exceeds maximum file size (20MB). Use shell command to process large files.",
            metadata.len()
        ));
    }

    let bytes = match std::fs::read(&full_path) {
        Ok(value) => value,
        Err(error) => return tool_error(format!("Failed to read file: {error}")),
    };

    let content = match String::from_utf8(bytes) {
        Ok(value) => value,
        Err(_) => return tool_error("File is not valid UTF-8".to_string()),
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

    tool_success(output, truncated)
}

fn write_file(args: &Value, workdir: &Path) -> ToolResult {
    let path = match arg_string(args, "path") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let content = match arg_string(args, "content") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let full_path = match resolve_in_workdir(workdir, path) {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    if let Some(parent) = full_path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return tool_error(format!("Failed to create parent directories: {error}"));
    }

    if let Err(error) = std::fs::write(&full_path, content) {
        return tool_error(format!("Failed to write file: {error}"));
    }

    let line_count = content.split('\n').count();
    let byte_count = content.len();

    tool_success(
        format!("Successfully wrote {line_count} lines ({byte_count} bytes) to {path}"),
        false,
    )
}

fn edit_file(args: &Value, workdir: &Path) -> ToolResult {
    let path = match arg_string(args, "path") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let old_string = match arg_string(args, "old_string") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let new_string = match arg_string(args, "new_string") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let replace_all = match arg_bool(args, "replace_all") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let full_path = match resolve_in_workdir(workdir, path) {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    let bytes = match std::fs::read(&full_path) {
        Ok(value) => value,
        Err(_) => return tool_error(format!("File not found: {path}")),
    };

    let content = match String::from_utf8(bytes) {
        Ok(value) => value,
        Err(_) => return tool_error("File is not valid UTF-8".to_string()),
    };

    let line_ending = detect_line_ending(&content);
    let normalized_content = normalize_to_lf(&content);
    let normalized_old = normalize_to_lf(old_string);
    let normalized_new = normalize_to_lf(new_string);

    if normalized_old.is_empty() {
        return tool_error("old_string must not be empty".to_string());
    }

    let occurrences = normalized_content.split(normalized_old.as_str()).count() - 1;

    if occurrences == 0 {
        let preview = preview_string(old_string);
        return tool_error(format!(
            "old_string not found in file.\n\nSearched for:\n{preview}\n\nMake sure whitespace and indentation match exactly."
        ));
    }

    if occurrences > 1 && !replace_all {
        return tool_error(format!(
            "Found {occurrences} matches for old_string. Use replace_all: true to replace all, or provide more context to make the match unique."
        ));
    }

    let replaced = if replace_all {
        normalized_content.replace(normalized_old.as_str(), &normalized_new)
    } else {
        match normalized_content.split_once(normalized_old.as_str()) {
            Some((before, after)) => format!("{before}{normalized_new}{after}"),
            None => return tool_error("old_string not found in file (unexpected)".to_string()),
        }
    };

    let restored = match line_ending {
        LineEnding::Crlf => replaced.replace('\n', "\r\n"),
        LineEnding::Cr => replaced.replace('\n', "\r"),
        LineEnding::Lf => replaced,
    };

    if let Err(error) = std::fs::write(&full_path, restored) {
        return tool_error(format!("Failed to write file: {error}"));
    }

    let count = if replace_all { occurrences } else { 1 };
    tool_success(
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

fn run_command(args: &Value, workdir: &Path, on_output: &mut dyn FnMut(&str, &str)) -> ToolResult {
    let command = match arg_string(args, "command") {
        Ok(value) => value,
        Err(error) => return tool_error(error),
    };

    if has_background_ampersand(command) {
        return tool_error(
            "Use background_process tool (action: \"start\") for background/long-running commands instead of '&'. See the tool description for details."
                .to_string(),
        );
    }

    let cwd = if let Some(cwd) = args.get("cwd") {
        if cwd.is_null() {
            workdir.to_path_buf()
        } else {
            match arg_string(args, "cwd") {
                Ok(value) => match resolve_in_workdir(workdir, value) {
                    Ok(path) => path,
                    Err(error) => return tool_error(error),
                },
                Err(error) => return tool_error(error),
            }
        }
    } else {
        workdir.to_path_buf()
    };

    let timeout_ms = match arg_number(args, "timeout", DEFAULT_RUN_TIMEOUT_MS as i64) {
        Ok(value) => value.max(0) as u64,
        Err(error) => return tool_error(error),
    };

    let command_start = Instant::now();

    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(value) => value,
        Err(error) => {
            return tool_error(format!("Failed to start command: {error}"));
        }
    };

    let stdout_pipe = child.stdout.take().expect("stdout was piped");
    let stderr_pipe = child.stderr.take().expect("stderr was piped");

    let (tx, rx) = mpsc::channel();
    let stdout_tx = tx.clone();
    let stderr_tx = tx.clone();

    let stdout_thread = thread::spawn(move || {
        read_pipe_lines(stdout_pipe, true, stdout_tx);
    });

    let stderr_thread = thread::spawn(move || {
        read_pipe_lines(stderr_pipe, false, stderr_tx);
    });

    drop(tx);

    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();
    let mut timed_out = false;
    let mut exit_status = None;

    'outer: loop {
        loop {
            match rx.try_recv() {
                Ok((is_stdout, chunk)) => {
                    on_output(if is_stdout { "stdout" } else { "stderr" }, &chunk);
                    if is_stdout {
                        stdout_buf.push_str(&chunk);
                    } else {
                        stderr_buf.push_str(&chunk);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // Pipes can close before the process is reaped: keep polling
                // try_wait so the exit code and the timeout still apply.
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                exit_status = Some(status);
                break 'outer;
            }
            Ok(None) => {}
            Err(error) => {
                return tool_error(format!("Failed to wait for command: {error}"));
            }
        }

        if command_start.elapsed() >= Duration::from_millis(timeout_ms) {
            timed_out = true;
            let _ = child.kill();
            let _ = child.wait();
            break 'outer;
        }

        thread::sleep(Duration::from_millis(50));
    }

    while let Ok((is_stdout, chunk)) = rx.recv() {
        on_output(if is_stdout { "stdout" } else { "stderr" }, &chunk);
        if is_stdout {
            stdout_buf.push_str(&chunk);
        } else {
            stderr_buf.push_str(&chunk);
        }
    }

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let command_duration_ms = elapsed_ms(command_start);
    let stdout = stdout_buf.trim().to_string();
    let stderr = stderr_buf.trim().to_string();

    let exit_code = if timed_out {
        124
    } else {
        match exit_status {
            Some(status) => status.code().unwrap_or(1),
            None => 1,
        }
    };

    let mut output = String::new();

    if !stdout.is_empty() {
        output.push_str(&stdout);
    }

    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str("[stderr]\n");
        output.push_str(&stderr);
    }

    output.push_str(&format!("\n\n[Exit code: {exit_code}]"));

    if timed_out {
        output.push_str(&format!("\n[Process timed out after {timeout_ms}ms]"));
    }

    let visible = strip_ansi(&output);
    let mut truncated = false;

    if visible.len() > RUN_MAX_BYTES {
        let mut end = RUN_MAX_BYTES;
        while end > 0 && !output.is_char_boundary(end) {
            end -= 1;
        }
        output.truncate(end);
        output.push_str("\n\n[Output truncated due to size limit]");
        truncated = true;
    }

    if visible.split('\n').count() > RUN_MAX_LINES {
        let limited: Vec<&str> = output.split('\n').take(RUN_MAX_LINES).collect();
        output = limited.join("\n");
        output.push_str("\n\n[Output truncated due to line limit]");
        truncated = true;
    }

    output.push_str(&format!(
        "\n[Duration: {}]",
        format_duration(command_duration_ms)
    ));

    if exit_code == 0 {
        ToolResult {
            success: true,
            output: Some(output),
            error: None,
            duration_ms: 0,
            truncated,
        }
    } else {
        ToolResult {
            success: false,
            output: Some(output),
            error: Some(format!("Command exited with code {exit_code}")),
            duration_ms: 0,
            truncated,
        }
    }
}

fn read_pipe_lines(pipe: impl std::io::Read, is_stdout: bool, tx: mpsc::Sender<(bool, String)>) {
    let mut reader = std::io::BufReader::new(pipe);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            // Lossy: one invalid UTF-8 byte must not cut off the rest of the output.
            Ok(n) if n > 0 => {
                if tx
                    .send((is_stdout, String::from_utf8_lossy(&line).into_owned()))
                    .is_err()
                {
                    break;
                }
            }
            _ => break,
        }
    }
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

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some(&'[') => {
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next.is_ascii() && (0x40u32..=0x7eu32).contains(&(next as u32)) {
                            break;
                        }
                    }
                }
                Some(&']') => {
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next == '\u{07}' || next == '\u{1b}' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            out.push(c);
        }
    }

    out
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        return format!("{ms}ms");
    }

    if ms < 60_000 {
        let mut seconds = ms / 1000;
        let mut tenths = ((ms % 1000) + 50) / 100;
        if tenths == 10 {
            seconds += 1;
            tenths = 0;
        }
        return format!("{seconds}.{tenths}s");
    }

    let mut minutes = ms / 60_000;
    let mut seconds = (ms % 60_000 + 500) / 1000;
    if seconds == 60 {
        minutes += 1;
        seconds = 0;
    }

    format!("{minutes}m {seconds}s")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "openfox-tools-test-{}-{nanos}-{counter}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn no_output() -> impl FnMut(&str, &str) {
        |_, _| {}
    }

    #[test]
    fn write_then_read_with_line_numbers() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let write_args = serde_json::json!({
            "path": "file.txt",
            "content": "hello\nworld\n"
        });
        let write_result = execute("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let read_args = serde_json::json!({
            "path": "file.txt"
        });
        let read_result = execute("read_file", &read_args, &dir, &mut on_output);
        assert!(read_result.success);
        let output = read_result.output.unwrap();
        assert!(output.contains("1\thello"));
        assert!(output.contains("2\tworld"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_single_no_multiple_and_replace_all() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let write_args = serde_json::json!({
            "path": "edit.txt",
            "content": "foo bar\nfoo baz\n"
        });
        let write_result = execute("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let single_args = serde_json::json!({
            "path": "edit.txt",
            "old_string": "foo bar",
            "new_string": "qux bar"
        });
        let single_result = execute("edit_file", &single_args, &dir, &mut on_output);
        assert!(single_result.success);
        assert!(single_result.output.unwrap().contains("1 occurrence"));

        let missing_args = serde_json::json!({
            "path": "edit.txt",
            "old_string": "missing",
            "new_string": "value"
        });
        let missing_result = execute("edit_file", &missing_args, &dir, &mut on_output);
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
        let multi_write_result = execute("write_file", &multi_write_args, &dir, &mut on_output);
        assert!(multi_write_result.success);

        let multi_args = serde_json::json!({
            "path": "multi.txt",
            "old_string": "abc",
            "new_string": "x"
        });
        let multi_result = execute("edit_file", &multi_args, &dir, &mut on_output);
        assert!(!multi_result.success);
        assert!(multi_result.error.unwrap().contains("Found 2 matches"));

        let replace_all_args = serde_json::json!({
            "path": "multi.txt",
            "old_string": "abc",
            "new_string": "x",
            "replace_all": true
        });
        let replace_all_result = execute("edit_file", &replace_all_args, &dir, &mut on_output);
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
        let write_result = execute("write_file", &write_args, &dir, &mut on_output);
        assert!(write_result.success);

        let edit_args = serde_json::json!({
            "path": "crlf.txt",
            "old_string": "line1",
            "new_string": "lineX"
        });
        let edit_result = execute("edit_file", &edit_args, &dir, &mut on_output);
        assert!(edit_result.success);

        let content = std::fs::read_to_string(dir.join("crlf.txt")).unwrap();
        assert_eq!(content, "lineX\r\nline2\r\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_escape_via_parent_dir_denied() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let args = serde_json::json!({
            "path": "../escape.txt",
            "content": "outside"
        });
        let result = execute("write_file", &args, &dir, &mut on_output);
        assert!(!result.success);
        assert!(
            result
                .error
                .unwrap()
                .contains("Path outside working directory")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn symlink_escape_denied() {
        let dir = temp_dir();
        let outside = temp_dir();
        let mut on_output = no_output();

        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), dir.join("link.txt")).unwrap();

        let args = serde_json::json!({
            "path": "link.txt"
        });
        let result = execute("read_file", &args, &dir, &mut on_output);
        assert!(!result.success);
        assert!(
            result
                .error
                .unwrap()
                .contains("Path outside working directory")
        );

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn run_command_echo_stdout_and_exit_code() {
        let dir = temp_dir();
        let mut chunks = Vec::new();
        let mut on_output = |stream: &str, chunk: &str| {
            chunks.push((stream.to_string(), chunk.to_string()));
        };

        let args = serde_json::json!({
            "command": "echo hello"
        });
        let result = execute("run_command", &args, &dir, &mut on_output);
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("hello"));
        assert!(output.contains("[Exit code: 0]"));
        assert!(!chunks.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_command_timeout_kills() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let args = serde_json::json!({
            "command": "sleep 2",
            "timeout": 100
        });
        let result = execute("run_command", &args, &dir, &mut on_output);
        assert!(!result.success);
        let output = result.output.unwrap();
        assert!(output.contains("[Exit code: 124]"));
        assert!(output.contains("timed out"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_command_trailing_ampersand_rejected() {
        let dir = temp_dir();
        let mut on_output = no_output();

        let args = serde_json::json!({
            "command": "echo hi &"
        });
        let result = execute("run_command", &args, &dir, &mut on_output);
        assert!(!result.success);
        assert!(
            result
                .error
                .unwrap()
                .contains("Use background_process tool")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
