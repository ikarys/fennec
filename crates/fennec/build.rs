//! Embeds the built web UI (`web/dist`, from `npm run build` in `web/`).
//! Without a build, a placeholder page says how to make one, so `cargo build`
//! never needs Node.

use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("web");
    std::fs::create_dir_all(&out).unwrap();
    println!("cargo:rerun-if-changed={}", dist.display());
    let built = ["index.html", "app.js", "app.css"]
        .iter()
        .all(|f| dist.join(f).is_file());
    for f in ["index.html", "app.js", "app.css"] {
        println!("cargo:rerun-if-changed={}", dist.join(f).display());
        let content = if built {
            std::fs::read(dist.join(f)).unwrap()
        } else if f == "index.html" {
            b"<!doctype html><meta charset=utf-8><title>fennec</title><p>The web UI is not built: run <code>npm ci &amp;&amp; npm run build</code> in <code>web/</code>, then <code>cargo build</code>.</p>".to_vec()
        } else {
            Vec::new()
        };
        std::fs::write(out.join(f), content).unwrap();
    }
}
