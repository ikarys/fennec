//! Helpers shared by the tools' tests.

use super::LocalTools;
use crate::agent::ports::Toolbox;
use crate::domain::agent::CommandAccess;
use crate::domain::event::{OutputStream, ToolResult};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn temp_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fennec-tools-test-{}-{nanos}-{counter}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn no_output() -> impl FnMut(OutputStream, &str) {
    |_, _| {}
}

pub fn run_tool(
    name: &str,
    args: &Value,
    dir: &Path,
    on_output: &mut dyn FnMut(OutputStream, &str),
) -> ToolResult {
    LocalTools::new(dir.to_path_buf()).execute(
        name,
        args,
        CommandAccess::ReadWrite,
        &AtomicBool::new(false),
        on_output,
    )
}
