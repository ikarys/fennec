//! The host toolbox: openfox's `read_file`, `write_file`, `edit_file` and
//! `run_command`, plus `load_skill`, run directly on the host inside one working
//! directory, and `web_search`, which queries the human's SearXNG instance: the
//! model only chooses the query.
//! The bwrap world will be a second `Toolbox` that runs them sandboxed.

mod args;
mod edit_file;
mod load_skill;
mod path_guard;
mod read_file;
mod run_command;
#[cfg(test)]
mod test_support;
mod web_search;
mod write_file;

use crate::agent::ports::{CommandRun, Commands, Toolbox};
use crate::domain::event::{OutputStream, ToolResult};
use crate::domain::preferences::SearchPrefs;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

pub struct LocalTools {
    workdir: PathBuf,
    search: SearchPrefs,
}

impl LocalTools {
    pub fn new(workdir: PathBuf) -> LocalTools {
        LocalTools {
            workdir,
            search: SearchPrefs::default(),
        }
    }

    pub fn with_search(mut self, search: SearchPrefs) -> LocalTools {
        self.search = search;
        self
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
            web_search::definition(),
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
            "web_search" => web_search::run(args, &self.search),
            _ => ToolResult::failure(format!("Unknown tool: {name}")),
        };
        result.duration_ms = start.elapsed().as_millis() as u64;
        result
    }
}

impl Commands for LocalTools {
    fn run(&self, command: &str, timeout: Duration, stop: &AtomicBool) -> CommandRun {
        let mut on_output = |_: OutputStream, _: &str| {};
        match run_command::process::execute(command, &self.workdir, timeout, stop, &mut on_output) {
            Ok(execution) => command_run(execution),
            Err(error) => CommandRun {
                exit_code: None,
                stopped: false,
                output: error,
            },
        }
    }
}

fn command_run(execution: run_command::process::Execution) -> CommandRun {
    let mut output = execution.stdout;
    if !execution.stderr.is_empty() {
        output.push('\n');
        output.push_str(&execution.stderr);
    }
    match execution.ending {
        run_command::process::Ending::Exited(status) => CommandRun {
            exit_code: status.code(),
            stopped: false,
            output,
        },
        run_command::process::Ending::TimedOut => CommandRun {
            exit_code: None,
            stopped: false,
            output: format!("{output}\n[timed out]"),
        },
        run_command::process::Ending::Stopped => CommandRun {
            exit_code: None,
            stopped: true,
            output,
        },
    }
}
