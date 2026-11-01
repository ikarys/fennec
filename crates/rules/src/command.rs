// ponytail: token matching is a guard rail, not a sandbox: eval, variables ($X -rf), aliases and a script written then run all bypass it. The bwrap sandbox stays the hard boundary.
//! Command-line tokenization and command-pattern matching.

use std::fmt;

/// A human-readable pattern parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError(pub String);

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PatternError {}

/// A parsed command pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPattern {
    /// The pattern as the human typed it, trimmed: stored and shown back.
    text: String,
    program: String,
    words: Vec<String>,
    flags: Vec<String>,
}

impl CommandPattern {
    /// Parses a single-segment command pattern.
    pub fn parse(pattern: &str) -> Result<Self, PatternError> {
        let parsed_segments = segments(pattern);
        if parsed_segments.is_empty() {
            return Err(PatternError("empty pattern".to_string()));
        }
        if parsed_segments.len() > 1 {
            return Err(PatternError(
                "pattern contains several segments".to_string(),
            ));
        }

        let tokens = &parsed_segments[0];
        if tokens.is_empty() {
            return Err(PatternError("empty pattern".to_string()));
        }

        let raw_program = &tokens[0];
        if raw_program.is_empty() {
            return Err(PatternError("empty program".to_string()));
        }
        if raw_program.starts_with('-') {
            return Err(PatternError("program must not start with '-'".to_string()));
        }

        let program = basename(raw_program).to_string();
        let mut words = Vec::new();
        let mut flags = Vec::new();

        for token in &tokens[1..] {
            if token.starts_with('-') {
                flags.extend(flags_of(token));
            } else {
                words.push(token.clone());
            }
        }

        flags.sort();
        flags.dedup();

        Ok(Self {
            text: pattern.trim().to_string(),
            program,
            words,
            flags,
        })
    }

    /// Returns true when this pattern matches one simple command segment.
    pub fn matches(&self, segment: &[String]) -> bool {
        if segment.is_empty() {
            return false;
        }
        if basename(&segment[0]) != self.program {
            return false;
        }

        let (segment_flags, segment_words) = segment_flags_and_words(segment);
        if !self.flags.iter().all(|flag| segment_flags.contains(flag)) {
            return false;
        }

        words_are_in_order_subsequence(&segment_words, &self.words)
    }

    /// Returns a human-readable description of how fennec understood the pattern.
    pub fn describe(&self) -> String {
        let program = &self.program;
        let mut parts = vec![format!("program {program}")];

        if !self.words.is_empty() {
            parts.push(format!("words {}", self.words.join(" ")));
        }
        if !self.flags.is_empty() {
            parts.push(format!("flags {}", self.flags.join(" ")));
        }

        parts.join(", ")
    }

    /// The pattern as typed (trimmed).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the program basename.
    pub fn program(&self) -> &str {
        &self.program
    }
}

fn flags_of(token: &str) -> Vec<String> {
    if token == "--" {
        return Vec::new();
    }

    if let Some(rest) = token.strip_prefix("--") {
        let name = rest.split('=').next().unwrap_or("");
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return vec![name.to_string()];
        }
        return Vec::new();
    }

    if let Some(rest) = token.strip_prefix('-')
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return rest.chars().map(|c| c.to_string()).collect();
    }

    Vec::new()
}

fn segment_flags_and_words(segment: &[String]) -> (Vec<String>, Vec<String>) {
    let mut flags = Vec::new();
    let mut words = Vec::new();
    let mut after_double_dash = false;

    for token in segment.iter().skip(1) {
        if after_double_dash {
            words.push(token.clone());
            continue;
        }

        if token == "--" {
            after_double_dash = true;
            continue;
        }

        if token.starts_with('-') {
            flags.extend(flags_of(token));
        } else {
            words.push(token.clone());
        }
    }

    flags.sort();
    flags.dedup();
    (flags, words)
}

fn words_are_in_order_subsequence(haystack: &[String], needle: &[String]) -> bool {
    let mut haystack_iter = haystack.iter();
    needle
        .iter()
        .all(|wanted| haystack_iter.by_ref().any(|actual| actual == wanted))
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Splits a shell command line into simple command segments.
pub fn segments(command: &str) -> Vec<Vec<String>> {
    segments_inner(command, 0)
}

fn segments_inner(command: &str, depth: usize) -> Vec<Vec<String>> {
    let raw_segments = tokenize(command, depth);
    let mut out = Vec::new();

    for segment in raw_segments {
        out.extend(process_segment(segment, depth));
    }

    out
}

fn process_segment(tokens: Vec<String>, depth: usize) -> Vec<Vec<String>> {
    let tokens = strip_wrappers(tokens);
    if tokens.is_empty() {
        return Vec::new();
    }

    if let Some(replacement) = shell_c_segments(&tokens, depth) {
        return replacement;
    }

    let mut out = vec![tokens.clone()];
    out.extend(find_exec_segments(&tokens, depth));
    out
}

fn strip_wrappers(mut tokens: Vec<String>) -> Vec<String> {
    loop {
        let mut skip = 0;

        while skip < tokens.len() {
            let token = &tokens[skip];

            if is_env_assignment(token) {
                skip += 1;
                continue;
            }

            match token.as_str() {
                "sudo" | "doas" => {
                    skip += 1;
                    while skip < tokens.len() && tokens[skip].starts_with('-') {
                        match tokens[skip].as_str() {
                            "-u" | "-g" => skip += 2,
                            _ => skip += 1,
                        }
                    }
                }
                "env" => {
                    skip += 1;
                    while skip < tokens.len() {
                        if tokens[skip] == "-x" || is_env_assignment(&tokens[skip]) {
                            skip += 1;
                        } else {
                            break;
                        }
                    }
                }
                "command" | "exec" | "nohup" | "builtin" => {
                    skip += 1;
                }
                "time" => {
                    skip += 1;
                    if skip < tokens.len() && tokens[skip] == "-p" {
                        skip += 1;
                    }
                }
                "nice" => {
                    skip += 1;
                    if skip < tokens.len() && tokens[skip] == "-n" {
                        skip += 2;
                    } else if skip < tokens.len()
                        && tokens[skip].starts_with('-')
                        && tokens[skip].len() > 1
                    {
                        skip += 1;
                    }
                }
                "timeout" => {
                    skip += 1;
                    while skip < tokens.len() && tokens[skip].starts_with('-') {
                        skip += 1;
                    }
                    if skip < tokens.len() {
                        skip += 1;
                    }
                }
                "xargs" => {
                    skip += 1;
                    while skip < tokens.len() && tokens[skip].starts_with('-') {
                        match tokens[skip].as_str() {
                            "-I" | "-n" | "-P" | "-L" | "-d" => skip += 2,
                            _ => skip += 1,
                        }
                    }
                }
                _ => break,
            }
        }

        if skip == 0 {
            break;
        }

        tokens.drain(..skip);
    }

    tokens
}

fn is_env_assignment(token: &str) -> bool {
    let Some(eq) = token.find('=') else {
        return false;
    };

    let name = &token[..eq];
    if name.is_empty() {
        return false;
    }

    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn shell_c_segments(segment: &[String], depth: usize) -> Option<Vec<Vec<String>>> {
    if depth >= 8 || segment.is_empty() {
        return None;
    }

    let program = basename(&segment[0]);
    if !matches!(program, "sh" | "bash" | "zsh" | "dash") {
        return None;
    }

    for (i, token) in segment.iter().enumerate().skip(1) {
        if is_c_flag(token)
            && let Some(script) = segment.get(i + 1)
        {
            return Some(segments_inner(script, depth + 1));
        }
    }

    None
}

fn is_c_flag(token: &str) -> bool {
    let Some(rest) = token.strip_prefix('-') else {
        return false;
    };

    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphabetic()) && rest.contains('c')
}

fn find_exec_segments(segment: &[String], depth: usize) -> Vec<Vec<String>> {
    if segment.is_empty() || basename(&segment[0]) != "find" {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut i = 1;

    while i < segment.len() {
        if matches!(segment[i].as_str(), "-exec" | "-execdir" | "-ok" | "-okdir") {
            i += 1;
            let mut extra = Vec::new();

            while i < segment.len() {
                let token = segment[i].clone();
                if token == ";" || token == "+" {
                    i += 1;
                    break;
                }
                extra.push(token);
                i += 1;
            }

            out.extend(process_segment(extra, depth));
        } else {
            i += 1;
        }
    }

    out
}

fn tokenize(input: &str, depth: usize) -> Vec<Vec<String>> {
    let mut segments: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut token = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                token.push(ch);
            }
            continue;
        }

        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    if let Some(next) = chars.peek() {
                        if matches!(next, '"' | '\\' | '$' | '`') {
                            token.push(*next);
                            chars.next();
                        } else {
                            token.push('\\');
                        }
                    } else {
                        token.push('\\');
                    }
                }
                '$' if chars.peek() == Some(&'(') => {
                    chars.next();
                    if let Some(inner) = scan_paren(&mut chars, depth) {
                        if depth < 8 {
                            segments.extend(tokenize(&inner, depth + 1));
                        }
                        token.push_str(&format!("$({inner})"));
                    }
                }
                '`' => {
                    if let Some(inner) = scan_backtick(&mut chars, depth) {
                        if depth < 8 {
                            segments.extend(tokenize(&inner, depth + 1));
                        }
                        token.push_str(&format!("`{inner}`"));
                    }
                }
                _ => token.push(ch),
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '\\' => {
                if let Some(next) = chars.next() {
                    token.push(next);
                }
            }
            ';' | '&' | '|' | '\n' | '(' | ')' | '{' | '}' | '`' => {
                push_token(&mut token, &mut current);
                push_segment(&mut current, &mut segments);
            }
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                push_token(&mut token, &mut current);
                push_segment(&mut current, &mut segments);
                if let Some(inner) = scan_paren(&mut chars, depth)
                    && depth < 8
                {
                    segments.extend(tokenize(&inner, depth + 1));
                }
            }
            ' ' | '\t' | '\r' => {
                push_token(&mut token, &mut current);
            }
            _ => token.push(ch),
        }
    }

    push_token(&mut token, &mut current);
    push_segment(&mut current, &mut segments);
    segments
}

fn push_token(token: &mut String, current: &mut Vec<String>) {
    if !token.is_empty() {
        current.push(std::mem::take(token));
    }
}

fn push_segment(current: &mut Vec<String>, segments: &mut Vec<Vec<String>>) {
    if !current.is_empty() {
        segments.push(std::mem::take(current));
    }
}

fn scan_paren(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    depth: usize,
) -> Option<String> {
    let mut inner = String::new();
    let mut paren_depth = 1usize;
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            inner.push(ch);
            continue;
        }

        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    if let Some(next) = chars.peek() {
                        if matches!(next, '"' | '\\' | '$' | '`') {
                            inner.push(*next);
                            chars.next();
                        } else {
                            inner.push('\\');
                        }
                    } else {
                        inner.push('\\');
                    }
                }
                '$' if chars.peek() == Some(&'(') && depth < 8 => {
                    chars.next();
                    if let Some(nested) = scan_paren(chars, depth + 1) {
                        inner.push_str(&format!("$({nested})"));
                    }
                }
                '`' if depth < 8 => {
                    if let Some(nested) = scan_backtick(chars, depth + 1) {
                        inner.push_str(&format!("`{nested}`"));
                    }
                }
                _ => inner.push(ch),
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '\\' => {
                if let Some(next) = chars.next() {
                    inner.push(next);
                }
            }
            '(' => {
                paren_depth += 1;
                inner.push(ch);
            }
            ')' => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    return Some(inner);
                }
                inner.push(ch);
            }
            '$' if chars.peek() == Some(&'(') && depth < 8 => {
                chars.next();
                if let Some(nested) = scan_paren(chars, depth + 1) {
                    inner.push_str(&format!("$({nested})"));
                }
            }
            '`' if depth < 8 => {
                if let Some(nested) = scan_backtick(chars, depth + 1) {
                    inner.push_str(&format!("`{nested}`"));
                }
            }
            _ => inner.push(ch),
        }
    }

    None
}

fn scan_backtick(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    depth: usize,
) -> Option<String> {
    let mut inner = String::new();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            inner.push(ch);
            continue;
        }

        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    if let Some(next) = chars.peek() {
                        if matches!(next, '"' | '\\' | '$' | '`') {
                            inner.push(*next);
                            chars.next();
                        } else {
                            inner.push('\\');
                        }
                    } else {
                        inner.push('\\');
                    }
                }
                '$' if chars.peek() == Some(&'(') && depth < 8 => {
                    chars.next();
                    if let Some(nested) = scan_paren(chars, depth + 1) {
                        inner.push_str(&format!("$({nested})"));
                    }
                }
                '`' if depth < 8 => {
                    if let Some(nested) = scan_backtick(chars, depth + 1) {
                        inner.push_str(&format!("`{nested}`"));
                    }
                }
                _ => inner.push(ch),
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '\\' => {
                if let Some(next) = chars.next() {
                    inner.push(next);
                }
            }
            '`' => return Some(inner),
            '$' if chars.peek() == Some(&'(') && depth < 8 => {
                chars.next();
                if let Some(nested) = scan_paren(chars, depth + 1) {
                    inner.push_str(&format!("$({nested})"));
                }
            }
            _ => inner.push(ch),
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(text: &str) -> CommandPattern {
        CommandPattern::parse(text).unwrap()
    }

    fn matches_any(command: &str, expected: &CommandPattern) -> bool {
        segments(command)
            .iter()
            .any(|segment| expected.matches(segment))
    }

    #[test]
    fn rm_rf_matches_expected_commands() {
        let p = pattern("rm -rf");

        assert!(matches_any("rm -fr x", &p));
        assert!(matches_any("rm -r -f x", &p));
        assert!(!matches_any("rm -Rf x", &p));
        assert!(matches_any("/bin/rm -rf x", &p));
        assert!(matches_any("sudo rm -rf /", &p));
        assert!(matches_any("sudo -u root rm -rf /", &p));
        assert!(matches_any("a && rm -rf b", &p));
        assert!(matches_any("ls | xargs rm -rf", &p));
        assert!(matches_any("bash -c 'rm -rf b'", &p));
        assert!(matches_any("bash -lc \"cd x && rm -rf b\"", &p));
        assert!(matches_any("find . -name x -exec rm -rf {} \\;", &p));
        assert!(matches_any("echo $(rm -rf b)", &p));
        assert!(matches_any("echo \"$(rm -rf b)\"", &p));
        assert!(matches_any("FOO=1 env BAR=2 nohup rm -rf b", &p));
        assert!(matches_any("timeout 5 rm -rf b", &p));
    }

    #[test]
    fn rm_rf_does_not_match_non_matches() {
        let p = pattern("rm -rf");

        assert!(!matches_any("echo \"rm -rf\"", &p));
        assert!(!matches_any("echo 'rm -rf b'", &p));
        assert!(!matches_any("rm -r x", &p));
        assert!(!matches_any("rmdir -rf x", &p));
        assert!(!matches_any("grep rm -rf", &p));
    }

    #[test]
    fn git_push_force_matches() {
        let p = pattern("git push --force");

        assert!(matches_any("git push origin main --force", &p));
        assert!(matches_any("git -C x push --force", &p));
        assert!(!matches_any("git push origin main", &p));
        assert!(!matches_any("git pull --force", &p));
    }

    #[test]
    fn parse_errors() {
        assert!(CommandPattern::parse("").is_err());
        assert!(CommandPattern::parse("-rf").is_err());
        assert!(CommandPattern::parse("a; b").is_err());
    }

    #[test]
    fn describe_git_push_force() {
        let p = pattern("git push --force");
        assert_eq!(p.describe(), "program git, words push, flags force");
    }
}
