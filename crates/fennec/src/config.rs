//! Settings from the environment: where the model is, where the store is.
//! The environment values are defaults; stored preferences override them.

use crate::domain::preferences::{ModelParams, Preferences};
use llm_providers::{Endpoint, ListedModel, list_models};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Settings {
    pub llm: LlmSettings,
    pub db_path: PathBuf,
}

#[derive(Clone)]
pub struct LlmSettings {
    pub base_url: String,
    pub api_key: Option<String>,
    /// Defaults from the environment (or the startup listing).
    pub defaults: ModelParams,
    /// The startup `/models` listing, used as a fallback and for listings.
    pub listed: Vec<ListedModel>,
}

impl Settings {
    /// `FENNEC_LLM_URL`, `FENNEC_MODEL`, `FENNEC_API_KEY`, `FENNEC_MAX_TOKENS`,
    /// `FENNEC_COMPACT_AT`, `FENNEC_DB`. The model and its context window
    /// default to what the endpoint's `/models` lists first.
    pub fn from_env() -> Result<Settings, String> {
        Ok(Settings {
            llm: LlmSettings::from_env()?,
            db_path: db_path()?,
        })
    }
}

impl LlmSettings {
    fn from_env() -> Result<LlmSettings, String> {
        let base_url =
            std::env::var("FENNEC_LLM_URL").unwrap_or_else(|_| "http://127.0.0.1:8000/v1".into());
        let api_key = std::env::var("FENNEC_API_KEY").ok();
        // fennec still starts when the endpoint is down: the human reaches
        // the UI, and the model chosen in Settings (or FENNEC_MODEL) is used
        // once the endpoint is back.
        let endpoint = Endpoint {
            base_url: base_url.clone(),
            api_key: api_key.clone(),
        };
        let listed = list_models(&endpoint).unwrap_or_else(|e| {
            eprintln!(
                "fennec: the model endpoint did not answer ({e}); starting without its listing"
            );
            Vec::new()
        });
        let model = std::env::var("FENNEC_MODEL")
            .ok()
            .filter(|m| !m.is_empty())
            .or_else(|| listed.first().map(|m| m.id.clone()))
            .unwrap_or_default();
        let context_window = listed
            .iter()
            .find(|m| m.id == model)
            .and_then(|m| m.context_window)
            .unwrap_or(128_000);
        let max_tokens = std::env::var("FENNEC_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(32_000);
        let compact_at = std::env::var("FENNEC_COMPACT_AT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.85);
        Ok(LlmSettings {
            base_url,
            api_key,
            defaults: ModelParams {
                model,
                max_tokens,
                context_window,
                compact_at,
                reasoning_effort: None,
            },
            listed,
        })
    }

    pub fn endpoint(&self) -> Endpoint {
        Endpoint {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
        }
    }

    /// The parameters the next turn should use, after stored preferences.
    pub fn for_turn(&self, prefs: &Preferences) -> ModelParams {
        prefs.resolve(&self.defaults, |id| {
            self.listed
                .iter()
                .find(|m| m.id == id)
                .and_then(|m| m.context_window)
        })
    }
}

/// `FENNEC_DB`, default `~/.local/share/fennec/fennec.db`; its directory is created.
pub fn db_path() -> Result<PathBuf, String> {
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
    Ok(path)
}

impl Settings {
    /// `FENNEC_USAGE_DB`, default `usage.db` next to the store. A file of its
    /// own: the usage ledger must outlive the sessions and survive a store reset.
    pub fn usage_path(&self) -> PathBuf {
        match std::env::var_os("FENNEC_USAGE_DB") {
            Some(p) => PathBuf::from(p),
            None => self.db_path.with_file_name("usage.db"),
        }
    }
}
