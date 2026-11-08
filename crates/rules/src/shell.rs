//! Shell command lines split into simple command segments: quotes, pipes, `$(…)`, wrappers
//! (`sudo`, `env`, `xargs`…) and `sh -c` bodies, so a rule sees each command that runs.

/// The program a path names: `/usr/bin/git` is `git`.
pub(crate) fn basename(path: &str) -> &str {
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

        // A flag that takes a value (`-u`, `-n`, `-I`) may end the line, so the
        // skip can run past the last token.
        tokens.drain(..skip.min(tokens.len()));
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

    #[test]
    fn a_wrapper_flag_ending_the_line_does_not_panic() {
        for line in ["xargs -I", "sudo -u", "nice -n", "ls | xargs -n"] {
            segments(line);
        }
    }
}
