use crate::domain::conversation::render_tool_result;
use crate::domain::event::Event;
use std::io::Write;

/// Renders events as a terminal transcript: answer text plain, thinking
/// and tool output dimmed.
#[derive(Default)]
pub struct Transcript {
    in_thinking: bool,
    /// Tool output may stop mid-line; the result mark then starts a new one.
    output_open: bool,
}

impl Transcript {
    pub fn print(&mut self, event: &Event) {
        const DIM: &str = "\x1b[2m";
        const RESET: &str = "\x1b[0m";
        let mut out = std::io::stdout().lock();
        if self.in_thinking && !matches!(event, Event::MessageThinking { .. }) {
            self.in_thinking = false;
            let _ = writeln!(out, "{RESET}");
        }
        let _ = match event {
            Event::MessageStart {
                role,
                content: Some(c),
                ..
            } => writeln!(out, "\n\x1b[1m{role}>\x1b[0m {c}"),
            Event::MessageStart { role, .. } => writeln!(out, "\n\x1b[1m{role}>\x1b[0m"),
            Event::MessageDelta { content, .. } => write!(out, "{content}"),
            Event::MessageThinking { content, .. } => {
                let start = if self.in_thinking { "" } else { DIM };
                self.in_thinking = true;
                write!(out, "{start}{content}")
            }
            Event::ToolCall { tool_call, .. } => {
                writeln!(out, "\n▸ {} {}", tool_call.name, tool_call.arguments)
            }
            Event::ToolOutput { content, .. } => {
                self.output_open = !content.ends_with('\n');
                write!(out, "{DIM}{content}{RESET}")
            }
            Event::ToolResult { result, .. } => {
                let text = render_tool_result(result);
                let first = text.lines().next().unwrap_or("");
                let mark = if result.success { "✓" } else { "✗" };
                if std::mem::take(&mut self.output_open) {
                    let _ = writeln!(out);
                }
                writeln!(out, "{mark} {first} ({} ms)", result.duration_ms)
            }
            Event::ChatError { error, .. } => writeln!(out, "\n\x1b[31merror: {error}\x1b[0m"),
            Event::ChatDone { reason, .. } => writeln!(out, "\n— {reason}"),
            Event::SessionInitialized { .. }
            | Event::MessageDone { .. }
            | Event::TurnSnapshot { .. }
            | Event::ContextState { .. }
            | Event::RunningChanged { .. } => Ok(()),
        };
        let _ = out.flush();
    }
}
