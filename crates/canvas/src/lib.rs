//! Storage and model for canvas artifacts.
//!
//! An artifact is a versioned document owned by a project and session.
//! This crate is fennec-free: it only knows string ids and its own SQLite file.

mod annotation;
mod artifact;
mod format;
mod revision;
mod store;

pub use annotation::{
    Anchor, Annotation, AnnotationError, MAX_NOTE_CHARS, MAX_QUOTE_CHARS, message, validate,
};
pub use artifact::{Artifact, NewArtifact, Source, Version, title_for};
pub use format::{Format, MAX_CONTENT_BYTES, SANDBOX_CSP};
pub use revision::{blocks, revisions};
pub use store::{CanvasStore, StoreError};
