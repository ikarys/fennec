//! Turns started from the UI: each runs on its own thread, reports through
//! the event log, and can be stopped. One turn at a time per session.

use crate::agent::turn::{Turn, run_turn};
use crate::config::Settings;
use crate::domain::event::Event;
use crate::llm::OpenAiClient;
use crate::store::SqliteStore;
use crate::tools::LocalTools;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct Turns {
    settings: Settings,
    /// Sessions with a turn running, and the flag that stops it. Shared with
    /// each turn's thread, which removes its entry when it ends.
    running: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl Turns {
    pub fn new(settings: Settings) -> Turns {
        Turns {
            settings,
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

    /// Starts a turn with `prompt`; refuses while one runs on the session.
    pub fn start(&self, session_id: &str, workdir: &str, prompt: String) -> Result<(), String> {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(session_id) {
                return Err("a turn is already running".into());
            }
            running.insert(session_id.to_string(), stop.clone());
        }
        let settings = self.settings.clone();
        let running = self.running.clone();
        let (session_id, workdir) = (session_id.to_string(), PathBuf::from(workdir));
        std::thread::spawn(move || {
            if let Err(e) = run(&settings, &session_id, workdir, &prompt, &stop) {
                eprintln!("fennec: session {session_id}: {e}");
            }
            running.lock().unwrap().remove(&session_id);
        });
        Ok(())
    }
}

fn run(
    settings: &Settings,
    session_id: &str,
    workdir: PathBuf,
    prompt: &str,
    stop: &AtomicBool,
) -> Result<(), String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let model = OpenAiClient::new(&settings.llm);
    let tools = LocalTools::new(workdir.clone());
    let mut on_event = |_: &Event| {};
    let mut turn = Turn {
        model: &model,
        log: &store,
        tools: &tools,
        session_id,
        workdir: &workdir,
        max_tokens: settings.llm.max_tokens,
        context_window: settings.llm.context_window,
        stop,
        on_event: &mut on_event,
    };
    run_turn(&mut turn, prompt).map(|_| ())
}
