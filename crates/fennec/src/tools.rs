//! The host toolbox: openfox's `read_file`, `write_file`, `edit_file` and
//! `run_command`, plus `load_skill`, run directly on the host inside one working directory.
//! The bwrap world will be a second `Toolbox` that runs them sandboxed.

mod args;
mod edit_file;
mod load_skill;
mod path_guard;
mod read_file;
mod run_command;
#[cfg(test)]
mod test_support;
mod write_file;

use crate::agent::ports::Toolbox;
use crate::domain::event::{OutputStream, ToolResult};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub struct LocalTools {
    workdir: PathBuf,
}

impl LocalTools {
    pub fn new(workdir: PathBuf) -> LocalTools {
        LocalTools { workdir }
    }
}

impl Toolbox for LocalTools {
    fn definitions(&self) -> Value {
        Value::Array(vec![
            read_file::definition(),
            write_file::definition(),
            edit_file::definition(),
            run_command::definition(),
            load_skill::definition(),
        ])
    }

    fn execute(
        &self,
        name: &str,
        args: &Value,
        stop: &AtomicBool,
        on_output: &mut dyn FnMut(OutputStream, &str),
    ) -> ToolResult {
        let start = Instant::now();
        let workdir = &self.workdir;
        let mut result = match name {
            "read_file" => read_file::run(args, workdir),
            "write_file" => write_file::run(args, workdir),
            "edit_file" => edit_file::run(args, workdir),
            "run_command" => run_command::run(args, workdir, stop, on_output),
            "load_skill" => load_skill::run(args, workdir),
            _ => ToolResult::failure(format!("Unknown tool: {name}")),
        };
        result.duration_ms = start.elapsed().as_millis() as u64;
        result
    }
}
