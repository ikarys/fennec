//! The build the UI names, so the human knows which version they are testing.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn git(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn main() {
    let sources = ["src", "ui", "build.rs", "Cargo.toml", "Cargo.lock"];
    let mut status = vec!["status", "--porcelain", "--"];
    status.extend(sources);
    let built = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    println!(
        "cargo:rustc-env=FENNEC_COMMIT={}",
        git(&["rev-parse", "--short", "HEAD"])
    );
    println!(
        "cargo:rustc-env=FENNEC_BRANCH={}",
        git(&["branch", "--show-current"])
    );
    println!("cargo:rustc-env=FENNEC_DIRTY={}", !git(&status).is_empty());
    println!("cargo:rustc-env=FENNEC_BUILT={built}");
    // A commit or a checkout changes the version without touching a source file.
    for f in ["HEAD", "index"] {
        let path = git(&["rev-parse", "--git-path", f]);
        if !path.is_empty() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    for s in sources {
        println!("cargo:rerun-if-changed={s}");
    }
}
