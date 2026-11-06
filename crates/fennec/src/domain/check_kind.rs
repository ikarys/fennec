//! What a criterion's check proves: a command that only searches the code proves the code exists, not that it works.

/// A check is existence-only when every command it runs only searches the code.
pub fn is_existence_only(check: &str) -> bool {
    if check.trim().is_empty() {
        return false;
    }

    let mut any_segment = false;
    for segment in segments(check) {
        if segment.trim().is_empty() {
            continue;
        }
        any_segment = true;
        if !is_search_command(command_of(&segment)) {
            return false;
        }
    }

    any_segment
}

fn segments(check: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut chars = check.chars().peekable();

    while let Some(c) = chars.next() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }

        match c {
            '\\' if !in_single => {
                current.push(c);
                escaped = true;
            }
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(c);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(c);
            }
            '\n' if !in_single && !in_double => {
                segments.push(current.clone());
                current.clear();
            }
            ';' if !in_single && !in_double => {
                segments.push(current.clone());
                current.clear();
            }
            '&' if !in_single && !in_double => {
                if chars.peek() == Some(&'&') {
                    chars.next();
                    segments.push(current.clone());
                    current.clear();
                } else {
                    current.push(c);
                }
            }
            '|' if !in_single && !in_double => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                segments.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }

    segments.push(current);
    segments
}

fn command_of(segment: &str) -> Option<String> {
    let mut rest = segment.trim();
    while let Some(stripped) = rest.strip_prefix('!') {
        rest = stripped.trim_start();
    }

    tokens(rest)
        .into_iter()
        .find(|token| !is_env_assignment(token))
        .map(|token| unquote(&token).to_string())
}

fn tokens(segment: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut has_token = false;

    for c in segment.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }

        match c {
            '\\' if !in_single => {
                current.push(c);
                escaped = true;
            }
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(c);
                has_token = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(c);
                has_token = true;
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if has_token {
                    tokens.push(current.clone());
                    current.clear();
                    has_token = false;
                }
            }
            _ => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if has_token {
        tokens.push(current);
    }

    tokens
}

fn is_env_assignment(token: &str) -> bool {
    let Some(eq) = token.find('=') else {
        return false;
    };
    let name = &token[..eq];
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

fn unquote(token: &str) -> &str {
    if token.len() >= 2
        && ((token.starts_with('\'') && token.ends_with('\''))
            || (token.starts_with('"') && token.ends_with('"')))
    {
        return &token[1..token.len() - 1];
    }
    token
}

fn is_search_command(command: Option<String>) -> bool {
    let Some(command) = command else {
        return false;
    };
    let name = command.rsplit('/').next().unwrap_or(&command);
    matches!(
        name,
        "rg" | "grep"
            | "egrep"
            | "fgrep"
            | "test"
            | "["
            | "[["
            | "ls"
            | "find"
            | "wc"
            | "head"
            | "tail"
            | "cat"
    )
}

#[cfg(test)]
mod tests {
    use super::is_existence_only;

    #[test]
    fn rg_search_is_existence_only() {
        assert!(is_existence_only("rg -q 'fn accounts' src/api.rs"));
    }

    #[test]
    fn multiple_rg_segments_are_existence_only() {
        assert!(is_existence_only(
            "rg -q 'route\\(\"/api/accounts\"' src/api.rs && rg -q 'async fn accounts' src/api.rs"
        ));
    }

    #[test]
    fn negated_rg_is_existence_only() {
        assert!(is_existence_only(
            "! rg -n 'let head = &line\\[\\.\\.10\\]' src/parse/pdf.rs"
        ));
    }

    #[test]
    fn test_file_is_existence_only() {
        assert!(is_existence_only("test -f docs/map.md"));
    }

    #[test]
    fn pipe_to_wc_is_existence_only() {
        assert!(is_existence_only("rg -c foo src | wc -l"));
    }

    #[test]
    fn cargo_test_is_not_existence_only() {
        assert!(!is_existence_only("cargo test csv_parse"));
    }

    #[test]
    fn path_cargo_test_is_not_existence_only() {
        assert!(!is_existence_only("~/.cargo/bin/cargo test --lib x"));
    }

    #[test]
    fn mixed_rg_and_cargo_is_not_existence_only() {
        assert!(!is_existence_only(
            "rg -q 'pub provider' src/llm.rs && ~/.cargo/bin/cargo test --lib rango_settings_roundtrip"
        ));
    }

    #[test]
    fn operators_inside_quotes_do_not_split() {
        assert!(is_existence_only("rg -q 'a|b;c' src"));
    }

    #[test]
    fn env_assignment_before_cargo_is_not_existence_only() {
        assert!(!is_existence_only("RUST_LOG=off cargo test"));
    }

    #[test]
    fn npm_is_not_existence_only() {
        assert!(!is_existence_only("npm --prefix web run build"));
    }

    #[test]
    fn empty_and_whitespace_checks_are_not_existence_only() {
        assert!(!is_existence_only(""));
        assert!(!is_existence_only("   "));
    }
}
