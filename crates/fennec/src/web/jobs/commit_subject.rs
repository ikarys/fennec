//! One model call proposing the integration's English commit subject
//! (indicative). Same shape as `web/session_name.rs` but synchronous and
//! called by the route.

use crate::agent::ports::{Delta, Model, Request};
use crate::config::Settings;
use crate::domain::commit_subject;
use crate::domain::preferences::ReasoningEffort;
use crate::llm::OpenAiClient;
use crate::store::SqliteStore;
use serde_json::json;
use std::sync::atomic::AtomicBool;

/// Proposes an English Conventional Commit subject for the integration.
pub fn suggest(
    settings: &Settings,
    task: &str,
    request: &str,
    title: &str,
    commits: &[&str],
    files: &[(&str, u64, u64)],
) -> Result<Option<String>, String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let prefs = store.preferences()?;
    let params = settings.llm.for_turn(&prefs, None);
    let model = OpenAiClient::new(settings.endpoint(&prefs, &params.provider)?, &params.model);
    let stop = AtomicBool::new(false);

    let content = commit_subject::prompt(task, request, title, commits, files);
    let messages = vec![json!({ "role": "user", "content": content })];
    let tools = json!([]);
    let model_request = Request {
        system_prompt: "",
        messages: &messages,
        tools: &tools,
        max_tokens: 4096,
        reasoning_effort: Some(ReasoningEffort::None),
        stop: &stop,
        session: "",
    };

    let mut on_delta = |_: Delta| {};
    let completion = model.stream(&model_request, &mut on_delta)?;
    Ok(commit_subject::clean(&completion.content))
}
