//! Artifact formats and content policy.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Html,
    Markdown,
    Svg,
    Pdf,
    Mermaid,
    Json,
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
            "pdf" => Some(Self::Pdf),
            "mmd" | "mermaid" => Some(Self::Mermaid),
            "json" => Some(Self::Json),
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
            "mermaid" => Some(Self::Mermaid),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    /// Canonical storage string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "markdown",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Mermaid => "mermaid",
            Self::Json => "json",
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
            "pdf" => Some(Self::Pdf),
            "mermaid" => Some(Self::Mermaid),
            "json" => Some(Self::Json),
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
            Self::Pdf => "application/pdf",
            Self::Mermaid => "text/plain; charset=utf-8",
            Self::Json => "application/json; charset=utf-8",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    /// Whether the content is text that can be edited as a string.
    pub fn is_text(self) -> bool {
        matches!(
            self,
            Self::Html | Self::Markdown | Self::Svg | Self::Mermaid | Self::Json
        )
    }

    /// The Content-Security-Policy to serve these bytes with; None only for a real PDF.
    ///
    /// Browser PDF viewers refuse sandboxed documents; they run in an isolated origin,
    /// `X-Content-Type-Options: nosniff` keeps it from being read as HTML, and the magic
    /// check keeps a mislabelled file sandboxed.
    pub fn sandbox_csp(self, bytes: &[u8]) -> Option<&'static str> {
        if self == Self::Pdf && bytes.starts_with(b"%PDF-") {
            return None;
        }
        Some(SANDBOX_CSP)
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
        assert_eq!(Format::of_path("a.pdf"), Some(Format::Pdf));
        assert_eq!(Format::of_path("a.mmd"), Some(Format::Mermaid));
        assert_eq!(Format::of_path("a.mermaid"), Some(Format::Mermaid));
        assert_eq!(Format::of_path("a.json"), Some(Format::Json));
        assert_eq!(Format::of_path("noext"), None);
        assert_eq!(Format::of_path("a.unknown"), None);
    }

    #[test]
    fn detects_languages() {
        assert_eq!(Format::of_language(" HTML "), Some(Format::Html));
        assert_eq!(Format::of_language("markdown"), Some(Format::Markdown));
        assert_eq!(Format::of_language("mermaid"), Some(Format::Mermaid));
        assert_eq!(Format::of_language("json"), Some(Format::Json));
        assert_eq!(Format::of_language("pdf"), None);
        assert_eq!(Format::of_language("rust"), None);
    }

    #[test]
    fn parse_round_trips() {
        for format in [
            Format::Html,
            Format::Markdown,
            Format::Svg,
            Format::Pdf,
            Format::Mermaid,
            Format::Json,
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
        assert_eq!(Format::Pdf.content_type(), "application/pdf");
        assert_eq!(Format::Mermaid.content_type(), "text/plain; charset=utf-8");
        assert_eq!(
            Format::Json.content_type(),
            "application/json; charset=utf-8"
        );
        assert_eq!(Format::Jpeg.content_type(), "image/jpeg");
        assert!(Format::Svg.is_text());
        assert!(Format::Mermaid.is_text());
        assert!(Format::Json.is_text());
        assert!(!Format::Pdf.is_text());
        assert!(!Format::Png.is_text());
    }

    #[test]
    fn sandbox_csp_is_none_only_for_real_pdf() {
        assert_eq!(Format::Pdf.sandbox_csp(b"%PDF-1.7"), None);
        assert_eq!(Format::Pdf.sandbox_csp(b"not a pdf"), Some(SANDBOX_CSP));
        assert_eq!(Format::Html.sandbox_csp(b"<html>"), Some(SANDBOX_CSP));
    }
}
