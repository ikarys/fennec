//! Terminal adapter: run a turn in the current directory, list sessions,
//! replay one.

mod transcript;

use crate::agent::ports::EventLog;
use crate::agent::turn::{Turn, run_turn};
use crate::config::Settings;
use crate::domain::event::{DoneReason, Event};
use crate::llm::OpenAiClient;
use crate::store::SqliteStore;
use crate::tools::LocalTools;
use std::sync::atomic::AtomicBool;
use transcript::Transcript;

/// Runs one turn in the current directory, in `session` or a new one.
pub fn run(settings: &Settings, session: Option<String>, prompt: &str) -> Result<(), String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let workdir = std::env::current_dir().map_err(|e| e.to_string())?;
    let project = store.project_for(&workdir.display().to_string())?;
    let session_id = match session {
        Some(id) => id,
        None => store.create_session(&project, &prompt.chars().take(80).collect::<String>())?,
    };
    eprintln!(
        "session {session_id} · {} · {}",
        settings.llm.model,
        workdir.display()
    );
    let model = OpenAiClient::new(&settings.llm);
    let tools = LocalTools::new(workdir.clone());
    // ponytail: no Ctrl-C handling, the process just dies; a signal handler
    // setting `stop` when an interrupted turn must end cleanly.
    let stop = AtomicBool::new(false);
    let mut transcript = Transcript::default();
    let mut on_event = |e: &Event| transcript.print(e);
    let mut turn = Turn {
        model: &model,
        log: &store,
        tools: &tools,
        session_id: &session_id,
        workdir: &workdir,
        max_tokens: settings.llm.max_tokens,
        context_window: settings.llm.context_window,
        stop: &stop,
        on_event: &mut on_event,
    };
    match run_turn(&mut turn, prompt)? {
        DoneReason::Complete => Ok(()),
        reason => Err(format!("turn ended: {reason}")),
    }
}

pub fn list_sessions(store: &SqliteStore) -> Result<(), String> {
    let projects = store.projects()?;
    for s in store.sessions(None)? {
        let workdir = projects
            .iter()
            .find(|p| p.id == s.project_id)
            .map_or("?", |p| p.workdir.as_str());
        println!("{}  {workdir}  {}", s.id, s.title);
    }
    Ok(())
}

pub fn replay(store: &SqliteStore, session_id: &str) -> Result<(), String> {
    let events = store.events(session_id)?;
    if events.is_empty() {
        return Err(format!("no session {session_id}"));
    }
    let mut transcript = Transcript::default();
    events.iter().for_each(|e| transcript.print(e));
    Ok(())
}
