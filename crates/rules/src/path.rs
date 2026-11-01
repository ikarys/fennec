//! The gitignore-like path matcher shared by path rules and fennec's protected files.
//! `*` and `?` match within one segment, a whole `**` segment matches zero or more,
//! a pattern containing a `/` is anchored at the root, a trailing `/` is dropped
//! (a directory pattern covers everything under it), `#` and `!` lines are no pattern.

/// The pattern trimmed and without its trailing `/`; None for a blank, comment or negation line.
pub fn normalize(pattern: &str) -> Option<String> {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern.starts_with('#') || pattern.starts_with('!') {
        return None;
    }

    // A trailing `/` is dropped: a directory pattern protects everything under
    // it anyway. A leading `/` is kept because it anchors the pattern.
    let pattern = pattern.strip_suffix('/').unwrap_or(pattern);
    if pattern.is_empty() {
        None
    } else {
        Some(pattern.to_string())
    }
}

fn split_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn pattern_matches(pattern: &str, path_segments: &[&str]) -> bool {
    let anchored = pattern.starts_with('/') || pattern.contains('/');
    let body = pattern.strip_prefix('/').unwrap_or(pattern);
    let pattern_segments = split_segments(body);
    if pattern_segments.is_empty() {
        return false;
    }

    let full_segments: Vec<&str> = if anchored {
        pattern_segments
    } else {
        let mut full_segments = Vec::with_capacity(pattern_segments.len() + 1);
        full_segments.push("**");
        full_segments.extend(pattern_segments);
        full_segments
    };

    match_segments(&full_segments, path_segments)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }

    if pattern[0] == "**" {
        // `**` as a whole segment matches zero or more path segments.
        if match_segments(&pattern[1..], path) {
            return true;
        }
        if !path.is_empty() && match_segments(pattern, &path[1..]) {
            return true;
        }
        return false;
    }

    if path.is_empty() {
        return false;
    }

    if segment_matches(pattern[0], path[0]) {
        match_segments(&pattern[1..], &path[1..])
    } else {
        false
    }
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    glob_match(&pattern, &text)
}

fn glob_match(pattern: &[char], text: &[char]) -> bool {
    let mut pattern_index = 0;
    let mut text_index = 0;
    let mut star_pattern_index = None;
    let mut star_text_index = 0;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star_pattern_index = Some(pattern_index);
            star_text_index = text_index;
            pattern_index += 1;
        } else if let Some(star) = star_pattern_index {
            pattern_index = star + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

/// True when `pattern` (normalized) matches `relative` (`/`-separated, no leading `./`)
/// or one of its ancestor directories.
pub fn covers(pattern: &str, relative: &str) -> bool {
    let path_segments = split_segments(relative);
    (1..=path_segments.len()).any(|length| pattern_matches(pattern, &path_segments[..length]))
}
