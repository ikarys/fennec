//! What the model reads back from a command: port of the output format of
//! openfox `tools/shell.ts`.

use super::process::{Ending, Execution};
use crate::domain::event::ToolResult;

const MAX_LINES: usize = 2000;
const MAX_BYTES: usize = 50_000;

pub fn report(execution: Execution, timeout_ms: u64) -> ToolResult {
    let exit_code = match execution.ending {
        Ending::Exited(status) => status.code().unwrap_or(1),
        Ending::TimedOut => 124,
        Ending::Stopped => 1,
    };
    let stdout = execution.stdout.trim();
    let stderr = execution.stderr.trim();

    let mut output = String::new();
    if !stdout.is_empty() {
        output.push_str(stdout);
    }
    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str("[stderr]\n");
        output.push_str(stderr);
    }
    output.push_str(&format!("\n\n[Exit code: {exit_code}]"));
    match execution.ending {
        Ending::TimedOut => output.push_str(&format!("\n[Process timed out after {timeout_ms}ms]")),
        Ending::Stopped => output.push_str("\n[Stopped by the user]"),
        Ending::Exited(_) => {}
    }
    let truncated = truncate(&mut output);
    output.push_str(&format!(
        "\n[Duration: {}]",
        format_duration(execution.duration_ms)
    ));

    let mut result = ToolResult::success(output, truncated);
    if exit_code != 0 {
        result.success = false;
        result.error = Some(format!("Command exited with code {exit_code}"));
    }
    result
}

/// Caps the output by bytes then lines, measured without ANSI escapes.
fn truncate(output: &mut String) -> bool {
    let visible = strip_ansi(output);
    let mut truncated = false;
    if visible.len() > MAX_BYTES {
        let mut end = MAX_BYTES;
        while end > 0 && !output.is_char_boundary(end) {
            end -= 1;
        }
        output.truncate(end);
        output.push_str("\n\n[Output truncated due to size limit]");
        truncated = true;
    }
    if visible.split('\n').count() > MAX_LINES {
        let limited: Vec<&str> = output.split('\n').take(MAX_LINES).collect();
        *output = limited.join("\n");
        output.push_str("\n\n[Output truncated due to line limit]");
        truncated = true;
    }
    truncated
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
