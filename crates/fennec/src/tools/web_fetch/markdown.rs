//! HTML to Markdown, by hand.
// ponytail: headings, code blocks, lists and paragraphs only; links, tables
// and emphasis are dropped to their text. Upgrade path: the `htmd` crate.

const DROPPED: [&str; 7] = [
    "script", "style", "noscript", "head", "svg", "iframe", "template",
];
const BLOCKS: [&str; 17] = [
    "p",
    "div",
    "br",
    "tr",
    "section",
    "article",
    "ul",
    "ol",
    "li",
    "table",
    "header",
    "footer",
    "nav",
    "main",
    "aside",
    "blockquote",
    "hr",
];

pub(super) fn to_markdown(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::new();
    let mut pre = false;
    let mut pos = 0;
    while pos < html.len() {
        let rest = &html[pos..];
        if rest.starts_with("<!--") {
            pos += rest.find("-->").map_or(rest.len(), |end| end + 3);
        } else if is_tag_start(rest) {
            let end = rest.find('>').map_or(rest.len(), |end| end + 1);
            let (name, closing) = tag_name(&rest[1..end.saturating_sub(1).max(1)]);
            pos += end;
            if !closing && DROPPED.contains(&name.as_str()) {
                let close = format!("</{name}");
                pos = lower[pos..].find(&close).map_or(html.len(), |at| {
                    let from = pos + at;
                    from + html[from..]
                        .find('>')
                        .map_or(html.len() - from, |gt| gt + 1)
                });
            } else {
                tag(&mut out, &name, closing, &mut pre);
            }
        } else {
            // Past the first character, not byte: the text may start with a multibyte one.
            let first = rest.chars().next().map_or(1, char::len_utf8);
            let end = rest[first..].find('<').map_or(rest.len(), |at| at + first);
            text(&mut out, &decode(&rest[..end]), pre);
            pos += end;
        }
    }
    tidy(&out)
}

fn is_tag_start(rest: &str) -> bool {
    let mut chars = rest.chars();
    chars.next() == Some('<')
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!')
}

fn tag_name(inner: &str) -> (String, bool) {
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(inner) => (true, inner),
        None => (false, inner),
    };
    let name = inner
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("");
    (name.to_ascii_lowercase(), closing)
}

fn tag(out: &mut String, name: &str, closing: bool, pre: &mut bool) {
    match name {
        "pre" if !closing => {
            newline(out);
            out.push_str("```\n");
            *pre = true;
        }
        "pre" => {
            newline(out);
            out.push_str("```\n");
            *pre = false;
        }
        _ if *pre => {}
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            blank_line(out);
            if !closing {
                let level = name[1..].parse::<usize>().unwrap_or(1);
                out.push_str(&"#".repeat(level));
                out.push(' ');
            }
        }
        "li" if !closing => {
            newline(out);
            out.push_str("- ");
        }
        _ if BLOCKS.contains(&name) => newline(out),
        _ => {}
    }
}

fn newline(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

fn blank_line(out: &mut String) {
    newline(out);
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
}

fn text(out: &mut String, text: &str, pre: bool) {
    if pre {
        out.push_str(text);
        return;
    }
    for c in text.chars() {
        if c.is_whitespace() {
            if !out.is_empty() && !out.ends_with([' ', '\n']) {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
}

/// Trims line ends and collapses runs of blank lines.
fn tidy(out: &str) -> String {
    let mut result = String::new();
    let mut blanks = 0;
    for line in out.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blanks += 1;
            if blanks > 1 {
                continue;
            }
        } else {
            blanks = 0;
        }
        result.push_str(line);
        result.push('\n');
    }
    result.trim().to_string() + "\n"
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest
            .find(';')
            .filter(|&end| end <= 10)
            .and_then(|end| entity(&rest[1..end]).map(|c| (c, end + 1)));
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_starting_with_a_multibyte_character() {
        assert_eq!(to_markdown("<p>§1 été</p><p>&lt;é</p>"), "§1 été\n<é\n");
    }

    #[test]
    fn headings_lists_and_paragraphs() {
        let html = "<html><body><h1>Title</h1><p>Some <b>bold</b>\n text</p><h3 id=x>Sub</h3><ul><li>one</li><li>two</li></ul></body></html>";
        assert_eq!(
            to_markdown(html),
            "# Title\n\nSome bold text\n\n### Sub\n\n- one\n- two\n"
        );
    }

    #[test]
    fn pre_keeps_its_text_in_a_fence() {
        let html = "<p>Run:</p><pre><code>a  b\n  c &lt;x&gt;</code></pre><p>done</p>";
        assert_eq!(to_markdown(html), "Run:\n```\na  b\n  c <x>\n```\ndone\n");
    }

    #[test]
    fn script_style_head_and_comments_are_dropped() {
        let html = "<head><title>T</title></head><script>var a = '<p>';</script><style>p{}</style><!-- hi --><p>kept</p><noscript>no</noscript>";
        assert_eq!(to_markdown(html), "kept\n");
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(
            to_markdown(
                "<p>a &amp; b &lt; c &quot;d&quot; &#39;e&#39; f&nbsp;g &#65;&#x42; &bogus; &</p>"
            ),
            "a & b < c \"d\" 'e' f g AB &bogus; &\n"
        );
    }

    #[test]
    fn blank_runs_collapse() {
        assert_eq!(
            to_markdown("<div></div><div></div><p>a</p><div></div><p>b</p>"),
            "a\nb\n"
        );
    }
}
