//! The pointer the model gets instead of the page: headings with line
//! numbers, and the first lines.

const MAX_HEADINGS: usize = 80;
const FIRST_LINES: usize = 20;
/// A page can be one huge line (minified JSON, a single-line HTML dump): the
/// pointer stays small whatever the page's shape.
const MAX_LINE_BYTES: usize = 200;
const FIRST_LINES_BYTES: usize = 2048;

fn clip(line: &str, bytes: usize) -> &str {
    &line[..line.floor_char_boundary(bytes)]
}

/// `(line, heading)` for each ATX heading outside fenced code blocks.
fn headings(markdown: &str) -> Vec<(usize, &str)> {
    let mut fence: Option<&str> = None;
    let mut found = Vec::new();
    for (index, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (None, Some(marker)) => fence = Some(marker),
            (Some(open), Some(marker)) if open == marker => fence = None,
            (None, None) if is_heading(line) => found.push((index + 1, line.trim_end())),
            _ => {}
        }
    }
    found
}

fn is_heading(line: &str) -> bool {
    let hashes = line.bytes().take_while(|&byte| byte == b'#').count();
    (1..=6).contains(&hashes) && line[hashes..].starts_with(' ')
}

pub(super) fn outline(markdown: &str) -> String {
    let all = headings(markdown);
    if all.is_empty() {
        return "Outline: none (no headings); search it with rg.".into();
    }
    let mut text = String::from("Outline (line: heading):");
    for (line, heading) in all.iter().take(MAX_HEADINGS) {
        text.push_str(&format!("\n{line}: {}", clip(heading, MAX_LINE_BYTES)));
    }
    if all.len() > MAX_HEADINGS {
        text.push_str(&format!("\n… {} more headings", all.len() - MAX_HEADINGS));
    }
    text
}

pub(super) fn first_lines(markdown: &str) -> String {
    let lines = markdown
        .lines()
        .take(FIRST_LINES)
        .map(|line| clip(line, MAX_LINE_BYTES))
        .collect::<Vec<_>>()
        .join("\n");
    clip(&lines, FIRST_LINES_BYTES).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_headings_with_line_numbers_and_skips_fenced_code() {
        let md = "# Title\ntext\n```sh\n# a comment\n```\n## Next\n#nospace\n";
        assert_eq!(
            outline(md),
            "Outline (line: heading):\n1: # Title\n6: ## Next"
        );
    }

    #[test]
    fn no_heading_says_so() {
        assert_eq!(
            outline("plain\ntext"),
            "Outline: none (no headings); search it with rg."
        );
    }

    #[test]
    fn caps_the_outline() {
        let md: String = (0..85).map(|n| format!("# h{n}\n")).collect();
        let text = outline(&md);
        assert_eq!(text.lines().count(), 1 + 80 + 1);
        assert!(text.ends_with("… 5 more headings"));
    }

    #[test]
    fn first_lines_are_capped() {
        let md: String = (0..30).map(|n| format!("l{n}\n")).collect();
        assert_eq!(first_lines(&md).lines().count(), 20);
    }

    #[test]
    fn a_single_huge_line_keeps_the_pointer_small() {
        let md = format!("# {}\n{}", "é".repeat(500), "x".repeat(400_000));
        assert!(outline(&md).len() < MAX_LINE_BYTES + 40);
        assert!(first_lines(&md).len() <= FIRST_LINES_BYTES);
        let wide: String = (0..20).map(|_| format!("{}\n", "y".repeat(150))).collect();
        assert!(first_lines(&wide).len() <= FIRST_LINES_BYTES);
    }
}
