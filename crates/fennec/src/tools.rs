//! The host toolbox: openfox's `read_file`, `write_file`, `edit_file` and
//! `run_command`, plus `load_skill`, run inside one working directory, and
//! `web_search`, which queries the human's SearXNG instance: the model only
//! chooses the query, and `web_fetch`, which saves a page in a cache and returns
//! a pointer to it (the model reads the parts it needs with `read_file`).
//! Commands run in the bwrap world when the human enabled it; the file tools
//! keep the in-process path guard. Writes to protected paths are refused;
//! commands see them read-only. A read-only agent's commands see the workdir
//! read-only.

mod args;
mod edit_file;
mod load_skill;
mod path_guard;
mod read_file;
mod rule_request;
mod run_command;
mod sandbox;
#[cfg(test)]
mod test_support;
mod web_fetch;
mod web_search;
mod write_file;

pub use sandbox::{Sandbox, SandboxHealth, probe};

use crate::agent::ports::{CommandRun, Commands, RuleCheck, Toolbox};
use crate::domain::agent::CommandAccess;
use crate::domain::event::{OutputStream, ToolResult};
use crate::domain::preferences::SearchPrefs;
use crate::domain::protection::Protection;
use rules::{Decision, Request, Rules};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

pub struct LocalTools {
    workdir: PathBuf,
    search: SearchPrefs,
    sandbox: Sandbox,
    protection: Protection,
    web_cache: PathBuf,
    rules: Rules,
    home: Option<String>,
    env: Vec<(String, PathBuf)>,
}

impl LocalTools {
    pub fn new(workdir: PathBuf) -> LocalTools {
        // The composition root sets the human's choice; tests run on the host.
        LocalTools {
            workdir,
            search: SearchPrefs::default(),
            sandbox: Sandbox::Off,
            // The composition root loads the project's protection; tests run
            // unprotected.
            protection: Protection::default(),
            web_cache: web_fetch::default_cache_dir(),
            rules: Rules::default(),
            // The rules crate expands `~` in path rules; the host's home is
            // the only home fennec knows.
            home: std::env::var_os("HOME").map(|h| h.to_string_lossy().into_owned()),
            // Extra environment for commands, set by the composition root.
            env: Vec::new(),
        }
    }

    pub fn with_search(mut self, search: SearchPrefs) -> LocalTools {
        self.search = search;
        self
    }

    pub fn with_sandbox(mut self, sandbox: Sandbox) -> LocalTools {
        self.sandbox = sandbox;
        self
    }

    pub fn with_protection(mut self, protection: Protection) -> LocalTools {
        self.protection = protection;
        self
    }

    pub fn with_env(mut self, env: Vec<(String, PathBuf)>) -> LocalTools {
        self.env = env;
        self
    }

    pub fn with_rules(mut self, rules: Rules) -> LocalTools {
        self.rules = rules;
        self
    }

    fn outside(&self, approved: bool) -> path_guard::Outside<'_> {
        path_guard::Outside {
            rules: &self.rules,
            home: self.home.as_deref(),
            approved,
        }
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
            web_fetch::definition(),
        ])
    }

    fn execute(
        &self,
        name: &str,
        args: &Value,
        access: CommandAccess,
        stop: &AtomicBool,
        on_output: &mut dyn FnMut(OutputStream, &str),
    ) -> ToolResult {
        self.dispatch(name, args, access, stop, on_output, false)
    }

    fn execute_approved(
        &self,
        name: &str,
        args: &Value,
        access: CommandAccess,
        stop: &AtomicBool,
        on_output: &mut dyn FnMut(OutputStream, &str),
    ) -> ToolResult {
        self.dispatch(name, args, access, stop, on_output, true)
    }

    fn check_rules(&self, name: &str, args: &Value) -> RuleCheck<'_> {
        rule_request::check(&self.rules, &self.workdir, self.home.as_deref(), name, args)
    }
}

impl LocalTools {
    /// Runs one tool call; `approved` lets a file tool reach the path the human approved once.
    fn dispatch(
        &self,
        name: &str,
        args: &Value,
        access: CommandAccess,
        stop: &AtomicBool,
        on_output: &mut dyn FnMut(OutputStream, &str),
        approved: bool,
    ) -> ToolResult {
        let start = Instant::now();
        let workdir = &self.workdir;
        let mut result = match name {
            "read_file" => read_file::run(args, workdir, &self.web_cache, &self.outside(approved)),
            "write_file" => write_file::run(args, workdir, &self.protection, &self.outside(approved)),
            "edit_file" => edit_file::run(args, workdir, &self.protection, &self.outside(approved)),
            "run_command" => match access {
                CommandAccess::ReadWrite => {
                    run_command::run(args, workdir, &self.sandbox, &self.env, stop, on_output)
                }
                CommandAccess::ReadOnly => match self.sandbox.read_only(workdir) {
                    Some(sandbox) => {
                        run_command::run(args, workdir, &sandbox, &self.env, stop, on_output)
                    },
                    None => ToolResult::failure(
                        "run_command is read-only in this mode and needs fennec's sandbox, which is turned off on the Settings page."
                            .to_string(),
                    ),
                },
            },
            "load_skill" => load_skill::run(args, workdir),
            "web_search" => web_search::run(args, &self.search),
            "web_fetch" => {
                if !self.search.web_fetch {
                    ToolResult::failure(
                        "web_fetch is turned off on the Settings page (Recherche web).".to_string(),
                    )
                } else {
                    web_fetch::run(args, &self.web_cache)
                }
            },
            _ => ToolResult::failure(format!("Unknown tool: {name}")),
        };
        result.duration_ms = start.elapsed().as_millis() as u64;
        result
    }
}

impl Commands for LocalTools {
    fn run(&self, command: &str, timeout: Duration, stop: &AtomicBool) -> CommandRun {
        // A workflow shell node cannot pause for the human: an explicit Ask
        // rule refuses it too, not only Block.
        let decision = self.rules.decide(&Request::Command(command));
        if let Decision::Block(rule) | Decision::Ask(Some(rule)) = decision {
            return CommandRun {
                exit_code: None,
                stopped: false,
                output: rule.refusal(),
            };
        }

        let mut on_output = |_: OutputStream, _: &str| {};
        let launch = run_command::process::Launch {
            command,
            workdir: &self.workdir,
            cwd: &self.workdir,
            sandbox: &self.sandbox,
            env: &self.env,
        };
        match run_command::process::execute(launch, timeout, stop, &mut on_output) {
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
