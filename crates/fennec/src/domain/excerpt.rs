//! A line quoted to the human, cut around what it was flagged for: a minified
//! line holds hundreds of thousands of characters on one row.

/// Characters a quoted line keeps, ellipses aside.
const MAX: usize = 160;

/// `line` when it fits in MAX characters, else the MAX characters around the
/// first `needle`, with `…` where it was cut.
pub fn around(line: &str, needle: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= MAX {
        return line.to_string();
    }
    let at = line
        .find(needle)
        .map_or(0, |byte| line[..byte].chars().count());
    let start = at.saturating_sub(MAX / 2).min(chars.len() - MAX);
    let end = start + MAX;
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_line_is_kept_whole() {
        assert_eq!(
            around("// eslint-disable", "eslint-disable"),
            "// eslint-disable"
        );
    }

    #[test]
    fn a_long_line_keeps_the_needle_in_the_middle() {
        let line = format!("{}é eslint-disable {}", "a".repeat(5000), "b".repeat(5000));
        let quoted = around(&line, "eslint-disable");
        assert!(quoted.contains("eslint-disable"));
        assert!(quoted.starts_with('…') && quoted.ends_with('…'));
        assert_eq!(quoted.chars().count(), MAX + 2);
    }

    #[test]
    fn a_needle_near_an_end_is_not_cut_off() {
        let line = format!("@ts-ignore {}", "x".repeat(1000));
        let quoted = around(&line, "@ts-ignore");
        assert!(quoted.starts_with("@ts-ignore"));
        assert_eq!(quoted.chars().count(), MAX + 1);
    }
}
