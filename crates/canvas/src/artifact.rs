//! Artifact identity and version metadata.

use serde::Serialize;

use crate::format::Format;

/// Where an artifact comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Source {
    /// A file of the session's working directory, project-relative; new versions are taken from it.
    File { path: String },
    /// A block of a message, promoted by the human; it only has the versions it was given.
    Message,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: i64,
    pub project_id: String,
    pub session_id: String,
    pub title: String,
    pub format: Format,
    pub source: Source,
    /// Unix milliseconds.
    pub created_at: u64,
    pub updated_at: u64,
    /// The number of the latest version; versions are numbered from 1.
    pub latest_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub number: u32,
    pub size: u64,
    pub created_at: u64,
}

/// A new artifact, before it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewArtifact {
    pub project_id: String,
    pub session_id: String,
    pub format: Format,
    pub source: Source,
}

/// Derive a human-readable title from the source and first content.
pub fn title_for(source: &Source, format: Format, content: &[u8]) -> String {
    let text = String::from_utf8_lossy(content);
    let raw = match source {
        Source::File { path } => file_name(path),
        Source::Message => match format {
            Format::Html => html_title(&text).unwrap_or_else(|| "Untitled".to_string()),
            Format::Markdown => markdown_title(&text).unwrap_or_else(|| "Untitled".to_string()),
            _ => "Untitled".to_string(),
        },
    };
    truncate_chars(&raw, 80)
}

fn file_name(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if name.is_empty() {
        "Untitled".to_string()
    } else {
        name.to_string()
    }
}

fn html_title(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let after = start + "<title".len();
    let rest = &lower[after..];
    let first = rest.chars().next()?;
    if !matches!(first, '>' | ' ' | '\t' | '\n' | '\r') {
        return None;
    }
    let open = rest.find('>')?;
    let close = rest[open + 1..].find("</title>")?;
    let inner = &text[after + open + 1..after + open + 1 + close];
    let title = inner.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn markdown_title(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim_start();
        if line.starts_with('#') {
            let title = line.trim_start_matches('#').trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_title_uses_last_segment() {
        let source = Source::File {
            path: "docs/a/b.html".into(),
        };
        assert_eq!(title_for(&source, Format::Html, b"<html></html>"), "b.html");
    }

    #[test]
    fn html_title_is_extracted() {
        let source = Source::Message;
        let content = b"<html><head><TITLE>  Hello  </TITLE></head></html>";
        assert_eq!(title_for(&source, Format::Html, content), "Hello");
    }

    #[test]
    fn markdown_title_is_extracted() {
        let source = Source::Message;
        let content = b"intro\n# My Doc\nbody";
        assert_eq!(title_for(&source, Format::Markdown, content), "My Doc");
    }

    #[test]
    fn falls_back_to_untitled() {
        let source = Source::Message;
        assert_eq!(title_for(&source, Format::Png, b"binary"), "Untitled");
        assert_eq!(
            title_for(&source, Format::Html, b"<html></html>"),
            "Untitled"
        );
    }

    #[test]
    fn truncates_on_char_boundary() {
        let source = Source::Message;
        let content = format!("# {}", "é".repeat(100));
        let content = content.as_bytes();
        let title = title_for(&source, Format::Markdown, content);
        assert_eq!(title.chars().count(), 80);
    }
}
