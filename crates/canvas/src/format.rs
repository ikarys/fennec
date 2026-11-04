//! Artifact formats and content policy.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Html,
    Markdown,
    Svg,
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl Format {
    /// Detect a format from a file extension.
    pub fn of_path(path: &str) -> Option<Format> {
        let (_, ext) = path.rsplit_once('.')?;
        match ext.to_ascii_lowercase().as_str() {
            "html" | "htm" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Markdown),
            "svg" => Some(Self::Svg),
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    /// Detect a format from a fenced code block language.
    pub fn of_language(lang: &str) -> Option<Format> {
        match lang.trim().to_ascii_lowercase().as_str() {
            "html" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Markdown),
            "svg" => Some(Self::Svg),
            _ => None,
        }
    }

    /// Canonical storage string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "markdown",
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Webp => "webp",
        }
    }

    /// Parse a canonical storage string.
    pub fn parse(s: &str) -> Option<Format> {
        match s {
            "html" => Some(Self::Html),
            "markdown" => Some(Self::Markdown),
            "svg" => Some(Self::Svg),
            "png" => Some(Self::Png),
            "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    /// MIME type for serving content.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Html => "text/html; charset=utf-8",
            Self::Markdown => "text/markdown; charset=utf-8",
            Self::Svg => "image/svg+xml; charset=utf-8",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    /// Whether the content is text that can be edited as a string.
    pub fn is_text(self) -> bool {
        matches!(self, Self::Html | Self::Markdown | Self::Svg)
    }
}

/// Every artifact is model output; this CSP gives it an opaque origin.
pub const SANDBOX_CSP: &str = "sandbox allow-scripts";

/// Maximum stored content size.
pub const MAX_CONTENT_BYTES: usize = 10 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_paths_case_insensitively() {
        assert_eq!(Format::of_path("a.HTML"), Some(Format::Html));
        assert_eq!(Format::of_path("a.MD"), Some(Format::Markdown));
        assert_eq!(Format::of_path("a.jpeg"), Some(Format::Jpeg));
        assert_eq!(Format::of_path("dir.v2/a.JPG"), Some(Format::Jpeg));
        assert_eq!(Format::of_path("page.htm"), Some(Format::Html));
        assert_eq!(Format::of_path("noext"), None);
        assert_eq!(Format::of_path("a.unknown"), None);
    }

    #[test]
    fn detects_languages() {
        assert_eq!(Format::of_language(" HTML "), Some(Format::Html));
        assert_eq!(Format::of_language("markdown"), Some(Format::Markdown));
        assert_eq!(Format::of_language("rust"), None);
    }

    #[test]
    fn parse_round_trips() {
        for format in [
            Format::Html,
            Format::Markdown,
            Format::Svg,
            Format::Png,
            Format::Jpeg,
            Format::Gif,
            Format::Webp,
        ] {
            assert_eq!(Format::parse(format.as_str()), Some(format));
        }
    }

    #[test]
    fn content_types_and_text() {
        assert_eq!(Format::Html.content_type(), "text/html; charset=utf-8");
        assert_eq!(Format::Jpeg.content_type(), "image/jpeg");
        assert!(Format::Svg.is_text());
        assert!(!Format::Png.is_text());
    }
}
