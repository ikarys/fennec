//! Ports openfox session/name-generator: the name's cleanup. The prompt is
//! adapted from openfox's to ask for the specific subject of the task.
//! Not ported: the disableAutoSessionTitle switch, the 'Session N' default check.

/// Adapted from openfox's `SESSION_NAME_PROMPT`: the only context sent to the model.
pub const SESSION_NAME_PROMPT: &str =
    "Generate a concise, descriptive session name (max 50 characters) based on the user's message.
Name the specific subject (the feature, file, bug or question), not the kind of step: never a generic name such as \"Next step\", \"Continue\" or \"Follow-up\". Write it in the language of the message.
Return ONLY the name, nothing else.

Example inputs and outputs:
- \"How do I set up React?\" → \"React setup\"
- \"Fix the authentication bug\" → \"Fix authentication bug\"
- \"Add unit tests for the API\" → \"Add API unit tests\"

User message: {message}";

/// The model-facing prompt for one user message.
pub fn prompt(message: &str) -> String {
    SESSION_NAME_PROMPT.replace("{message}", message)
}

/// Cleans the model's answer into a usable session title.
///
/// Takes the first non-empty line, removes one layer of surrounding
/// quotes/backticks and a trailing period, then enforces the 50-char limit
/// by character count.
pub fn clean(raw: &str) -> Option<String> {
    let line = raw.lines().map(str::trim).find(|line| !line.is_empty())?;
    let mut name = strip_surrounding_quotes(line);
    name = name.strip_suffix('.').unwrap_or(name);
    let name = name.trim();

    if name.chars().count() < 3 {
        return None;
    }
    if name.chars().count() > 50 {
        let truncated: String = name.chars().take(47).collect();
        return Some(format!("{truncated}..."));
    }
    Some(name.to_string())
}

/// Returns a title that is unique among `taken` sibling titles.
///
/// Comparison is trimmed and case-insensitive. When the name is taken, the
/// smallest `#n` suffix (starting at 2) that is not taken is appended.
pub fn disambiguate(name: &str, taken: &[String]) -> String {
    let name = name.trim();
    if !taken.iter().any(|t| t.trim().eq_ignore_ascii_case(name)) {
        return name.to_string();
    }

    let mut n = 2;
    loop {
        let candidate = format!("{name} #{n}");
        if !taken
            .iter()
            .any(|t| t.trim().eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
        n += 1;
    }
}

/// Strips matching surrounding quotes or backticks, one layer at a time.
fn strip_surrounding_quotes(mut name: &str) -> &str {
    while let Some(inner) = ['"', '\'', '`'].iter().find_map(|&q| {
        name.strip_prefix(q)
            .and_then(|rest| rest.strip_suffix(q))
            .filter(|_| name.len() >= 2)
    }) {
        name = inner;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_inserts_the_message() {
        assert!(prompt("hi").contains("User message: hi"));
    }

    #[test]
    fn clean_uses_the_first_non_empty_line() {
        assert_eq!(clean("  Fix auth  "), Some("Fix auth".into()));
        assert_eq!(clean("\n\nsecond line\n"), Some("second line".into()));
        assert_eq!(clean(""), None);
        assert_eq!(clean("   \n  \n"), None);
    }

    #[test]
    fn clean_rejects_names_that_are_too_short() {
        assert_eq!(clean("a"), None);
        assert_eq!(clean("ab"), None);
        assert_eq!(clean("abc"), Some("abc".into()));
    }

    #[test]
    fn clean_strips_surrounding_quotes_and_a_trailing_period() {
        assert_eq!(clean("\"Fix auth bug\""), Some("Fix auth bug".into()));
        assert_eq!(clean("'Fix auth bug'"), Some("Fix auth bug".into()));
        assert_eq!(clean("`Fix auth bug`"), Some("Fix auth bug".into()));
        assert_eq!(clean("Fix auth bug."), Some("Fix auth bug".into()));
        assert_eq!(clean("\"Fix auth bug.\""), Some("Fix auth bug".into()));
        assert_eq!(clean("Only one quote'"), Some("Only one quote'".into()));
    }

    #[test]
    fn clean_truncates_by_chars_not_bytes() {
        let long = "é".repeat(60);
        let name = clean(&long).unwrap();
        assert_eq!(name.chars().count(), 50);
        assert!(name.ends_with("..."));
    }

    #[test]
    fn disambiguate_keeps_free_name() {
        let taken = vec!["Other".to_string()];
        assert_eq!(disambiguate("Fix auth", &taken), "Fix auth");
    }

    #[test]
    fn disambiguate_appends_smallest_suffix() {
        let taken = vec!["Fix auth".to_string()];
        assert_eq!(disambiguate("Fix auth", &taken), "Fix auth #2");
    }

    #[test]
    fn disambiguate_skips_taken_suffixes() {
        let taken = vec!["Fix auth".to_string(), "Fix auth #2".to_string()];
        assert_eq!(disambiguate("Fix auth", &taken), "Fix auth #3");
    }

    #[test]
    fn disambiguate_matches_case_insensitively_and_trims() {
        let taken = vec!["  fix auth  ".to_string(), "fix auth #2".to_string()];
        assert_eq!(disambiguate("FIX AUTH", &taken), "FIX AUTH #3");
    }
}
