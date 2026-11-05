//! `canvas` tool: the agent shows the human a document in fennec's canvas and
//! iterates on it; the same title in a session is the same canvas, each call a
//! version. It writes fennec's own canvas store, never the project, so a
//! read-only agent may use it.

use super::args;
use crate::domain::event::ToolResult;
use canvas::{CanvasStore, Format, NewArtifact, Source};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Where the session's canvases are stored.
#[derive(Debug, Clone)]
pub struct CanvasTarget {
    pub db: PathBuf,
    pub project_id: String,
    pub session_id: String,
}

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "canvas",
            "description": concat!(
                "Show the human a document (HTML page or mockup, Markdown document, SVG, Mermaid diagram, JSON data) in fennec's canvas pane, ",
                "where they read it, browse its versions and annotate it. Call it again with the same title and the ",
                "whole revised document to make a new version; their notes come back to you as messages. Prefer it to a ",
                "code block when the human wants to see or iterate on a document."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "The canvas's name; reuse the exact same title to revise it."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["html", "markdown", "svg", "mermaid", "json"],
                        "description": "The document format."
                    },
                    "content": {
                        "type": "string",
                        "description": "The whole document, self-contained: CSS and scripts inline, no CDN. Mermaid: the diagram source only, no fences."
                    }
                },
                "required": ["title", "format", "content"]
            }
        }
    })
}

pub(super) fn run(args: &Value, target: Option<&CanvasTarget>) -> ToolResult {
    try_run(args, target).unwrap_or_else(ToolResult::failure)
}

fn try_run(args: &Value, target: Option<&CanvasTarget>) -> Result<ToolResult, String> {
    let target = target.ok_or_else(|| "The canvas needs a session: none here.".to_string())?;
    let title = args::string(args, "title")?.trim();
    if title.is_empty() {
        return Err("canvas requires a non-empty 'title'.".to_string());
    }
    if title.chars().count() > 200 {
        return Err("canvas title is too long (max 200 chars).".to_string());
    }

    let raw_format = args::string(args, "format")?;
    let format = Format::parse(raw_format)
        .filter(|format| {
            matches!(
                format,
                Format::Html | Format::Markdown | Format::Svg | Format::Mermaid | Format::Json
            )
        })
        .ok_or_else(|| "format must be html, markdown, svg, mermaid or json.".to_string())?;

    let content = args::string(args, "content")?;
    if content.trim().is_empty() {
        return Err("canvas requires a non-empty 'content'.".to_string());
    }
    if format == Format::Json {
        serde_json::from_str::<Value>(content)
            .map_err(|e| format!("canvas content is not valid JSON: {e}."))?;
    }

    let store = CanvasStore::open(&target.db).map_err(|e| e.to_string())?;
    let output = show(&store, target, title, format, content)?;
    Ok(ToolResult::success(output, false))
}

fn show(
    store: &CanvasStore,
    target: &CanvasTarget,
    title: &str,
    format: Format,
    content: &str,
) -> Result<String, String> {
    let bytes = content.as_bytes();
    match store
        .find_titled(&target.session_id, title)
        .map_err(|e| e.to_string())?
    {
        Some(artifact) if artifact.format == format => {
            match store
                .record_version(artifact.id, bytes)
                .map_err(|e| e.to_string())?
            {
                Some(version) => Ok(format!(
                    "Canvas \"{title}\" updated: version {version}, shown to the human."
                )),
                None => Ok(format!(
                    "Canvas \"{title}\" unchanged: the content is the same as version {}.",
                    artifact.latest_version
                )),
            }
        }
        _ => {
            let new = NewArtifact {
                project_id: target.project_id.clone(),
                session_id: target.session_id.clone(),
                format,
                source: Source::Message,
                title: Some(title.to_string()),
            };
            store.create(new, bytes).map_err(|e| e.to_string())?;
            Ok(format!(
                "Canvas \"{title}\" created (version 1), shown to the human."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(result: &ToolResult) -> String {
        result
            .output
            .clone()
            .or(result.error.clone())
            .unwrap_or_default()
    }

    fn target() -> CanvasTarget {
        CanvasTarget {
            db: std::env::temp_dir().join(format!(
                "fennec-canvas-tool-{}.db",
                crate::domain::id::new_id()
            )),
            project_id: "project".to_string(),
            session_id: "session".to_string(),
        }
    }

    #[test]
    fn creates_then_versions_same_title() {
        let target = target();
        let args = json!({
            "title": "Page",
            "format": "html",
            "content": "<html><body>one</body></html>"
        });
        let result = run(&args, Some(&target));
        assert!(text(&result).contains("created (version 1)"));

        let args = json!({
            "title": "Page",
            "format": "html",
            "content": "<html><body>two</body></html>"
        });
        let result = run(&args, Some(&target));
        assert!(text(&result).contains("updated: version 2"));

        let result = run(&args, Some(&target));
        assert!(text(&result).contains("unchanged: the content is the same as version 2"));
    }

    #[test]
    fn refuses_unknown_format() {
        let target = target();
        let args = json!({
            "title": "Page",
            "format": "png",
            "content": "x"
        });
        let result = run(&args, Some(&target));
        assert!(text(&result).contains("format must be html, markdown, svg, mermaid or json."));
    }

    #[test]
    fn creates_mermaid_canvas() {
        let target = target();
        let args = json!({
            "title": "Diagram",
            "format": "mermaid",
            "content": "graph TD; A --> B"
        });
        let result = run(&args, Some(&target));
        assert!(text(&result).contains("created (version 1)"));
    }

    #[test]
    fn refuses_invalid_json() {
        let target = target();
        let args = json!({
            "title": "Data",
            "format": "json",
            "content": "{ not json"
        });
        let result = run(&args, Some(&target));
        assert!(text(&result).contains("canvas content is not valid JSON:"));
    }

    #[test]
    fn refuses_without_target() {
        let args = json!({
            "title": "Page",
            "format": "html",
            "content": "x"
        });
        let result = run(&args, None);
        assert!(text(&result).contains("The canvas needs a session: none here."));
    }

    #[test]
    fn different_title_makes_second_canvas() {
        let target = target();
        let first = json!({
            "title": "One",
            "format": "markdown",
            "content": "# one"
        });
        let second = json!({
            "title": "Two",
            "format": "markdown",
            "content": "# two"
        });
        assert!(
            run(&first, Some(&target))
                .output
                .unwrap_or_default()
                .contains("created")
        );
        assert!(
            run(&second, Some(&target))
                .output
                .unwrap_or_default()
                .contains("created")
        );

        let store = CanvasStore::open(&target.db).unwrap();
        assert!(
            store
                .find_titled(&target.session_id, "One")
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .find_titled(&target.session_id, "Two")
                .unwrap()
                .is_some()
        );
    }
}
