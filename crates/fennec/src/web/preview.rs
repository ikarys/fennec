//! The files of a session's project, served for the preview. Model-written
//! HTML must never run with fennec's origin: the `sandbox` CSP gives the page
//! an opaque origin, whose requests to the API the guard refuses.

use super::App;
use super::http::{error, percent_decode, send_with};
use super::routes::session_and_project;
use std::net::TcpStream;
use std::path::Path;

const MAX_PREVIEW_BYTES: u64 = 10 * 1024 * 1024;

pub(super) fn serve(
    app: &App,
    stream: &mut TcpStream,
    session_id: &str,
    segments: &[&str],
) -> Result<(), String> {
    let store = app.store()?;
    let Some((_, project)) = session_and_project(&store, session_id)? else {
        return error(stream, 404, "no such session");
    };
    let Some(relative) = relative_path_from_segments(segments) else {
        return error(stream, 404, "not found");
    };
    // Canonical paths: a symlink out of the project is refused like `..`.
    let workdir = Path::new(&project.workdir).canonicalize();
    let full = workdir
        .as_ref()
        .ok()
        .and_then(|workdir| workdir.join(&relative).canonicalize().ok())
        .filter(|full| workdir.as_ref().is_ok_and(|w| full.starts_with(w)) && full.is_file());
    let Some(full) = full else {
        return error(stream, 404, "not found");
    };
    if std::fs::metadata(&full).map_err(|e| e.to_string())?.len() > MAX_PREVIEW_BYTES {
        return error(stream, 413, "file too large");
    }
    let content_type = content_type_of(&full);
    let body = std::fs::read(&full).map_err(|e| e.to_string())?;
    send_with(
        stream,
        200,
        &content_type,
        &[("Content-Security-Policy", "sandbox allow-scripts")],
        &body,
    )
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
