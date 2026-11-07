//! The files of a session's project, served for the preview. Model-written
//! HTML must never run with fennec's origin: the `sandbox` CSP gives the page
//! an opaque origin, whose requests to the API the guard refuses. Real PDFs
//! are served without it because browser PDF viewers refuse sandboxed documents.

use crate::web::App;
use crate::web::http::HttpError;
use crate::web::http::{error, percent_decode, send_with};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

const MAX_PREVIEW_BYTES: u64 = 10 * 1024 * 1024;

pub(crate) fn serve(
    app: &App,
    stream: &mut TcpStream,
    session_id: &str,
    segments: &[&str],
) -> Result<(), HttpError> {
    let Some(relative) = relative_path_from_segments(segments) else {
        return error(stream, 404, "not found");
    };
    let full = match session_file(app, session_id, &relative)? {
        SessionFile::Found(full) => full,
        SessionFile::Missing => return error(stream, 404, "not found"),
        SessionFile::TooLarge => return error(stream, 413, "file too large"),
    };
    let content_type = content_type_of(&full);
    let body = std::fs::read(&full).map_err(|e| e.to_string())?;
    let csp = canvas::Format::of_path(&relative).map_or(Some(canvas::SANDBOX_CSP), |format| {
        format.sandbox_csp(&body)
    });
    let header = csp.map(|csp| ("Content-Security-Policy", csp));
    send_with(stream, 200, &content_type, header.as_slice(), &body)
}

/// A file of a session's working directory, as the preview and the canvas may read it.
pub(crate) enum SessionFile {
    Found(PathBuf),
    /// No such session or file, or outside the working directory.
    Missing,
    TooLarge,
}

/// Resolves a project-relative path in the session's working directory (its
/// worktree when it has one).
pub(crate) fn session_file(
    app: &App,
    session_id: &str,
    relative: &str,
) -> Result<SessionFile, String> {
    let store = app.store()?;
    let Some((session, project)) = app.session_and_project(session_id)? else {
        return Ok(SessionFile::Missing);
    };
    // Canonical paths: a symlink out of the place is refused like `..`.
    let workdir = app
        .place(&store, &session, &project)?
        .workdir
        .canonicalize();
    let full = workdir
        .as_ref()
        .ok()
        .and_then(|workdir| workdir.join(relative).canonicalize().ok())
        .filter(|full| workdir.as_ref().is_ok_and(|w| full.starts_with(w)) && full.is_file());
    let Some(full) = full else {
        return Ok(SessionFile::Missing);
    };
    if std::fs::metadata(&full).map_err(|e| e.to_string())?.len() > MAX_PREVIEW_BYTES {
        return Ok(SessionFile::TooLarge);
    }
    Ok(SessionFile::Found(full))
}

fn relative_path_from_segments(segments: &[&str]) -> Option<String> {
    if segments.is_empty() {
        return None;
    }
    let mut parts = Vec::with_capacity(segments.len());
    for segment in segments {
        let decoded = percent_decode(segment);
        if decoded.is_empty() || decoded == "." || decoded == ".." {
            return None;
        }
        parts.push(decoded);
    }
    Some(parts.join("/"))
}

fn content_type_of(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (mime, text) = match extension.as_str() {
        "html" | "htm" => ("text/html", true),
        "md" | "markdown" => ("text/markdown", true),
        "css" => ("text/css", true),
        "js" | "mjs" => ("text/javascript", true),
        "json" => ("application/json", true),
        "svg" => ("image/svg+xml", true),
        "pdf" => ("application/pdf", false),
        "mmd" | "mermaid" => ("text/plain", true),
        "png" => ("image/png", false),
        "jpg" | "jpeg" => ("image/jpeg", false),
        "gif" => ("image/gif", false),
        "webp" => ("image/webp", false),
        "txt" => ("text/plain", true),
        _ => ("application/octet-stream", false),
    };
    if text {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_from_segments_refuses_dot_segments() {
        assert_eq!(
            relative_path_from_segments(&["a", "b"]),
            Some("a/b".to_string())
        );
        assert_eq!(relative_path_from_segments(&[]), None);
        assert_eq!(relative_path_from_segments(&[""]), None);
        assert_eq!(relative_path_from_segments(&["."]), None);
        assert_eq!(relative_path_from_segments(&[".."]), None);
        assert_eq!(relative_path_from_segments(&["a", ".."]), None);
        assert_eq!(relative_path_from_segments(&["a", "."]), None);
        assert_eq!(relative_path_from_segments(&["a", ""]), None);
    }

    #[test]
    fn content_type_of_uses_extension() {
        assert_eq!(
            content_type_of(Path::new("page.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("page.HTM")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("notes.md")),
            "text/markdown; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("notes.markdown")),
            "text/markdown; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("style.css")),
            "text/css; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("app.js")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("app.mjs")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("data.json")),
            "application/json; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("icon.svg")),
            "image/svg+xml; charset=utf-8"
        );
        assert_eq!(content_type_of(Path::new("doc.pdf")), "application/pdf");
        assert_eq!(
            content_type_of(Path::new("diagram.mmd")),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("diagram.mermaid")),
            "text/plain; charset=utf-8"
        );
        assert_eq!(content_type_of(Path::new("image.png")), "image/png");
        assert_eq!(content_type_of(Path::new("photo.jpg")), "image/jpeg");
        assert_eq!(content_type_of(Path::new("photo.jpeg")), "image/jpeg");
        assert_eq!(content_type_of(Path::new("anim.gif")), "image/gif");
        assert_eq!(content_type_of(Path::new("image.webp")), "image/webp");
        assert_eq!(
            content_type_of(Path::new("readme.txt")),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            content_type_of(Path::new("blob.bin")),
            "application/octet-stream"
        );
        assert_eq!(
            content_type_of(Path::new("noext")),
            "application/octet-stream"
        );
    }
}
