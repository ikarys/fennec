//! The `App` the web adapter's tests build: a real store on disk, no turn running.

use super::App;
use crate::config::{LlmSettings, Settings};
use crate::domain::preferences::ModelParams;
use crate::notifier::Notifier;
use crate::web::turns::Turns;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A file-backed store: `App::store` opens one connection per call, so an
/// in-memory DB would be empty on every call.
pub(super) fn app(db_path: PathBuf) -> App {
    let settings = Settings {
        llm: LlmSettings {
            base_url: "http://localhost".into(),
            api_key: None,
            defaults: ModelParams {
                provider: "local".into(),
                model: "test".into(),
                max_tokens: 100,
                context_window: 1000,
                compact_at: 0.85,
                reasoning_effort: None,
            },
            listed: Vec::new(),
        },
        db_path,
    };
    let notifier = Arc::new(Notifier::default());
    App {
        turns: Turns::new(settings.clone(), notifier.clone()),
        settings,
        port: 0,
        notifier: notifier.clone(),
        skill_folds: Mutex::new(HashMap::new()),
        idea_reports: Mutex::new(HashMap::new()),
        idea_queues: Mutex::default(),
        worktree_scan: Mutex::default(),
        server_runs: Arc::new(Mutex::new(HashMap::new())),
        plugins: Arc::new(crate::plugins::PluginManager::new(
            crate::tools::Sandbox::Off,
            std::env::temp_dir(),
            std::env::temp_dir(),
            notifier.clone(),
        )),
        agents_home: std::env::temp_dir(),
        update_check: Mutex::default(),
    }
}
