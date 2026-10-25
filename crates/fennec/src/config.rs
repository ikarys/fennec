//! Settings from the environment: where the model is, where the store is.

use std::path::PathBuf;

#[derive(Clone)]
pub struct Settings {
    pub llm: LlmSettings,
    pub db_path: PathBuf,
}

#[derive(Clone)]
pub struct LlmSettings {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub max_tokens: u32,
    pub context_window: u64,
    /// The compaction threshold as a fraction of the context window
    /// (openfox `compactionThreshold`); 0 disables it.
    pub compact_at: f64,
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
        let listed = list_models(&base_url, api_key.as_deref())?;
        let model = match std::env::var("FENNEC_MODEL") {
            Ok(m) => m,
            Err(_) => listed["data"][0]["id"]
                .as_str()
                .map(String::from)
                .ok_or_else(|| format!("{base_url}/models lists no model; set FENNEC_MODEL"))?,
        };
        let context_window = listed["data"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|m| m["id"] == model.as_str())
            .and_then(|m| m["max_model_len"].as_u64())
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
            model,
            api_key,
            max_tokens,
            context_window,
            compact_at,
        })
    }
}

fn list_models(base_url: &str, api_key: Option<&str>) -> Result<serde_json::Value, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = ureq::get(&url);
    if let Some(key) = api_key {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    req.call()
        .and_then(|r| r.into_body().read_json())
        .map_err(|e| format!("{url}: {e}"))
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
