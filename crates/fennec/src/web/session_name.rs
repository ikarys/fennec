//! Background session-name generation, ported from openfox's
//! `generateSessionNameForSession`: one model call, no project context, no
//! thinking, and the result is stored as a `session.name_generated` event.

use super::notifier::{Notifier, NotifyingLog};
use crate::agent::ports::{Delta, EventLog, Model, Request};
use crate::config::Settings;
use crate::domain::event::Event;
use crate::domain::preferences::ReasoningEffort;
use crate::domain::session_name;
use crate::llm::OpenAiClient;
use crate::store::SqliteStore;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Spawns the model call on its own thread so the creation response and the
/// first turn are not blocked by it.
pub fn spawn(settings: Settings, notifier: Arc<Notifier>, session_id: String, prompt: String) {
    std::thread::spawn(move || {
        if let Err(e) = generate(&settings, &notifier, &session_id, &prompt) {
            eprintln!("fennec: session {session_id}: session name: {e}");
        }
    });
}

fn generate(
    settings: &Settings,
    notifier: &Notifier,
    session_id: &str,
    prompt: &str,
) -> Result<(), String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let prefs = store.preferences()?;
    let params = settings.llm.for_turn(&prefs);
    let model = OpenAiClient::new(&settings.llm, &params.model);
    let stop = AtomicBool::new(false);

    // Keep the prompt short: it is only a naming hint, not a full context.
    let truncated: String = prompt.chars().take(2000).collect();
    let content = session_name::prompt(&truncated);
    let messages = vec![json!({ "role": "user", "content": content })];
    let tools = json!([]);
    let request = Request {
        system_prompt: "",
        messages: &messages,
        tools: &tools,
        max_tokens: 4096,
        reasoning_effort: Some(ReasoningEffort::None),
        stop: &stop,
    };

    let mut on_delta = |_: Delta| {};
    let completion = model.stream(&request, &mut on_delta)?;
    let Some(name) = session_name::clean(&completion.content) else {
        eprintln!("fennec: session {session_id}: session name: no usable name");
        return Ok(());
    };

    store.rename_session(session_id, &name)?;
    let log = NotifyingLog {
        log: &store,
        notifier,
    };
    log.append(session_id, &Event::SessionNameGenerated { name })?;
    Ok(())
}
