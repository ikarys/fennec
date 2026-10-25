//! Outcome-line parsing and output trimming for agent nodes.

/// The prompt suffix that asks for an outcome line; empty for one output.
pub(super) fn outcome_instruction(outputs: &[String]) -> String {
    if outputs.len() <= 1 {
        String::new()
    } else {
        format!(
            "\n\nWhen you are done, end your last message with one line: Outcome: <one of: {}>",
            outputs.join(", ")
        )
    }
}

/// The last line, if it is `Outcome: <one of outputs>` (case-insensitive).
pub(super) fn parse_outcome(text: &str, outputs: &[String]) -> Option<String> {
    let last_line = text.lines().rev().find(|line| !line.trim().is_empty())?;
    let lower = last_line.trim().to_lowercase();
    let value = lower
        .strip_prefix("outcome:")?
        .trim()
        .trim_end_matches('.')
        .trim_matches(|c: char| c == '`' || c == '*' || c.is_whitespace());
    outputs
        .iter()
        .find(|o| o.eq_ignore_ascii_case(value))
        .cloned()
}

/// Drops the last line when it is an `Outcome:` line.
pub(super) fn strip_outcome_line(text: &str) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    if let Some(last) = lines.last()
        && last.trim().to_lowercase().starts_with("outcome:")
    {
        return lines[..lines.len() - 1].join("\n").trim().to_string();
    }
    text.trim().to_string()
}

/// The last 8 KiB of `text`, cut on a char boundary.
pub(super) fn last_8kib(text: &str) -> String {
    const MAX: usize = 8 * 1024;
    if text.len() <= MAX {
        return text.to_string();
    }
    let start = text.len() - MAX;
    let start = text
        .char_indices()
        .find(|(i, _)| *i >= start)
        .map(|(i, _)| i)
        .unwrap_or(0);
    text[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_instruction_is_empty_for_one_output() {
        assert_eq!(outcome_instruction(&["done".into()]), "");
    }

    #[test]
    fn outcome_instruction_lists_outputs() {
        let instruction = outcome_instruction(&["ok".into(), "ko".into()]);
        assert!(instruction.contains("Outcome: <one of: ok, ko>"));
    }

    #[test]
    fn parse_outcome_reads_the_last_line() {
        let text = "work\nOutcome: ko";
        assert_eq!(
            parse_outcome(text, &["ok".into(), "ko".into()]),
            Some("ko".into())
        );
    }

    #[test]
    fn parse_outcome_is_case_insensitive_and_trims_markup() {
        let text = "Outcome: `OK`. ";
        assert_eq!(
            parse_outcome(text, &["ok".into(), "ko".into()]),
            Some("ok".into())
        );
    }

    #[test]
    fn parse_outcome_rejects_unknown_values() {
        assert_eq!(parse_outcome("Outcome: nope", &["ok".into()]), None);
    }

    #[test]
    fn strip_outcome_line_drops_the_outcome_line() {
        assert_eq!(strip_outcome_line("result\nOutcome: ok"), "result");
    }

    #[test]
    fn strip_outcome_line_keeps_other_text() {
        assert_eq!(strip_outcome_line("result"), "result");
    }

    #[test]
    fn last_8kib_keeps_short_text() {
        assert_eq!(last_8kib("abc"), "abc");
    }

    #[test]
    fn last_8kib_cuts_on_a_char_boundary() {
        let text = "é".repeat(10_000);
        let out = last_8kib(&text);
        assert!(out.len() <= 8 * 1024);
        assert!(out.starts_with('é'));
    }
}
