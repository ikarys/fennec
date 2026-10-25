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

/// Caps the output at `MAX_BYTES` and `MAX_LINES`. openfox keeps the head;
/// errors and test summaries sit at the end, so fennec drops the middle:
/// the first 40% and the last 60% of each budget survive, around a marker.
fn truncate(output: &mut String) -> bool {
    let visible = strip_ansi(output);
    if visible.len() <= MAX_BYTES && visible.split('\n').count() <= MAX_LINES {
        return false;
    }
    let lines: Vec<&str> = output.split('\n').collect();
    let (head_lines, head_bytes) = (MAX_LINES * 2 / 5, MAX_BYTES * 2 / 5);
    let head = fit(lines.iter().copied(), head_lines, head_bytes, prefix);
    let mut tail = fit(
        lines[head.len()..].iter().rev().copied(),
        MAX_LINES - head_lines,
        MAX_BYTES - head_bytes,
        suffix,
    );
    tail.reverse();
    let omitted = lines.len() - head.len() - tail.len();
    *output = format!(
        "{}\n[... {omitted} lines omitted ...]\n{}",
        head.join("\n"),
        tail.join("\n")
    );
    true
}

/// Takes lines while they fit in both budgets; the first line that does not
/// fit is `cut` to the bytes left, so one huge line still shows something.
/// Bytes are counted with ANSI escapes, which only keeps a little less.
fn fit<'a>(
    lines: impl Iterator<Item = &'a str>,
    max_lines: usize,
    max_bytes: usize,
    cut: fn(&'a str, usize) -> &'a str,
) -> Vec<&'a str> {
    let mut kept = Vec::new();
    let mut bytes = 0;
    for line in lines.take(max_lines) {
        let room = max_bytes - bytes;
        if line.len() + 1 > room {
            if room > 1 {
                kept.push(cut(line, room - 1));
            }
            break;
        }
        bytes += line.len() + 1;
        kept.push(line);
    }
    kept
}

fn prefix(line: &str, bytes: usize) -> &str {
    &line[..line.floor_char_boundary(bytes)]
}

fn suffix(line: &str, bytes: usize) -> &str {
    &line[line.ceil_char_boundary(line.len() - bytes)..]
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

    #[test]
    fn short_output_is_kept_whole() {
        let mut output = String::from("hello\nworld");
        assert!(!truncate(&mut output));
        assert_eq!(output, "hello\nworld");
    }

    #[test]
    fn a_long_output_keeps_its_head_and_tail_around_a_marker() {
        let mut output = (0..5000)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(truncate(&mut output));
        let lines: Vec<&str> = output.split('\n').collect();
        assert_eq!(lines.len(), MAX_LINES + 1);
        assert_eq!(lines[0], "line 0");
        assert_eq!(lines[MAX_LINES * 2 / 5], "[... 3000 lines omitted ...]");
        assert_eq!(lines[MAX_LINES], "line 4999");
    }

    #[test]
    fn a_huge_line_is_cut_on_char_boundaries() {
        let mut output = "é".repeat(60_000);
        assert!(truncate(&mut output));
        assert!(output.len() <= MAX_BYTES + 40);
        let (head, rest) = output.split_once('\n').unwrap();
        assert!(head.chars().all(|c| c == 'é'));
        assert_eq!(rest, "[... 0 lines omitted ...]\n");
    }
}
