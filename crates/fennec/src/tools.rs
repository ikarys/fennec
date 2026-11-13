//! The host toolbox: `read_file`, `write_file`, `edit_file` and
//! `run_command`, plus `load_skill`, run inside one working directory, and
//! `web_search`, which queries the human's SearXNG instance: the model only
//! chooses the query, and `web_fetch`, which saves a page in a cache and returns
//! a pointer to it (the model reads the parts it needs with `read_file`).
//! Commands run in the bwrap world when the human enabled it; the file tools
//! keep the in-process path guard. Writes to protected paths are refused;
//! commands see them read-only. A read-only agent's commands see the workdir
//! read-only. `canvas` shows the human a document in fennec's canvas store,
//! never the project, so read-only agents have it too. `mcp_tools` and
//! `mcp_call` reach the human's MCP servers, declared as soon as one is
//! configured so a toggle never changes the frozen tool list.

mod args;
mod canvas;
mod edit_file;
mod load_skill;
mod mcp_call;
mod path_guard;
mod read_file;
mod rule_request;
mod run_command;

#[cfg(test)]
pub(crate) mod test_support;
mod web_fetch;
mod web_search;
mod write_file;

pub use canvas::CanvasTarget;

use crate::agent::ports::{CommandRun, Commands, RuleCheck, Toolbox};
use crate::domain::agent::CommandAccess;
use crate::domain::event::{OutputStream, ToolResult};
use crate::domain::mcp::{self, McpServer};
use crate::domain::preferences::SearchPrefs;
use crate::domain::protection::Protection;
use crate::sandbox::Sandbox;
use rules::{Decision, Request, Rules};
use serde_json::Value;
use std::path::{Path, PathBuf};
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
    // The human's project folder when the workdir is a worktree of it;
    // its paths are redirected to the workdir.
    project_dir: Option<PathBuf>,
    // The session's active MCP servers; their tools run on demand in the
    // sandbox world.
    mcp: Vec<McpServer>,
    // Whether the human configured any MCP server, active or not: then
    // `mcp_tools` and `mcp_call` are declared.
    mcp_offered: bool,
    // The session's canvas store; no session, no canvas tool.
    canvas: Option<canvas::CanvasTarget>,
    // Sealed MCP OAuth credentials for HTTP servers; applied before connect.
    mcp_vault: Option<crate::mcp::oauth::Vault>,
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
            project_dir: None,
            mcp: Vec::new(),
            mcp_offered: false,
            canvas: None,
            mcp_vault: None,
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

    /// Track a human checkout only when it differs from the session workdir.
    pub fn with_project_dir(mut self, dir: PathBuf) -> LocalTools {
        self.project_dir = (dir != self.workdir).then_some(dir);
        self
    }

    /// The session's active MCP servers; their cached tools are offered to the
    /// model and run on demand.
    pub fn with_mcp(mut self, servers: Vec<McpServer>, offered: bool) -> LocalTools {
        self.mcp = servers;
        self.mcp_offered = offered;
        self
    }

    pub fn with_canvas(mut self, target: canvas::CanvasTarget) -> LocalTools {
        self.canvas = Some(target);
        self
    }

    pub fn with_mcp_vault(mut self, vault: crate::mcp::oauth::Vault) -> LocalTools {
        self.mcp_vault = Some(vault);
        self
    }

    fn redirected(&self, name: &str, args: &Value) -> Option<(Value, String, String)> {
        if !matches!(name, "read_file" | "write_file" | "edit_file") {
            return None;
        }

        let project_dir = self.project_dir.as_ref()?;
        let original = args.get("path")?.as_str()?;
        let path = Path::new(original);
        if !path.is_absolute() || path.starts_with(&self.workdir) {
            return None;
        }

        let rest = path.strip_prefix(project_dir).ok()?;
        let redirected = self.workdir.join(rest);
        let mut args = args.clone();
        args["path"] = Value::String(redirected.to_string_lossy().into_owned());
        Some((
            args,
            original.to_string(),
            redirected.to_string_lossy().into_owned(),
        ))
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
        let mut tools = vec![
            read_file::definition(),
            write_file::definition(),
            edit_file::definition(),
            run_command::definition(),
            load_skill::definition(),
            web_search::definition(),
            web_fetch::definition(),
        ];
        if self.canvas.is_some() {
            tools.push(canvas::definition());
        }
        if self.mcp_offered {
            tools.extend(mcp::meta_tool_definitions());
        }
        Value::Array(tools)
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
        let redirected = self.redirected(name, args);
        let args = redirected.as_ref().map_or(args, |(a, _, _)| a);
        // An `mcp_call` is checked as `{server}_{tool}`, so the human's
        // per-tool rules apply to the tool it reaches.
        let mcp_name = mcp_call::rule_name(name, args);
        rule_request::check(
            &self.rules,
            &self.workdir,
            self.home.as_deref(),
            &self.protection,
            mcp_name.as_deref().unwrap_or(name),
            args,
        )
    }

    fn offers_mcp(&self) -> bool {
        self.mcp_offered
    }

    fn mcp_servers(&self) -> &[McpServer] {
        &self.mcp
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
        let redirected = self.redirected(name, args);
        let args = redirected.as_ref().map_or(args, |(a, _, _)| a);
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
            "mcp_tools" | "mcp_call" => mcp_call::run(
                name,
                args,
                &mcp_call::McpContext {
                    servers: &self.mcp,
                    sandbox: &self.sandbox,
                    workdir,
                    access,
                    vault: self.mcp_vault.as_ref(),
                },
            ),
            "canvas" => canvas::run(args, self.canvas.as_ref()),
            "web_search" => web_search::run(args, &self.search),
            "web_fetch" => {
                if !self.search.web_fetch {
                    ToolResult::failure(
                        "web_fetch is turned off on the Settings page (Behaviour > Web search).".to_string(),
                    )
                } else {
                    web_fetch::run(args, &self.web_cache)
                }
            },
            _ => ToolResult::failure(format!("Unknown tool: {name}")),
        };
        if result.success
            && let Some((_, from, to)) = redirected.as_ref()
            && let Some(output) = result.output.as_mut()
        {
            output.push_str(&format!(
                "\n\n(fennec: {from} is the human's checkout; used {to} in your working directory.)"
            ));
        }
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

#[cfg(test)]
mod redirect_tests {
    use serde_json::json;
    use std::path::PathBuf;

    use super::LocalTools;

    fn tools(project_dir: Option<&str>) -> LocalTools {
        let mut tools = LocalTools::new(PathBuf::from("/wt/1"));
        if let Some(dir) = project_dir {
            tools = tools.with_project_dir(PathBuf::from(dir));
        }
        tools
    }

    #[test]
    fn redirects_path_under_project_dir() {
        let tools = tools(Some("/x/proj"));
        let args = json!({ "path": "/x/proj/src/a.ts" });
        let Some((args, from, to)) = tools.redirected("read_file", &args) else {
            panic!("expected redirect");
        };
        assert_eq!(args["path"], "/wt/1/src/a.ts");
        assert_eq!(from, "/x/proj/src/a.ts");
        assert_eq!(to, "/wt/1/src/a.ts");
    }

    #[test]
    fn does_not_redirect_sibling_prefix() {
        let tools = tools(Some("/x/proj"));
        let args = json!({ "path": "/x/proj2/src/a.ts" });
        assert!(tools.redirected("write_file", &args).is_none());
    }

    #[test]
    fn does_not_redirect_relative_path() {
        let tools = tools(Some("/x/proj"));
        let args = json!({ "path": "src/a.ts" });
        assert!(tools.redirected("edit_file", &args).is_none());
    }

    #[test]
    fn does_not_redirect_without_project_dir() {
        let tools = tools(None);
        let args = json!({ "path": "/x/proj/src/a.ts" });
        assert!(tools.redirected("read_file", &args).is_none());
    }

    #[test]
    fn never_redirects_run_command() {
        let tools = tools(Some("/x/proj"));
        let args = json!({ "path": "/x/proj/src/a.ts" });
        assert!(tools.redirected("run_command", &args).is_none());
    }
}

#[cfg(test)]
mod mcp_rule_tests {
    use super::LocalTools;
    use crate::agent::ports::Toolbox;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn an_mcp_call_is_checked_against_the_rules_of_the_tool_it_reaches() {
        let rule = rules::Rule {
            id: 1,
            scope: rules::Scope::Global,
            target: rules::Target::parse(rules::TargetKind::Tool, "api_create_issue", None)
                .unwrap(),
            action: rules::Action::Block,
            note: None,
            origin: rules::Origin::Human,
        };
        let tools =
            LocalTools::new(PathBuf::from("/wt/1")).with_rules(rules::Rules::new(vec![rule]));
        let args = json!({"server": "api", "tool": "create_issue", "arguments": {}});
        let check = tools.check_rules("mcp_call", &args);
        assert!(matches!(check.decision, rules::Decision::Block(_)));
        let other = json!({"server": "api", "tool": "list_issues", "arguments": {}});
        let check = tools.check_rules("mcp_call", &other);
        assert!(!matches!(check.decision, rules::Decision::Block(_)));
    }
}
