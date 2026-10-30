//! Embeds the built web UI (`web/dist`, from `npm run build` in `web/`).
//! Without a build, a placeholder page says how to make one, so `cargo build`
//! never needs Node.
//!
//! Every regular file in `web/dist` is copied to `OUT_DIR/web` and listed in
//! the generated `assets.rs`, so Vite's fixed output names (fonts, lazy
//! chunks) are served, not just a few hard-coded files.

use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let out = out_dir.join("web");
    std::fs::create_dir_all(&out).unwrap();
    println!("cargo:rerun-if-changed={}", dist.display());

    // Vite has produced a usable tree only when its entry point exists;
    // otherwise the crate must fall back to the placeholder page.
    let mut embedded = Vec::new();
    if dist.join("index.html").is_file() {
        // Embed every regular file directly in dist. This Vite config has no
        // subdirectories, and sorting makes the generated table reproducible.
        embedded = std::fs::read_dir(&dist)
            .unwrap()
            .filter_map(|entry| {
                entry
                    .ok()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
            })
            .filter(|name| dist.join(name).is_file())
            .collect();
        embedded.sort_unstable();
        for name in &embedded {
            println!("cargo:rerun-if-changed={}", dist.join(name).display());
            std::fs::copy(dist.join(name), out.join(name)).unwrap();
        }
    } else {
        // No UI build: keep the crate compiling with a placeholder page.
        embedded.push(String::from("index.html"));
        let placeholder = b"<!doctype html><meta charset=utf-8><title>fennec</title><p>The web UI is not built: run <code>npm ci &amp;&amp; npm run build</code> in <code>web/</code>, then <code>cargo build</code>.</p>";
        std::fs::write(out.join("index.html"), placeholder).unwrap();
    }

    // Generate a static table for `assets.rs`; `include_bytes!` uses the
    // absolute `OUT_DIR` so the generated file need not know source paths.
    let mut assets = String::from("const ASSETS: &[(&str, &[u8])] = &[\n");
    for name in &embedded {
        assets.push_str("    (\"/");
        assets.push_str(name);
        assets.push_str("\", include_bytes!(concat!(env!(\"OUT_DIR\"), \"/web/");
        assets.push_str(name);
        assets.push_str("\"))),\n");
    }
    assets.push_str("];\n");
    std::fs::write(out_dir.join("assets.rs"), assets).unwrap();
}
