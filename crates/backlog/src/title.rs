//! List rows need a short, scannable title for an idea.
//! Prefer the human-approved plan title when present, otherwise derive a first-sentence fallback.

/// The short title of an idea: its approved plan's title, else the first sentence of its text, cut at a word boundary.
pub fn title(text: &str, plan: Option<&str>) -> String {
    // The plan title is the human-approved label, so it wins when valid.
    if let Some(mut candidate) = plan_title(plan) {
        candidate = cap_at_word_boundary(&candidate);
        return finalize(candidate);
    }

    let line = first_text_line(text);
    let mut candidate = first_sentence(&line);
    candidate = cap_at_word_boundary(&candidate);
    finalize(candidate)
}

fn plan_title(plan: Option<&str>) -> Option<String> {
    let plan = plan?;
    let mut last = None;

    // The last matching heading wins because later plan edits should override earlier ones.
    for line in plan.lines() {
        if let Some(title) = plan_heading_title(line) {
            last = Some(title);
        }
    }

    last
}

fn plan_heading_title(line: &str) -> Option<String> {
    // Accept exactly two or three leading '#', then required whitespace.
    let rest = if let Some(rest) = line.strip_prefix("###") {
        rest
    } else {
        line.strip_prefix("##")?
    };

    if rest.starts_with('#') {
        return None;
    }

    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }

    let rest = rest.trim_start();
    let rest = rest.strip_prefix("Plan")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;

    let title = rest.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn first_text_line(text: &str) -> String {
    match text.lines().find(|line| !line.trim().is_empty()) {
        Some(line) => line.trim().to_string(),
        None => String::new(),
    }
}

fn first_sentence(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let next_is = |i: usize, c: char| chars.get(i) == Some(&c);
    // Cutting earlier than 12 chars would leave a label ("Fix", "Idée"), not a title.
    for (i, &c) in chars.iter().enumerate().skip(12) {
        let sentence_end = matches!(c, '.' | '?' | '!' | ':') && next_is(i + 1, ' ');
        let dash = c == ' ' && (next_is(i + 1, '-') || next_is(i + 1, '—')) && next_is(i + 2, ' ');
        if sentence_end || dash {
            return cut_prefix(&chars, i);
        }
    }
    line.to_string()
}

fn cut_prefix(chars: &[char], up_to: usize) -> String {
    let s: String = chars.iter().take(up_to).collect();
    s.trim_end().to_string()
}

fn cap_at_word_boundary(s: &str) -> String {
    if s.chars().count() <= 72 {
        return s.to_string();
    }

    let prefix: Vec<char> = s.chars().take(72).collect();
    let cut = prefix.iter().rposition(|c| *c == ' ');
    let take = cut.unwrap_or(prefix.len());

    let mut out: String = prefix.iter().take(take).collect();
    out = trim_trailing_cuts(&out).to_string();
    out.push('…');
    out
}

fn trim_trailing_cuts(s: &str) -> &str {
    s.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':' | '-' | '—'))
}

fn finalize(mut s: String) -> String {
    if s.ends_with('.') {
        s.pop();
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(first) if first.is_lowercase() => first.to_uppercase().chain(chars).collect(),
        _ => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_title_wins() {
        assert_eq!(
            title(
                "Something else. More text.",
                Some("## Plan: Chosen title\n")
            ),
            "Chosen title"
        );
    }

    #[test]
    fn last_plan_heading_wins() {
        let plan = "## Plan: First\n### Plan: Second\n";
        assert_eq!(title("text", Some(plan)), "Second");
    }

    #[test]
    fn h3_plan_heading_works() {
        assert_eq!(title("text", Some("### Plan: ok\n")), "Ok");
    }

    #[test]
    fn plan_without_heading_falls_back_to_text() {
        assert_eq!(
            title("First sentence. Rest.", Some("No heading here\n")),
            "First sentence"
        );
    }

    #[test]
    fn first_sentence_cut_at_dot_space() {
        assert_eq!(title("Hello wide world. Second.", None), "Hello wide world");
    }

    #[test]
    fn short_prefix_colon_not_cut() {
        assert_eq!(title("Fix: a long thing. End.", None), "Fix: a long thing");
    }

    #[test]
    fn long_text_without_punctuation_cut_at_word_with_ellipsis() {
        let long =
            "Aaaaaaaa bbbbbbbb cccccccc dddddddd eeeeeeee ffffffff gggggggg hhhhhhhh iiiiiiii";
        assert_eq!(
            title(long, None),
            "Aaaaaaaa bbbbbbbb cccccccc dddddddd eeeeeeee ffffffff gggggggg hhhhhhhh…"
        );
    }

    #[test]
    fn multibyte_french_near_boundary_does_not_panic() {
        let text = "A".repeat(71) + "é b";
        let expected = "A".repeat(71) + "é…";
        assert_eq!(title(&text, None), expected);
    }

    #[test]
    fn lowercase_first_letter_is_upper_cased() {
        assert_eq!(title("hello world", None), "Hello world");
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(title("", None), "");
    }
}
