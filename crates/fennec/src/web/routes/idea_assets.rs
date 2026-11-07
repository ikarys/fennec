//! What an idea's pinned files become when it is framed or built: images as
//! model attachments, text files inlined into the prompt, binary files written
//! into the session's workdir so the agent can read them.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use backlog::IdeaAttachment;

use crate::domain::attachment::{self, Attachment};
use crate::domain::id::new_id;

/// The workdir subfolder binary idea assets are materialized in.
const ASSETS_DIR: &str = ".fennec/assets";

/// Split an idea's attachments into (images for the model, a prompt addendum).
/// Text files are inlined; binary files are written under `workdir` and
/// referenced by their workdir-relative path.
pub fn prepare(attachments: &[IdeaAttachment], workdir: &Path) -> (Vec<Attachment>, String) {
    let mut images = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    for (i, attachment) in attachments.iter().enumerate() {
        if attachment.mime_type.starts_with("image/") {
            match attachment::from_data_url(new_id(), attachment.filename.clone(), &attachment.data)
            {
                Ok(image) => images.push(image),
                Err(_) => notes.push(format!(
                    "Attached image `{}` could not be read and was left out.",
                    attachment.filename
                )),
            }
            continue;
        }

        let Some(bytes) = decode(&attachment.data) else {
            continue;
        };
        if is_text(&attachment.mime_type) {
            let content = String::from_utf8_lossy(&bytes).to_string();
            // A file with its own code fences would break a triple-backtick
            // wrapper, so widen the fence to four.
            let fence = if content.contains("```") {
                "````"
            } else {
                "```"
            };
            notes.push(format!(
                "Attached file `{}`:\n\n{fence}\n{content}\n{fence}",
                attachment.filename,
            ));
        } else {
            match write_asset(workdir, i, &attachment.filename, &bytes) {
                Ok(path) => notes.push(format!(
                    "Attached file `{}` is saved at `{}` in the working directory.",
                    attachment.filename, path
                )),
                Err(_) => notes.push(format!(
                    "Attached file `{}` could not be saved to the working directory.",
                    attachment.filename
                )),
            }
        }
    }

    let addendum = if notes.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", notes.join("\n\n"))
    };

    (images, addendum)
}

fn is_text(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/yaml"
                | "application/x-yaml"
                | "application/csv"
        )
}

fn decode(data_url: &str) -> Option<Vec<u8>> {
    let rest = data_url.strip_prefix("data:")?;
    let (_, payload) = rest.split_once(";base64,")?;
    STANDARD.decode(payload).ok()
}

/// Writes the bytes under `workdir/.fennec/assets/` and returns the
/// workdir-relative path the agent reads with read_file.
fn write_asset(
    workdir: &Path,
    index: usize,
    filename: &str,
    bytes: &[u8],
) -> Result<String, std::io::Error> {
    // A filename like `../x` must not escape the assets folder; the index keeps
    // two attachments with the same basename from overwriting each other.
    let base = Path::new(filename)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "asset".into());
    let name = format!("{index}-{base}");
    let dir = workdir.join(ASSETS_DIR);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(&name), bytes)?;
    Ok(format!("{ASSETS_DIR}/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_url(mime: &str, b64: &str) -> String {
        format!("data:{mime};base64,{b64}")
    }

    fn png_data() -> String {
        STANDARD.encode([0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
    }

    #[test]
    fn prepare_splits_images_text_and_binary() {
        let workdir =
            std::env::temp_dir().join(format!("fennec-idea-assets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(workdir.join(ASSETS_DIR));

        let attachments = vec![
            IdeaAttachment {
                filename: "shot.png".into(),
                mime_type: "image/png".into(),
                size: 8,
                data: data_url("image/png", &png_data()),
            },
            IdeaAttachment {
                filename: "spec.md".into(),
                mime_type: "text/markdown".into(),
                size: 10,
                data: data_url("text/markdown", &STANDARD.encode(b"hello spec")),
            },
            IdeaAttachment {
                filename: "blob.bin".into(),
                mime_type: "application/octet-stream".into(),
                size: 4,
                data: data_url("application/octet-stream", &STANDARD.encode([0u8, 1, 2, 3])),
            },
        ];

        let (images, addendum) = prepare(&attachments, &workdir);

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].filename, "shot.png");
        assert_eq!(images[0].mime_type, "image/png");

        assert!(addendum.contains("spec.md"));
        assert!(addendum.contains("hello spec"));
        assert!(addendum.contains("blob.bin"));
        assert!(addendum.contains(&format!("{ASSETS_DIR}/2-blob.bin")));

        let written = std::fs::read(workdir.join(ASSETS_DIR).join("2-blob.bin")).unwrap();
        assert_eq!(written, [0u8, 1u8, 2u8, 3u8]);

        let _ = std::fs::remove_dir_all(workdir.join(ASSETS_DIR));
    }

    #[test]
    fn prepare_with_no_attachments_is_empty() {
        let (images, addendum) = prepare(&[], Path::new("/tmp"));
        assert!(images.is_empty());
        assert!(addendum.is_empty());
    }

    #[test]
    fn prepare_notes_a_unreadable_image() {
        let attachment = IdeaAttachment {
            filename: "broken.png".into(),
            mime_type: "image/png".into(),
            size: 4,
            data: data_url("image/png", "not-base64!!"),
        };
        let (images, addendum) = prepare(&[attachment], Path::new("/tmp"));
        assert!(images.is_empty());
        assert!(addendum.contains("broken.png"));
    }

    #[test]
    fn prepare_notes_a_binary_it_cannot_save() {
        // Block the assets folder with a regular file so create_dir_all fails.
        let workdir = std::env::temp_dir().join(format!(
            "fennec-idea-assets-unsavable-{}",
            std::process::id()
        ));
        let assets = workdir.join(ASSETS_DIR);
        let _ = std::fs::remove_dir_all(&workdir);
        std::fs::create_dir_all(assets.parent().unwrap()).unwrap();
        std::fs::write(&assets, b"block").unwrap();

        let attachment = IdeaAttachment {
            filename: "blob.bin".into(),
            mime_type: "application/octet-stream".into(),
            size: 4,
            data: data_url("application/octet-stream", &STANDARD.encode([0u8, 1, 2, 3])),
        };

        let (images, addendum) = prepare(std::slice::from_ref(&attachment), &workdir);

        assert!(images.is_empty());
        assert!(addendum.contains("blob.bin"));
        assert!(addendum.contains("could not be saved"));

        let _ = std::fs::remove_file(&assets);
        let _ = std::fs::remove_dir_all(&workdir);
    }

    #[test]
    fn prepare_widens_the_fence_for_text_with_code() {
        let content = "intro\n```\ncode\n```\noutro";
        let attachment = IdeaAttachment {
            filename: "spec.md".into(),
            mime_type: "text/markdown".into(),
            size: content.len() as u64,
            data: data_url("text/markdown", &STANDARD.encode(content.as_bytes())),
        };
        let (images, addendum) = prepare(&[attachment], Path::new("/tmp"));
        assert!(images.is_empty());
        assert!(addendum.contains("````"));
        assert!(addendum.contains("code"));
    }

    #[test]
    fn prepare_keeps_two_assets_with_the_same_name() {
        let workdir =
            std::env::temp_dir().join(format!("fennec-idea-assets-dup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workdir);
        let attachments = [0u8, 1].map(|v| IdeaAttachment {
            filename: "config.json".into(),
            mime_type: "application/octet-stream".into(),
            size: 1,
            data: data_url("application/octet-stream", &STANDARD.encode([v])),
        });
        let (images, addendum) = prepare(&attachments, &workdir);
        assert!(images.is_empty());
        assert!(addendum.contains("0-config.json"));
        assert!(addendum.contains("1-config.json"));
        assert_eq!(
            std::fs::read(workdir.join(ASSETS_DIR).join("0-config.json")).unwrap(),
            [0u8]
        );
        assert_eq!(
            std::fs::read(workdir.join(ASSETS_DIR).join("1-config.json")).unwrap(),
            [1u8]
        );
        let _ = std::fs::remove_dir_all(&workdir);
    }
}
