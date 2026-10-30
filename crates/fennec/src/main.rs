//! `fennec`: the composition root. Parses the command line, reads the
//! settings and hands them to the web or terminal adapter.

mod adr;
mod agent;
mod agents;
mod architecture;
mod cli;
mod config;
mod domain;
mod folders;
mod git;
mod instructions;
mod integration;
mod llm;
mod metering;
mod module_map;
mod protected;
mod skills;
mod store;
mod tools;
mod web;
mod workflows;
mod worktree;

use config::Settings;
use store::SqliteStore;

const USAGE: &str = "usage:
  fennec [serve] [--port <n>]              web UI on http://127.0.0.1:7777
  fennec run [--session <id>] [--agent <id>] [--image <path>]... <prompt>
                                           run the agent loop in the current directory;
                                           agent: plan (default, read-only) or builder
  fennec compact <session>                 compact a session in the current directory
  fennec sessions                          list sessions
  fennec log <session>                     replay a session from the event store

env: FENNEC_LLM_URL (default http://127.0.0.1:8000/v1), FENNEC_MODEL (default: first of /models),
     FENNEC_API_KEY, FENNEC_MAX_TOKENS (default 32000), FENNEC_COMPACT_AT (default 0.85),
     FENNEC_MODEL_CONCURRENCY (local endpoints, default 1), FENNEC_DB (default ~/.local/share/fennec/fennec.db);
     these are defaults; the Settings page overrides them";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("fennec: {e}");
        std::process::exit(1);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("serve") | Some("--port") => {
            let rest = if args.first().is_some_and(|a| a == "serve") {
                &args[1..]
            } else {
                args
            };
            let port = match rest {
                [] => 7777,
                [flag, n] if flag == "--port" => n.parse().map_err(|_| USAGE)?,
                _ => return Err(USAGE.into()),
            };
            web::serve(port, Settings::from_env()?)
        }
        Some("run") => {
            let mut session = None;
            let mut agent = None;
            let mut images = Vec::new();
            let mut i = 1;
            while i < args.len() {
                let flag = &args[i];
                if !flag.starts_with("--") {
                    break;
                }
                let value = args.get(i + 1).ok_or(USAGE)?.clone();
                match flag.as_str() {
                    "--session" => session = Some(value),
                    "--agent" => agent = Some(value),
                    "--image" => images.push(value),
                    _ => return Err(USAGE.into()),
                }
                i += 2;
            }
            let prompt = args[i..].join(" ");
            if prompt.trim().is_empty() {
                return Err(USAGE.into());
            }
            cli::run(&Settings::from_env()?, session, agent, &images, &prompt)
        }
        Some("compact") => cli::compact(&Settings::from_env()?, args.get(1).ok_or(USAGE)?),
        Some("sessions") => cli::list_sessions(&open_store()?),
        Some("log") => cli::replay(&open_store()?, args.get(1).ok_or(USAGE)?),
        _ => Err(USAGE.into()),
    }
}

/// `sessions` and `log` only read the store: no need to reach the model.
fn open_store() -> Result<SqliteStore, String> {
    Ok(SqliteStore::open(&config::db_path()?)?)
}
