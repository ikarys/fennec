//! Turns started from the UI: each runs on its own thread, reports through
//! the event log, and can be stopped. One turn at a time per session.

use super::notifier::{Notifier, NotifyingLog};
use crate::agent::turn::{Turn, compact_session, run_turn};
use crate::config::Settings;
use crate::domain::event::Event;
use crate::domain::slash::TurnInput;
use crate::instructions::all_instructions;
use crate::llm::OpenAiClient;
use crate::skills;
use crate::store::SqliteStore;
use crate::tools::LocalTools;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct Turns {
    settings: Settings,
    notifier: Arc<Notifier>,
    /// Sessions with a turn running, and the flag that stops it. Shared with
    /// each turn's thread, which removes its entry when it ends.
    running: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl Turns {
    pub fn new(settings: Settings, notifier: Arc<Notifier>) -> Turns {
        Turns {
            settings,
            notifier,
            running: Arc::default(),
        }
    }

    pub fn is_running(&self, session_id: &str) -> bool {
        self.running.lock().unwrap().contains_key(session_id)
    }

    pub fn stop(&self, session_id: &str) {
        if let Some(flag) = self.running.lock().unwrap().get(session_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Starts a turn; refuses while one runs on the session.
    pub fn start(&self, session_id: &str, workdir: &str, input: TurnInput) -> Result<(), String> {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(session_id) {
                return Err("a turn is already running".into());
            }
            running.insert(session_id.to_string(), stop.clone());
        }
        let settings = self.settings.clone();
        let notifier = self.notifier.clone();
        let running = self.running.clone();
        let (session_id, workdir) = (session_id.to_string(), PathBuf::from(workdir));
        std::thread::spawn(move || {
            if let Err(e) = run(&settings, &notifier, &session_id, workdir, &input, &stop) {
                eprintln!("fennec: session {session_id}: {e}");
            }
            running.lock().unwrap().remove(&session_id);
        });
        Ok(())
    }
}

fn run(
    settings: &Settings,
    notifier: &Notifier,
    session_id: &str,
    workdir: PathBuf,
    input: &TurnInput,
    stop: &AtomicBool,
) -> Result<(), String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let log = NotifyingLog {
        log: &store,
        notifier,
    };
    let prefs = store.preferences()?;
    let params = settings.llm.for_turn(&prefs);
    let instructions = all_instructions(prefs.global_instructions.as_deref(), &workdir);
    let skills = skills::discover(&workdir);
    let model = OpenAiClient::new(&settings.llm, &params.model);
    let tools = LocalTools::new(workdir.clone());
    let mut on_event = |_: &Event| {};
    let mut turn = Turn {
        model: &model,
        log: &log,
        tools: &tools,
        session_id,
        workdir: &workdir,
        instructions: &instructions,
        skills: &skills,
        max_tokens: params.max_tokens,
        context_window: params.context_window,
        compact_at: params.compact_at,
        reasoning_effort: params.reasoning_effort,
        stop,
        on_event: &mut on_event,
    };
    match input {
        TurnInput::Prompt(prompt) => run_turn(&mut turn, prompt).map(|_| ()),
        TurnInput::Compact => compact_session(&mut turn).map(|_| ()),
    }
}
