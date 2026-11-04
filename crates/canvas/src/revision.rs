//! The later versions of an artifact pinned from a message block: the blocks of the same
//! format the agent wrote after the one the latest version holds.

use crate::format::Format;

/// The fenced code blocks of `text` whose language is `format`, in order, each without its fences.
pub fn blocks(text: &str, format: Format) -> Vec<String> {
    let mut result = Vec::new();
    let mut lines = text.lines();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let Some(open_count) = leading_backticks(trimmed) else {
            continue;
        };

        let info = &trimmed[open_count..];
        let is_target =
            info.split_whitespace().next().and_then(Format::of_language) == Some(format);

        if !is_target {
            skip_to_close(&mut lines, open_count);
            continue;
        }

        let mut content = Vec::new();
        let mut closed = false;

        for line in &mut lines {
            if is_closing_fence(line, open_count) {
                closed = true;
                break;
            }
            content.push(line.to_string());
        }

        if closed {
            result.push(content.join("\n"));
        }
    }

    result
}

/// The blocks of `format` in `texts` (the session's assistant texts, oldest first) that come
/// after the last block equal to `latest` (compared with surrounding whitespace trimmed),
/// in order. Empty when `latest` is not found among the blocks.
pub fn revisions(texts: &[String], format: Format, latest: &[u8]) -> Vec<String> {
    let latest = String::from_utf8_lossy(latest).into_owned();
    let latest = latest.trim();

    let mut all = Vec::new();
    for text in texts {
        all.extend(blocks(text, format));
    }

    match all.iter().rposition(|block| block.trim() == latest) {
        Some(index) => all[index + 1..].to_vec(),
        None => Vec::new(),
    }
}

fn leading_backticks(line: &str) -> Option<usize> {
    let count = line.bytes().take_while(|&b| b == b'`').count();
    (count >= 3).then_some(count)
}

fn is_closing_fence(line: &str, open_count: usize) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= open_count && trimmed.bytes().all(|b| b == b'`')
}

fn skip_to_close<'a>(lines: &mut impl Iterator<Item = &'a str>, open_count: usize) {
    for line in lines {
        if is_closing_fence(line, open_count) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{blocks, revisions};
    use crate::format::Format;

    #[test]
    fn finds_two_html_blocks_in_one_text() {
        let text = "```html\n<a>\n```\n```html\n<b>\n```";
        assert_eq!(blocks(text, Format::Html), vec!["<a>", "<b>"]);
    }

    #[test]
    fn skips_a_js_block() {
        let text = "```js\nlet x = 1;\n```\n```html\nok\n```";
        assert_eq!(blocks(text, Format::Html), vec!["ok"]);
    }

    #[test]
    fn skips_nested_html_inside_md() {
        let text = "````md\n```html\ninner\n```\n````";
        assert_eq!(blocks(text, Format::Markdown), vec!["```html\ninner\n```"]);
        assert_eq!(blocks(text, Format::Html), Vec::<String>::new());
    }

    #[test]
    fn ignores_an_unclosed_block() {
        let text = "```html\none\n```\n```html\nopen";
        assert_eq!(blocks(text, Format::Html), vec!["one"]);
    }

    #[test]
    fn returns_blocks_after_latest_across_texts() {
        let texts = vec![
            "```html\nfirst\n```".to_string(),
            "```html\nlatest\n```\n```html\nnext\n```".to_string(),
            "```html\nafter\n```".to_string(),
        ];
        assert_eq!(
            revisions(&texts, Format::Html, b"latest"),
            vec!["next", "after"]
        );
    }

    #[test]
    fn returns_empty_when_latest_is_unknown() {
        let texts = vec!["```html\na\n```".to_string()];
        assert_eq!(revisions(&texts, Format::Html, b"b"), Vec::<String>::new());
    }

    #[test]
    fn matches_latest_despite_a_trailing_newline() {
        let texts = vec!["```html\ncontent\n```\n```html\nnext\n```".to_string()];
        assert_eq!(revisions(&texts, Format::Html, b"content\n"), vec!["next"]);
    }
}
