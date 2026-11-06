//! The web UI, built by Vite into `web/dist` and embedded by `build.rs`.
//!
//! `build.rs` copies every file from `web/dist` into `OUT_DIR/web` and
//! generates the `ASSETS` table, so new Vite output (fonts, lazy chunks) is
//! served without changing this file.

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

pub struct Asset {
    pub content_type: &'static str,
    pub body: &'static [u8],
}

/// The content type for an asset file name's extension.
fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.') {
        Some((_, "js")) => "text/javascript; charset=utf-8",
        Some((_, "css")) => "text/css; charset=utf-8",
        Some((_, "html")) => "text/html; charset=utf-8",
        Some((_, "woff2")) => "font/woff2",
        Some((_, "woff")) => "font/woff",
        Some((_, "svg")) => "image/svg+xml",
        Some((_, "png")) => "image/png",
        Some((_, "json")) => "application/json",
        Some((_, "webmanifest")) => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

/// The asset at `path`; any other path is a client-side route of the
/// single-page app, answered with its page.
pub fn get(path: &str) -> Asset {
    if let Some((key, body)) = ASSETS.iter().find(|(key, _)| *key == path).copied() {
        Asset {
            content_type: content_type(key),
            body,
        }
    } else {
        Asset {
            content_type: "text/html; charset=utf-8",
            body: ASSETS
                .iter()
                .find(|(key, _)| *key == "/index.html")
                .copied()
                .map(|(_, body)| body)
                .expect("build.rs always embeds index.html"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_and_client_routes_are_html() {
        assert_eq!(get("/").content_type, "text/html; charset=utf-8");
        assert_eq!(get("/s/abc").content_type, "text/html; charset=utf-8");
    }

    #[test]
    fn index_is_html() {
        let asset = get("/index.html");
        assert_eq!(asset.content_type, "text/html; charset=utf-8");
    }
}
