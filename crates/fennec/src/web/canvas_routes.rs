//! Canvas routes (contract `docs/api.md`).
//!
//! The `canvas` crate owns the artifacts; this module only translates HTTP to
//! its calls and reads the pinned files from the session's working directory.

use std::net::TcpStream;

use canvas::{Anchor, Artifact, CanvasStore, Format, NewArtifact, Source, StoreError};
use serde_json::{Value, json};

use super::App;
use super::http::{self, Request};
use super::preview::{SessionFile, session_file};
use crate::agent::ports::EventLog;
use crate::domain::conversation::assistant_texts;

const CREATE_BODY_ERROR: &str =
    "body must be {\"path\": ...} or {\"language\": ..., \"content\": ...}";

pub(super) fn canvas_store(app: &App) -> Result<CanvasStore, String> {
    CanvasStore::open(&app.settings.canvas_path()).map_err(|e| e.to_string())
}

pub(super) fn list_session(
    app: &App,
    stream: &mut TcpStream,
    session_id: &str,
) -> Result<(), String> {
    if app.store()?.session(session_id)?.is_none() {
        return http::error(stream, 404, "session not found");
    }

    let artifacts = canvas_store(app)?
        .list_session(session_id)
        .map_err(|e| e.to_string())?;
    http::send_json(stream, 200, &json!({ "artifacts": artifacts }))
}

pub(super) fn list_project(
    app: &App,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return http::error(stream, 404, "project not found");
    }

    let artifacts = canvas_store(app)?
        .list_project(project_id)
        .map_err(|e| e.to_string())?;
    http::send_json(stream, 200, &json!({ "artifacts": artifacts }))
}

pub(super) fn create(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    session_id: &str,
) -> Result<(), String> {
    let Some(session) = app.store()?.session(session_id)? else {
        return http::error(stream, 404, "session not found");
    };
    let body = match body_create(req) {
        Ok(body) => body,
        Err(e) => return http::error(stream, 400, &e),
    };
    let store = canvas_store(app)?;

    match body {
        CreateBody::File { path } => {
            create_file(app, &store, stream, &session.project_id, &session.id, &path)
        }
        CreateBody::Block { format, content } => create_block(
            &store,
            stream,
            &session.project_id,
            &session.id,
            format,
            &content,
        ),
    }
}

// ponytail: versions are taken when the canvas is read, so two writes between reads make one version; hook the file tools if the steps matter.
pub(super) fn sync(app: &App, store: &CanvasStore, artifact: &Artifact) -> Result<(), String> {
    let contents = match &artifact.source {
        Source::File { path } => {
            let SessionFile::Found(file) = session_file(app, &artifact.session_id, path)? else {
                return Ok(());
            };
            vec![std::fs::read(&file).map_err(|e| e.to_string())?]
        }
        Source::Message => block_revisions(app, store, artifact)?,
    };
    for content in contents {
        match store.record_version(artifact.id, &content) {
            Ok(_) | Err(StoreError::Empty | StoreError::TooLarge) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

// ponytail: any later block of the same format becomes a version, even one about something
// else; tie the blocks to the annotation they answer if that misfires.
/// The blocks of the artifact's format the agent wrote after the one its latest version holds.
fn block_revisions(
    app: &App,
    store: &CanvasStore,
    artifact: &Artifact,
) -> Result<Vec<Vec<u8>>, String> {
    let Some(latest) = store
        .content(artifact.id, artifact.latest_version)
        .map_err(|e| e.to_string())?
    else {
        return Ok(Vec::new());
    };
    let events = app.store()?.events(&artifact.session_id)?;
    let texts = assistant_texts(&events);
    Ok(canvas::revisions(&texts, artifact.format, &latest)
        .into_iter()
        .map(String::into_bytes)
        .collect())
}

pub(super) fn detail(app: &App, stream: &mut TcpStream, artifact_id: &str) -> Result<(), String> {
    let Ok(id) = parse_id(artifact_id) else {
        return http::error(stream, 404, "artifact not found");
    };
    let store = canvas_store(app)?;
    let Some(artifact) = store.artifact(id).map_err(|e| e.to_string())? else {
        return http::error(stream, 404, "artifact not found");
    };

    sync(app, &store, &artifact)?;
    let Some(artifact) = store.artifact(id).map_err(|e| e.to_string())? else {
        return http::error(stream, 404, "artifact not found");
    };

    let versions = store.versions(id).map_err(|e| e.to_string())?;
    let annotations = store.annotations(id).map_err(|e| e.to_string())?;
    http::send_json(
        stream,
        200,
        &json!({
            "artifact": artifact,
            "versions": versions,
            "annotations": annotations
        }),
    )
}

pub(super) fn content(
    app: &App,
    stream: &mut TcpStream,
    artifact_id: &str,
    number: &str,
) -> Result<(), String> {
    let Ok(id) = parse_id(artifact_id) else {
        return http::error(stream, 404, "artifact not found");
    };
    let Ok(version) = number.parse::<u32>() else {
        return http::error(stream, 404, "version not found");
    };
    let store = canvas_store(app)?;
    let Some(artifact) = store.artifact(id).map_err(|e| e.to_string())? else {
        return http::error(stream, 404, "artifact not found");
    };
    let Some(bytes) = store.content(id, version).map_err(|e| e.to_string())? else {
        return http::error(stream, 404, "version not found");
    };

    http::send_with(
        stream,
        200,
        artifact.format.content_type(),
        &[("Content-Security-Policy", canvas::SANDBOX_CSP)],
        &bytes,
    )
}

/// Deletes the artifact from the library only, never the file in the working directory.
pub(super) fn delete(app: &App, stream: &mut TcpStream, artifact_id: &str) -> Result<(), String> {
    let Ok(id) = parse_id(artifact_id) else {
        return http::error(stream, 404, "artifact not found");
    };
    let deleted = canvas_store(app)?.delete(id).map_err(|e| e.to_string())?;

    if deleted {
        http::send_json(stream, 200, &json!({ "ok": true }))
    } else {
        http::error(stream, 404, "artifact not found")
    }
}

pub(super) fn annotate(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    artifact_id: &str,
) -> Result<(), String> {
    let Ok(id) = parse_id(artifact_id) else {
        return http::error(stream, 404, "artifact not found");
    };
    let (version, anchor, note) = match annotate_body(req) {
        Ok(parts) => parts,
        Err(e) => return http::error(stream, 400, &e),
    };

    let store = canvas_store(app)?;
    let Some(artifact) = store.artifact(id).map_err(|e| e.to_string())? else {
        return http::error(stream, 404, "artifact not found");
    };
    let annotation = match store.annotate(id, version, anchor, &note) {
        Ok(value) => value,
        Err(e) => return store_error(stream, e),
    };

    http::send_json(
        stream,
        201,
        &json!({
            "annotation": annotation,
            "message": canvas::message(&artifact, &annotation),
        }),
    )
}

#[derive(Debug)]
enum CreateBody {
    File { path: String },
    Block { format: Format, content: String },
}

fn create_file(
    app: &App,
    store: &CanvasStore,
    stream: &mut TcpStream,
    project_id: &str,
    session_id: &str,
    path: &str,
) -> Result<(), String> {
    let Some(format) = Format::of_path(path) else {
        return http::error(stream, 400, "this file cannot be shown on a canvas");
    };

    if let Some(artifact) = store
        .find_file(session_id, path)
        .map_err(|e| e.to_string())?
    {
        sync(app, store, &artifact)?;
        let Some(artifact) = store.artifact(artifact.id).map_err(|e| e.to_string())? else {
            return http::error(stream, 404, "artifact not found");
        };
        return http::send_json(stream, 200, &json!({ "artifact": artifact }));
    }

    match session_file(app, session_id, path)? {
        SessionFile::Found(file) => {
            let content = std::fs::read(&file).map_err(|e| e.to_string())?;
            let artifact = match store.create(
                NewArtifact {
                    project_id: project_id.to_owned(),
                    session_id: session_id.to_owned(),
                    format,
                    source: Source::File {
                        path: path.to_owned(),
                    },
                    title: None,
                },
                &content,
            ) {
                Ok(value) => value,
                Err(e) => return store_error(stream, e),
            };
            http::send_json(stream, 201, &json!({ "artifact": artifact }))
        }
        SessionFile::Missing => http::error(stream, 404, "file not found"),
        SessionFile::TooLarge => http::error(stream, 413, "file too large"),
    }
}

fn create_block(
    store: &CanvasStore,
    stream: &mut TcpStream,
    project_id: &str,
    session_id: &str,
    format: Format,
    content: &str,
) -> Result<(), String> {
    let artifact = match store.create(
        NewArtifact {
            project_id: project_id.to_owned(),
            session_id: session_id.to_owned(),
            format,
            source: Source::Message,
            title: None,
        },
        content.as_bytes(),
    ) {
        Ok(value) => value,
        Err(e) => return store_error(stream, e),
    };

    http::send_json(stream, 201, &json!({ "artifact": artifact }))
}

fn body_create(req: &Request) -> Result<CreateBody, String> {
    create_body_from_value(&req.json()?)
}

fn annotate_body(req: &Request) -> Result<(u32, Anchor, String), String> {
    let value = req.json()?;
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or("version is required")?;
    let anchor = value.get("anchor").ok_or("anchor is required")?;
    let anchor: Anchor = serde_json::from_value(anchor.clone()).map_err(|e| e.to_string())?;
    let note = value
        .get("note")
        .and_then(Value::as_str)
        .ok_or("note is required")?;
    Ok((version, anchor, note.to_owned()))
}

fn create_body_from_value(value: &Value) -> Result<CreateBody, String> {
    if let Some(path) = value.get("path").and_then(Value::as_str) {
        return Ok(CreateBody::File {
            path: path.to_owned(),
        });
    }

    let Some(language) = value.get("language").and_then(Value::as_str) else {
        return Err(CREATE_BODY_ERROR.to_owned());
    };
    let Some(content) = value.get("content").and_then(Value::as_str) else {
        return Err(CREATE_BODY_ERROR.to_owned());
    };

    let format =
        Format::of_language(language).ok_or_else(|| "unsupported canvas language".to_owned())?;
    Ok(CreateBody::Block {
        format,
        content: content.to_owned(),
    })
}

fn parse_id(raw: &str) -> Result<i64, String> {
    raw.parse().map_err(|_| "artifact not found".to_owned())
}

fn store_error(stream: &mut TcpStream, e: StoreError) -> Result<(), String> {
    match e {
        StoreError::Empty => http::error(stream, 400, "empty content"),
        StoreError::TooLarge => http::error(stream, 413, "content too large"),
        StoreError::NotFound => http::error(stream, 404, "artifact or version not found"),
        StoreError::Annotation(e) => http::error(stream, 400, &e.to_string()),
        _ => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_file_body() {
        let body = create_body_from_value(&json!({ "path": "index.html" })).unwrap();
        let CreateBody::File { path } = body else {
            panic!("expected file body");
        };
        assert_eq!(path, "index.html");
    }

    #[test]
    fn parses_block_body() {
        let body =
            create_body_from_value(&json!({ "language": "markdown", "content": "hi" })).unwrap();
        let CreateBody::Block { format, content } = body else {
            panic!("expected block body");
        };
        assert_eq!(format.as_str(), "markdown");
        assert_eq!(content, "hi");
    }

    #[test]
    fn rejects_unknown_body() {
        assert!(create_body_from_value(&json!({ "x": 1 })).is_err());
    }

    #[test]
    fn rejects_unknown_language() {
        assert!(create_body_from_value(&json!({ "language": "pdf", "content": "x" })).is_err());
    }

    #[test]
    fn parses_id() {
        assert_eq!(parse_id("7").unwrap(), 7);
        assert!(parse_id("nope").is_err());
    }
}
