//! Ports openfox session/name-generator: the prompt and the name's cleanup.
//! Not ported: the disableAutoSessionTitle switch, the 'Session N' default check.

/// openfox `SESSION_NAME_PROMPT`: the only context sent to the model.
pub const SESSION_NAME_PROMPT: &str =
    "Generate a concise, descriptive session name (max 50 characters) based on the user's message.
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
}
