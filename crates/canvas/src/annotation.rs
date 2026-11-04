//! Notes anchored to artifact versions.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::artifact::{Artifact, Source};

/// What part of a version a note points at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Anchor {
    /// The whole document.
    Whole,
    /// A passage the human selected, verbatim.
    Quote { text: String },
    /// A rectangle of the rendered page, in fractions of its width and height (0..=1).
    Region {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: i64,
    pub artifact_id: i64,
    pub version: u32,
    pub anchor: Anchor,
    pub note: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationError {
    EmptyNote,
    NoteTooLong,
    QuoteTooLong,
    EmptyQuote,
    RegionOutOfBounds,
}

impl fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyNote => "note is empty",
            Self::NoteTooLong => "note is too long",
            Self::QuoteTooLong => "quote is too long",
            Self::EmptyQuote => "quote is empty",
            Self::RegionOutOfBounds => "region is out of bounds",
        };
        f.write_str(message)
    }
}

impl std::error::Error for AnnotationError {}

pub const MAX_NOTE_CHARS: usize = 2_000;
pub const MAX_QUOTE_CHARS: usize = 2_000;

/// Validate an anchor and note before storage.
pub fn validate(anchor: &Anchor, note: &str) -> Result<(), AnnotationError> {
    let note = note.trim();
    if note.is_empty() {
        return Err(AnnotationError::EmptyNote);
    }
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(AnnotationError::NoteTooLong);
    }

    if let Anchor::Quote { text } = anchor {
        let text = text.trim();
        if text.is_empty() {
            return Err(AnnotationError::EmptyQuote);
        }
        if text.chars().count() > MAX_QUOTE_CHARS {
            return Err(AnnotationError::QuoteTooLong);
        }
    }

    if let Anchor::Region {
        x,
        y,
        width,
        height,
    } = anchor
    {
        let tolerance = 1e-9;
        let in_bounds = [x, y, width, height].iter().all(|v| v.is_finite());
        if !in_bounds
            || *x < 0.0
            || *y < 0.0
            || *width <= 0.0
            || *height <= 0.0
            || *x + *width > 1.0 + tolerance
            || *y + *height > 1.0 + tolerance
        {
            return Err(AnnotationError::RegionOutOfBounds);
        }
    }

    Ok(())
}

/// Build the English message sent to the agent for an annotation.
pub fn message(artifact: &Artifact, annotation: &Annotation) -> String {
    let header = match &artifact.source {
        Source::File { path } => {
            format!("On the canvas `{path}` (version {}):", annotation.version)
        }
        Source::Message => format!(
            "On the canvas \"{}\" (version {}):",
            artifact.title, annotation.version
        ),
    };

    let mut lines = vec![header];
    match &annotation.anchor {
        Anchor::Whole => {}
        Anchor::Quote { text } => {
            for line in text.trim().lines() {
                lines.push(format!("> {line}"));
            }
        }
        Anchor::Region {
            x,
            y,
            width,
            height,
        } => {
            let x0 = (x * 100.0).round() as i64;
            let x1 = ((x + width) * 100.0).round() as i64;
            let y0 = (y * 100.0).round() as i64;
            let y1 = ((y + height) * 100.0).round() as i64;
            lines.push(format!(
                "Region: x {x0}%–{x1}%, y {y0}%–{y1}% of the rendered page."
            ));
        }
    }

    lines.push(String::new());
    lines.push(annotation.note.trim().to_string());
    // A block has no file to edit: the agent's next block of this format is the next version.
    if artifact.source == Source::Message {
        lines.push(String::new());
        lines.push(format!(
            "Answer with the whole revised document in one ```{} block: it becomes the canvas's next version.",
            artifact.format.as_str()
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Format;

    fn artifact() -> Artifact {
        Artifact {
            id: 1,
            project_id: "p".into(),
            session_id: "s".into(),
            title: "Doc".into(),
            format: Format::Html,
            source: Source::Message,
            created_at: 0,
            updated_at: 0,
            latest_version: 1,
        }
    }

    #[test]
    fn validates_note_and_anchor() {
        assert!(validate(&Anchor::Whole, "  ok  ").is_ok());
        assert_eq!(
            validate(&Anchor::Whole, "   "),
            Err(AnnotationError::EmptyNote)
        );
        assert_eq!(
            validate(&Anchor::Quote { text: "   ".into() }, "note"),
            Err(AnnotationError::EmptyQuote)
        );
        assert_eq!(
            validate(
                &Anchor::Region {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0
                },
                "note"
            ),
            Ok(())
        );
        assert_eq!(
            validate(
                &Anchor::Region {
                    x: 0.5,
                    y: 0.5,
                    width: 0.6,
                    height: 0.1
                },
                "note"
            ),
            Err(AnnotationError::RegionOutOfBounds)
        );
    }

    #[test]
    fn message_for_whole() {
        let annotation = Annotation {
            id: 1,
            artifact_id: 1,
            version: 2,
            anchor: Anchor::Whole,
            note: "  make it better ".into(),
            created_at: 0,
        };
        assert_eq!(
            message(&artifact(), &annotation),
            "On the canvas \"Doc\" (version 2):\n\nmake it better\n\nAnswer with the whole revised document in one ```html block: it becomes the canvas's next version."
        );
    }

    #[test]
    fn message_for_quote() {
        let annotation = Annotation {
            id: 1,
            artifact_id: 1,
            version: 1,
            anchor: Anchor::Quote {
                text: "line one\nline two".into(),
            },
            note: "note".into(),
            created_at: 0,
        };
        assert_eq!(
            message(&artifact(), &annotation),
            "On the canvas \"Doc\" (version 1):\n> line one\n> line two\n\nnote\n\nAnswer with the whole revised document in one ```html block: it becomes the canvas's next version."
        );
    }

    #[test]
    fn message_for_region() {
        let annotation = Annotation {
            id: 1,
            artifact_id: 1,
            version: 3,
            anchor: Anchor::Region {
                x: 0.1,
                y: 0.2,
                width: 0.3,
                height: 0.4,
            },
            note: "note".into(),
            created_at: 0,
        };
        assert_eq!(
            message(&artifact(), &annotation),
            "On the canvas \"Doc\" (version 3):\nRegion: x 10%–40%, y 20%–60% of the rendered page.\n\nnote\n\nAnswer with the whole revised document in one ```html block: it becomes the canvas's next version."
        );
    }

    #[test]
    fn file_source_uses_path() {
        let mut a = artifact();
        a.source = Source::File {
            path: "a/b.html".into(),
        };
        let annotation = Annotation {
            id: 1,
            artifact_id: 1,
            version: 1,
            anchor: Anchor::Whole,
            note: "note".into(),
            created_at: 0,
        };
        assert_eq!(
            message(&a, &annotation),
            "On the canvas `a/b.html` (version 1):\n\nnote"
        );
    }
}
