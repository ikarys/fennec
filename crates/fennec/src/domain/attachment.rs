//! Port of openfox shared/types `Attachment` and tools/read `detectImageType`: the images a human attaches to a message or read_file returns, sent to the model as image parts.
//!
//! Not ported: text/PDF attachments, the vision fallback model for models without vision.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    /// A `data:` URL, the form the model API takes.
    pub data: String,
}

pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// openfox `detectImageType`: the MIME type of the bytes, from their magic
/// bytes; SVG is only trusted for a `.svg` path that starts like XML/SVG.
pub fn sniff_image(bytes: &[u8], path: &str) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"BM") {
        return Some("image/bmp");
    }
    if path.to_lowercase().ends_with(".svg") {
        let text = String::from_utf8_lossy(bytes);
        let head = text.trim_start();
        if head.starts_with("<?xml") || head.starts_with("<svg") {
            return Some("image/svg+xml");
        }
    }
    None
}

pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

pub fn from_bytes(id: String, filename: String, bytes: &[u8]) -> Result<Attachment, String> {
    let Some(mime) = sniff_image(bytes, &filename) else {
        return Err(format!(
            "{filename}: not an image (png, jpeg, gif, webp, bmp, svg)"
        ));
    };
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(format!("{filename}: image larger than 5 MB"));
    }
    Ok(Attachment {
        id,
        filename,
        mime_type: mime.to_string(),
        size: bytes.len() as u64,
        data: data_url(mime, bytes),
    })
}

pub fn from_data_url(id: String, filename: String, url: &str) -> Result<Attachment, String> {
    let not_a_data_url = || format!("{filename}: not a base64 data URL");
    let Some(rest) = url.strip_prefix("data:") else {
        return Err(not_a_data_url());
    };
    let Some((declared, payload)) = rest.split_once(";base64,") else {
        return Err(not_a_data_url());
    };
    if declared.is_empty() {
        return Err(not_a_data_url());
    }
    let bytes = STANDARD
        .decode(payload.as_bytes())
        .map_err(|_| not_a_data_url())?;
    // from_bytes sniffs the content and rebuilds `data` in canonical form.
    let attachment = from_bytes(id, filename.clone(), &bytes)?;
    if attachment.mime_type != declared {
        return Err(format!(
            "{filename}: declared {declared} but the content is {}",
            attachment.mime_type
        ));
    }
    Ok(attachment)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0]
    }

    #[test]
    fn sniff_image_recognises_the_supported_magic_bytes() {
        assert_eq!(sniff_image(&png(), "a.png"), Some("image/png"));
        assert_eq!(
            sniff_image(&[0xff, 0xd8, 0xff, 0], "a.jpg"),
            Some("image/jpeg")
        );
        assert_eq!(sniff_image(b"GIF87a", "a.gif"), Some("image/gif"));
        assert_eq!(sniff_image(b"GIF89a", "a.gif"), Some("image/gif"));
        assert_eq!(
            sniff_image(b"RIFF\x00\x00\x00\x00WEBP", "a.webp"),
            Some("image/webp")
        );
        assert_eq!(sniff_image(b"BM", "a.bmp"), Some("image/bmp"));
    }

    #[test]
    fn sniff_image_trusts_svg_only_for_a_svg_path_that_starts_like_svg() {
        let svg = b"  <?xml version=\"1.0\"?><svg/>";
        assert_eq!(sniff_image(svg, "a.svg"), Some("image/svg+xml"));
        assert_eq!(sniff_image(svg, "A.SVG"), Some("image/svg+xml"));
        assert_eq!(sniff_image(svg, "a.txt"), None);
        assert_eq!(sniff_image(b"hello", "a.svg"), None);
        assert_eq!(sniff_image(b"hello", "a.txt"), None);
    }

    #[test]
    fn data_url_round_trips_through_from_data_url() {
        let bytes = png();
        let original = from_bytes("id".into(), "a.png".into(), &bytes).unwrap();
        let parsed = from_data_url("other".into(), "a.png".into(), &original.data).unwrap();
        assert_eq!(parsed.id, "other");
        assert_eq!(parsed.mime_type, "image/png");
        assert_eq!(parsed.size, bytes.len() as u64);
        assert_eq!(parsed.data, original.data);
    }

    #[test]
    fn from_data_url_rejects_a_declared_mime_that_does_not_match_the_content() {
        let bytes = png();
        let url = format!("data:image/jpeg;base64,{}", STANDARD.encode(&bytes));
        let error = from_data_url("id".into(), "a.png".into(), &url).unwrap_err();
        assert_eq!(
            error,
            "a.png: declared image/jpeg but the content is image/png"
        );
    }

    #[test]
    fn from_bytes_rejects_an_image_above_the_limit() {
        let mut bytes = vec![0u8; MAX_IMAGE_BYTES + 1];
        bytes[..8].copy_from_slice(&png()[..8]);
        let error = from_bytes("id".into(), "a.png".into(), &bytes).unwrap_err();
        assert_eq!(error, "a.png: image larger than 5 MB");
    }

    #[test]
    fn from_data_url_rejects_bad_base64_and_non_data_urls() {
        let not_base64 =
            from_data_url("id".into(), "a.png".into(), "data:image/png;base64,!!!").unwrap_err();
        assert_eq!(not_base64, "a.png: not a base64 data URL");
        let not_data = from_data_url("id".into(), "a.png".into(), "http://a.png").unwrap_err();
        assert_eq!(not_data, "a.png: not a base64 data URL");
    }
}
