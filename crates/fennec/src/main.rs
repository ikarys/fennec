//! `fennec`: the agent loop on the command line, until the web UI is ported.

mod chat;
mod events;
mod llm;
mod tools;

use events::{Event, Store};
use std::io::Write;
use std::path::PathBuf;

const USAGE: &str = "usage:
  fennec run [--session <id>] <prompt>   run the agent loop in the current directory
  fennec sessions                        list sessions
  fennec log <session>                   replay a session from the event store

env: FENNEC_LLM_URL (default http://127.0.0.1:8000/v1), FENNEC_MODEL (default: first of /models),
     FENNEC_API_KEY, FENNEC_MAX_TOKENS (default 32000), FENNEC_DB (default ~/.local/share/fennec/fennec.db)";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("fennec: {e}");
        std::process::exit(1);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let store = open_store()?;
    match args.first().map(String::as_str) {
        Some("run") => {
            let (session, prompt) = match args.get(1).map(String::as_str) {
                Some("--session") => (args.get(2).cloned(), args[3.min(args.len())..].join(" ")),
                _ => (None, args[1..].join(" ")),
            };
            if prompt.trim().is_empty() {
                return Err(USAGE.into());
            }
            cmd_run(&store, session, &prompt)
        }
        Some("sessions") => {
            for (id, title, workdir) in store.sessions().map_err(|e| e.to_string())? {
                println!("{id}  {workdir}  {title}");
            }
            Ok(())
        }
        Some("log") => {
            let id = args.get(1).ok_or(USAGE)?;
            let events = store.events(id).map_err(|e| e.to_string())?;
            if events.is_empty() {
                return Err(format!("no session {id}"));
            }
            let mut printer = Printer::default();
            events.iter().for_each(|e| printer.print(e));
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

fn open_store() -> Result<Store, String> {
    let path = match std::env::var_os("FENNEC_DB") {
        Some(p) => PathBuf::from(p),
        None => {
            let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
            PathBuf::from(home).join(".local/share/fennec/fennec.db")
        }
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Store::open(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn cmd_run(store: &Store, session: Option<String>, prompt: &str) -> Result<(), String> {
    let workdir = std::env::current_dir().map_err(|e| e.to_string())?;
    let base_url =
        std::env::var("FENNEC_LLM_URL").unwrap_or_else(|_| "http://127.0.0.1:8000/v1".into());
    let api_key = std::env::var("FENNEC_API_KEY").ok();
    let model_name = match std::env::var("FENNEC_MODEL") {
        Ok(m) => m,
        Err(_) => first_model(&base_url, api_key.as_deref())?,
    };
    let max_tokens = std::env::var("FENNEC_MAX_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(32_000);
    let session_id = match session {
        Some(id) => id,
        None => {
            let title: String = prompt.chars().take(60).collect();
            store
                .create_session(&workdir.display().to_string(), &title)
                .map_err(|e| e.to_string())?
        }
    };
    eprintln!(
        "session {session_id} · {model_name} · {}",
        workdir.display()
    );
    let client = llm::OpenAiClient {
        base_url,
        model: model_name.clone(),
        api_key,
    };
    let mut printer = Printer::default();
    let mut on_event = |e: &Event| printer.print(e);
    let mut cfg = chat::LoopConfig {
        model: &client,
        model_name: &model_name,
        store,
        session_id: &session_id,
        workdir: &workdir,
        max_tokens,
        on_event: &mut on_event,
    };
    let reason = chat::run_turn(&mut cfg, prompt)?;
    if reason == "complete" {
        Ok(())
    } else {
        Err(format!("turn ended: {reason}"))
    }
}

fn first_model(base_url: &str, api_key: Option<&str>) -> Result<String, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = ureq::get(&url);
    if let Some(key) = api_key {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let body: serde_json::Value = req
        .call()
        .and_then(|r| r.into_body().read_json())
        .map_err(|e| format!("{url}: {e}"))?;
    body["data"][0]["id"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("{url}: no model listed; set FENNEC_MODEL"))
}

/// Renders events as a terminal transcript: answer text plain, thinking
/// and tool output dimmed.
#[derive(Default)]
struct Printer {
    in_thinking: bool,
    /// Tool output may stop mid-line; the result mark then starts a new one.
    output_open: bool,
}

impl Printer {
    fn print(&mut self, event: &Event) {
        const DIM: &str = "\x1b[2m";
        const RESET: &str = "\x1b[0m";
        let mut out = std::io::stdout().lock();
        if self.in_thinking && !matches!(event, Event::MessageThinking { .. }) {
            self.in_thinking = false;
            let _ = writeln!(out, "{RESET}");
        }
        let _ = match event {
            Event::MessageStart {
                role,
                content: Some(c),
                ..
            } => writeln!(out, "\n\x1b[1m{role}>\x1b[0m {c}"),
            Event::MessageStart { role, .. } => writeln!(out, "\n\x1b[1m{role}>\x1b[0m"),
            Event::MessageDelta { content, .. } => write!(out, "{content}"),
            Event::MessageThinking { content, .. } => {
                let start = if self.in_thinking { "" } else { DIM };
                self.in_thinking = true;
                write!(out, "{start}{content}")
            }
            Event::ToolCall { tool_call, .. } => {
                writeln!(out, "\n▸ {} {}", tool_call.name, tool_call.arguments)
            }
            Event::ToolOutput { content, .. } => {
                self.output_open = !content.ends_with('\n');
                write!(out, "{DIM}{content}{RESET}")
            }
            Event::ToolResult { result, .. } => {
                let text = events::render_tool_result(result);
                let first = text.lines().next().unwrap_or("");
                let mark = if result.success { "✓" } else { "✗" };
                if std::mem::take(&mut self.output_open) {
                    let _ = writeln!(out);
                }
                writeln!(out, "{mark} {first} ({} ms)", result.duration_ms)
            }
            Event::ChatError { error, .. } => writeln!(out, "\n\x1b[31merror: {error}\x1b[0m"),
            Event::ChatDone { reason, .. } => writeln!(out, "\n— {reason}"),
            Event::SessionInitialized { .. } | Event::MessageDone { .. } => Ok(()),
        };
        let _ = out.flush();
    }
}
