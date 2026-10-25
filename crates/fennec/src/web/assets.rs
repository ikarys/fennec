//! The web UI, built by Vite into `web/dist` and embedded by `build.rs`.

const INDEX_HTML: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web/index.html"));
const APP_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web/app.js"));
const WORKFLOWS_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web/app-WorkflowsPage.js"));
const APP_CSS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web/app.css"));

pub struct Asset {
    pub content_type: &'static str,
    pub body: &'static [u8],
}

/// The asset at `path`; any other path is a client-side route of the
/// single-page app, answered with its page.
pub fn get(path: &str) -> Asset {
    match path {
        "/app.js" => Asset {
            content_type: "text/javascript; charset=utf-8",
            body: APP_JS,
        },
        "/app-WorkflowsPage.js" => Asset {
            content_type: "text/javascript; charset=utf-8",
            body: WORKFLOWS_JS,
        },
        "/app.css" => Asset {
            content_type: "text/css; charset=utf-8",
            body: APP_CSS,
        },
        _ => Asset {
            content_type: "text/html; charset=utf-8",
            body: INDEX_HTML,
        },
    }
}
