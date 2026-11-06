//! The changelog release-please writes, read back as releases: what the About tab shows,
//! and which releases arrived since the human last looked.
use crate::domain::release::Version;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Release {
    pub version: String,
    pub date: Option<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub title: String,
    pub items: Vec<String>,
}

/// The releases, newest first as the file lists them.
pub fn parse(markdown: &str) -> Vec<Release> {
    let mut releases: Vec<Release> = Vec::new();
    let mut current: Option<Release> = None;
    let mut current_section: Option<usize> = None;

    for line in markdown.lines() {
        if line.starts_with("## ") {
            if let Some(release) = current.take() {
                releases.push(release);
            }

            let rest = line.strip_prefix("## ").unwrap_or("");
            current = Some(Release {
                version: release_version(rest),
                date: release_date(line),
                sections: Vec::new(),
            });
            current_section = None;
            continue;
        }

        if line.starts_with("### ") {
            if let Some(release) = current.as_mut() {
                let title = line.strip_prefix("### ").unwrap_or("").trim().to_string();
                release.sections.push(Section {
                    title,
                    items: Vec::new(),
                });
                current_section = Some(release.sections.len() - 1);
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("* ").or_else(|| line.strip_prefix("- ")) {
            if let Some(release) = current.as_mut()
                && let Some(section) = current_section
            {
                let item = clean_item(rest);
                if !item.is_empty() {
                    release.sections[section].items.push(item);
                }
            }
            continue;
        }
    }

    if let Some(release) = current {
        releases.push(release);
    }

    releases
}

/// How many releases are newer than `seen` and not newer than `current`.
/// `seen` or `current` that does not parse counts nothing.
pub fn unseen(releases: &[Release], seen: &str, current: &str) -> usize {
    let Some(seen) = Version::parse(seen) else {
        return 0;
    };
    let Some(current) = Version::parse(current) else {
        return 0;
    };

    releases
        .iter()
        .filter(|release| {
            Version::parse(&release.version)
                .is_some_and(|version| version > seen && version <= current)
        })
        .count()
}

fn release_version(rest: &str) -> String {
    if rest.starts_with('[')
        && let Some(end) = rest.find(']')
    {
        return rest[1..end].to_string();
    }

    rest.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}

fn release_date(line: &str) -> Option<String> {
    let open = line.rfind('(')?;
    let close = line[open + 1..].rfind(')')?;
    let inner = line[open + 1..open + 1 + close].trim();

    if is_date(inner) {
        Some(inner.to_string())
    } else {
        None
    }
}

fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && (0..4).all(|i| bytes[i].is_ascii_digit())
        && (5..7).all(|i| bytes[i].is_ascii_digit())
        && (8..10).all(|i| bytes[i].is_ascii_digit())
}

fn clean_item(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    text = strip_trailing_commit_link(&text);
    text = replace_issue_links(&text);
    text = unbold_scope(&text);
    text.trim().to_string()
}

fn strip_trailing_commit_link(text: &str) -> String {
    if let Some(start) = text.rfind("([")
        && commit_link_at(text, start).is_some()
    {
        return text[..start].to_string();
    }

    text.to_string()
}

fn commit_link_at(text: &str, start: usize) -> Option<usize> {
    let close_bracket = text[start + 2..].find("](")?;
    let close_bracket = start + 2 + close_bracket;
    let hash = &text[start + 2..close_bracket];

    if !is_hash(hash) {
        return None;
    }

    let url_close = text[close_bracket + 2..].find(')')?;
    let end = close_bracket + 2 + url_close + 1;

    if end < text.len() && text.as_bytes()[end] == b')' {
        Some(end + 1)
    } else {
        None
    }
}

fn is_hash(value: &str) -> bool {
    (7..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn replace_issue_links(text: &str) -> String {
    let mut out = String::new();
    let mut i = 0;

    while i < text.len() {
        if text[i..].starts_with("([#")
            && let Some((replacement, next)) = issue_link_at(text, i)
        {
            out.push_str(&replacement);
            i = next;
            continue;
        }

        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

fn issue_link_at(text: &str, start: usize) -> Option<(String, usize)> {
    let close_bracket = text[start + 3..].find("](")?;
    let close_bracket = start + 3 + close_bracket;
    let number = &text[start + 3..close_bracket];

    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let url_close = text[close_bracket + 2..].find(')')?;
    let end = close_bracket + 2 + url_close + 1;

    if end < text.len() && text.as_bytes()[end] == b')' {
        let replacement = format!("(#{number})");
        Some((replacement, end + 1))
    } else {
        None
    }
}

fn unbold_scope(text: &str) -> String {
    let Some(after) = text.strip_prefix("**") else {
        return text.to_string();
    };
    let Some(pos) = after.find(":**") else {
        return text.to_string();
    };

    let scope = &after[..pos];
    let rest = &after[pos + 3..];
    let mut out = String::with_capacity(scope.len() + 1 + rest.len());
    out.push_str(scope);
    out.push(':');
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> Release {
        Release {
            version: version.to_string(),
            date: None,
            sections: Vec::new(),
        }
    }

    #[test]
    fn parses_both_heading_forms() {
        let markdown = "\
# Changelog

## [0.2.0](https://github.com/example/project/compare/v0.1.0...v0.2.0) (2026-11-07)

## 0.1.0 (2026-11-01)
";

        let releases = parse(markdown);

        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].version, "0.2.0");
        assert_eq!(releases[0].date, Some("2026-11-07".to_string()));
        assert_eq!(releases[1].version, "0.1.0");
        assert_eq!(releases[1].date, Some("2026-11-01".to_string()));
    }

    #[test]
    fn parses_sections_and_items_in_order() {
        let markdown = "\
## [1.0.0](https://github.com/example/project/compare/v0.9.0...v1.0.0) (2026-01-01)

### Features

* a ([#1](https://github.com/example/project/issues/1)) ([abc1234](https://github.com/example/project/commit/abc1234))
* b ([def5678](https://github.com/example/project/commit/def5678))

### Bug Fixes

* c ([#2](https://github.com/example/project/issues/2)) ([abc1234](https://github.com/example/project/commit/abc1234))
";

        let releases = parse(markdown);
        let release = &releases[0];

        assert_eq!(release.sections.len(), 2);
        assert_eq!(release.sections[0].title, "Features");
        assert_eq!(
            release.sections[0].items,
            vec!["a (#1)".to_string(), "b".to_string()]
        );
        assert_eq!(release.sections[1].title, "Bug Fixes");
        assert_eq!(release.sections[1].items, vec!["c (#2)".to_string()]);
    }

    #[test]
    fn cleans_items() {
        let markdown = "\
## 1.0.0 (2026-01-01)

### Features

* show a first-run checklist ([#201](https://github.com/example/project/issues/201)) ([21470c6](https://github.com/example/project/commit/21470c6aaaa))
* **ui:** fold criteria to one line ([4bc9e01](https://github.com/example/project/commit/4bc9e01bbbb))
* keep test temp dirs in one folder ([#203](https://github.com/example/project/issues/203)) ([8a52993](https://github.com/example/project/commit/8a52993ccc))
";

        let releases = parse(markdown);
        let expected = vec![
            "show a first-run checklist (#201)".to_string(),
            "ui: fold criteria to one line".to_string(),
            "keep test temp dirs in one folder (#203)".to_string(),
        ];

        assert_eq!(releases[0].sections[0].items, expected);
    }

    #[test]
    fn ignores_text_before_first_release() {
        let markdown = "\
# Changelog

Some intro text.

## 0.1.0 (2026-11-01)
";

        let releases = parse(markdown);

        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].version, "0.1.0");
    }

    #[test]
    fn unseen_counts_only_releases_in_range() {
        let releases = vec![release("0.3.0"), release("0.2.0"), release("0.1.0")];

        assert_eq!(unseen(&releases, "0.1.0", "0.3.0"), 2);
        assert_eq!(unseen(&releases, "0.2.0", "0.3.0"), 1);
        assert_eq!(unseen(&releases, "0.3.0", "0.3.0"), 0);
        assert_eq!(unseen(&releases, "0.1.0", "0.2.0"), 1);
    }

    #[test]
    fn unparsable_seen_counts_zero() {
        let releases = vec![release("0.1.0")];

        assert_eq!(unseen(&releases, "not-a-version", "0.1.0"), 0);
        assert_eq!(unseen(&releases, "0.1.0", "not-a-version"), 0);
    }
}
