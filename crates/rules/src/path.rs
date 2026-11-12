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

/// True when the normalized `pattern` matches `relative` or one of its ancestor
/// directories; `relative` is `/`-separated, without a leading `./`.
pub fn covers(pattern: &str, relative: &str) -> bool {
    let path_segments = split_segments(relative);
    (1..=path_segments.len()).any(|length| pattern_matches(pattern, &path_segments[..length]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_is_trimmed() {
        assert_eq!(
            normalize("  node_modules  "),
            Some(String::from("node_modules"))
        );
    }

    #[test]
    fn a_trailing_slash_is_dropped() {
        assert_eq!(normalize("docs/"), Some(String::from("docs")));
    }

    #[test]
    fn a_leading_slash_is_kept() {
        assert_eq!(normalize("/Cargo.lock"), Some(String::from("/Cargo.lock")));
    }

    #[test]
    fn a_blank_line_is_none() {
        assert_eq!(normalize("   "), None);
    }

    #[test]
    fn a_comment_line_is_none() {
        assert_eq!(normalize("# ignore"), None);
    }

    #[test]
    fn a_negation_line_is_none() {
        assert_eq!(normalize("!keep"), None);
    }

    #[test]
    fn a_lone_slash_is_none() {
        // A lone slash becomes blank after the trailing slash is dropped.
        assert_eq!(normalize("/"), None);
    }

    #[test]
    fn an_unanchored_name_matches_at_any_depth() {
        assert!(covers("node_modules", "a/node_modules/x/y.js"));
    }

    #[test]
    fn an_anchored_root_pattern_matches_only_at_root() {
        assert!(covers("/Cargo.lock", "Cargo.lock"));
        assert!(!covers("/Cargo.lock", "sub/Cargo.lock"));
    }

    #[test]
    fn an_inner_slash_pattern_is_anchored_at_root() {
        assert!(covers(".agents/protected", ".agents/protected"));
        assert!(!covers(".agents/protected", "x/.agents/protected"));
    }

    #[test]
    fn a_star_matches_within_one_segment_only() {
        assert!(covers("*.lock", "Cargo.lock"));
        assert!(covers("*.lock", "sub/Cargo.lock"));
        assert!(!covers("*.lock", "a/b"));
    }

    #[test]
    fn a_question_mark_matches_one_character() {
        assert!(covers("a?c", "abc"));
        assert!(!covers("a?c", "ac"));
    }

    #[test]
    fn a_double_star_prefix_matches_any_depth() {
        assert!(covers("**/build", "build"));
        assert!(covers("**/build", "a/build"));
    }

    #[test]
    fn a_double_star_middle_matches_zero_segments() {
        assert!(covers("src/**/mod.rs", "src/mod.rs"));
    }

    #[test]
    fn a_double_star_middle_matches_several_segments() {
        assert!(covers("src/**/mod.rs", "src/a/b/mod.rs"));
    }

    #[test]
    fn a_directory_pattern_covers_descendants() {
        assert!(covers("docs", "docs/adr/001.md"));
    }

    #[test]
    fn a_non_matching_path_is_not_covered() {
        assert!(!covers("Cargo.lock", "Cargo.toml"));
    }

    #[test]
    fn a_file_name_starting_with_the_pattern_is_not_covered() {
        assert!(!covers("Cargo.lock", "Cargo.lock.bak"));
    }

    #[test]
    fn empty_path_segments_are_ignored() {
        assert!(covers("a/b", "a//b"));
    }
}
