//! Settings from the environment: where the model is, where the store is.
//! The environment values are defaults; stored preferences override them.

use crate::domain::preferences::{LOCAL_PROVIDER, ModelParams, ModelRef, Preferences};
use llm_providers::{ApiKey, Endpoint, ListedModel, MasterKey, list_models};
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

    /// The master key that seals stored API keys, next to the store: the
    /// sandbox hides the store's directory, so commands never read it.
    pub fn secret_path(&self) -> PathBuf {
        self.db_path.with_file_name("secret.key")
    }

    /// Where `provider` is reached. The local provider is the environment's
    /// endpoint, overridden only by what its stored entry sets.
    pub fn endpoint(&self, prefs: &Preferences, provider: &str) -> Result<Endpoint, String> {
        let entry = prefs.provider(provider);
        let (base_url, env_key) = if provider == LOCAL_PROVIDER {
            let base_url = entry
                .map(|p| p.base_url.clone())
                .filter(|url| !url.is_empty())
                .unwrap_or_else(|| self.llm.base_url.clone());
            (base_url, self.llm.api_key.clone())
        } else {
            let entry = entry.ok_or_else(|| format!("unknown provider {provider}"))?;
            (entry.base_url.clone(), None)
        };
        let api_key = match entry.and_then(|p| p.api_key.clone()) {
            Some(key) => self.resolve_api_key(key)?,
            None => env_key,
        };
        Ok(Endpoint { base_url, api_key })
    }

    /// The master key file is read only when a sealed key is used.
    pub(crate) fn resolve_api_key(&self, key: ApiKey) -> Result<Option<String>, String> {
        key.resolve(|sealed| {
            let master = MasterKey::load_or_create(&self.secret_path())
                .map_err(|e| format!("read the master key: {e}"))?;
            master
                .open(sealed)
                .map_err(|e| format!("open the stored API key: {e}"))
        })
    }

    /// `FENNEC_USAGE_DB`, default `usage.db` next to the store. A file of its
    /// own: the usage ledger must outlive the sessions and survive a store reset.
    pub fn usage_path(&self) -> PathBuf {
        match std::env::var_os("FENNEC_USAGE_DB") {
            Some(p) => PathBuf::from(p),
            None => self.db_path.with_file_name("usage.db"),
        }
    }

    /// `FENNEC_RULES_DB`, default `rules.db` next to the store. A file of its
    /// own: the human's rules are owned by the `rules` crate, not fennec's store.
    pub fn rules_path(&self) -> PathBuf {
        match std::env::var_os("FENNEC_RULES_DB") {
            Some(p) => PathBuf::from(p),
            None => self.db_path.with_file_name("rules.db"),
        }
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
                provider: LOCAL_PROVIDER.to_string(),
                model,
                max_tokens,
                context_window,
                compact_at,
                reasoning_effort: None,
            },
            listed,
        })
    }

    /// The parameters the next turn should use, after stored preferences.
    pub fn for_turn(&self, prefs: &Preferences, chosen: Option<&ModelRef>) -> ModelParams {
        prefs.resolve(chosen, &self.defaults, |selected| {
            if selected.provider == LOCAL_PROVIDER {
                self.listed
                    .iter()
                    .find(|m| m.id == selected.model)
                    .and_then(|m| m.context_window)
            } else {
                None
            }
        })
    }
}

/// How many model rounds may run at once in the process, from
/// `FENNEC_MODEL_CONCURRENCY`. Default one: a local GPU serves one round at a
/// time; raise it when the endpoint tolerates parallel requests.
pub fn model_concurrency() -> usize {
    std::env::var("FENNEC_MODEL_CONCURRENCY")
        .ok()
        .and_then(|v| parse_concurrency(&v))
        .unwrap_or(1)
}

/// A valid concurrency is a positive integer; anything else is rejected so
/// `model_concurrency` falls back to its default.
fn parse_concurrency(value: &str) -> Option<usize> {
    value.parse().ok().filter(|n| *n >= 1)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preferences::{ModelRef, Provider};
    use std::collections::BTreeMap;
    use std::path::Path;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fennec-config-{}-{name}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn settings(dir: &Path) -> Settings {
        Settings {
            llm: LlmSettings {
                base_url: "http://127.0.0.1:8000/v1".into(),
                api_key: Some("env-key".into()),
                defaults: ModelParams {
                    provider: LOCAL_PROVIDER.to_string(),
                    model: "m".into(),
                    max_tokens: 100,
                    context_window: 2048,
                    compact_at: 0.85,
                    reasoning_effort: None,
                },
                listed: vec![],
            },
            db_path: dir.join("fennec.db"),
        }
    }

    fn env_endpoint(settings: &Settings) -> Endpoint {
        Endpoint {
            base_url: settings.llm.base_url.clone(),
            api_key: settings.llm.api_key.clone(),
        }
    }

    fn local_provider(base_url: &str, api_key: Option<ApiKey>) -> Provider {
        Provider {
            id: LOCAL_PROVIDER.to_string(),
            name: String::new(),
            base_url: base_url.into(),
            api_key,
            models: BTreeMap::new(),
        }
    }

    #[test]
    fn model_concurrency_defaults_to_one_and_rejects_non_positive() {
        assert_eq!(parse_concurrency("2"), Some(2));
        assert_eq!(parse_concurrency("0"), None);
        assert_eq!(parse_concurrency("abc"), None);
        assert!(model_concurrency() >= 1);
    }

    /// Same server and key as before providers, so the same prompt cache.
    #[test]
    fn the_local_endpoint_is_unchanged_without_a_stored_entry() {
        let dir = test_dir("local_unchanged");
        let settings = settings(&dir);

        assert_eq!(
            settings
                .endpoint(&Preferences::default(), LOCAL_PROVIDER)
                .unwrap(),
            env_endpoint(&settings)
        );

        let mut legacy: Preferences =
            serde_json::from_str(r#"{"model":"m","models":{"m":{"maxTokens":5}}}"#).unwrap();
        legacy.migrate();
        assert_eq!(
            settings.endpoint(&legacy, LOCAL_PROVIDER).unwrap(),
            env_endpoint(&settings)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stored_local_entry_overrides_the_environment() {
        let dir = test_dir("stored_local");
        let settings = settings(&dir);

        let mut prefs = Preferences::default();
        prefs.providers.push(local_provider(
            "http://stored:1234/v1",
            Some(ApiKey::Plain("stored-key".into())),
        ));

        assert_eq!(
            settings.endpoint(&prefs, LOCAL_PROVIDER).unwrap(),
            Endpoint {
                base_url: "http://stored:1234/v1".into(),
                api_key: Some("stored-key".into()),
            }
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_provider_is_an_error() {
        let dir = test_dir("unknown");
        let settings = settings(&dir);

        let err = settings
            .endpoint(&Preferences::default(), "other")
            .unwrap_err();
        assert!(err.contains("unknown provider"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sealed_key_opens_with_the_master_key() {
        let dir = test_dir("sealed");
        let settings = settings(&dir);

        let master = MasterKey::load_or_create(&settings.secret_path()).unwrap();
        let sealed = master.seal("plain-key");

        let mut prefs = Preferences::default();
        prefs
            .providers
            .push(local_provider("", Some(ApiKey::Sealed(sealed))));

        let endpoint = settings.endpoint(&prefs, LOCAL_PROVIDER).unwrap();
        assert_eq!(endpoint.api_key, Some("plain-key".into()));
        assert!(settings.secret_path().exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_env_key_resolves_from_the_environment() {
        let dir = test_dir("env_key");
        let settings = settings(&dir);

        let mut prefs = Preferences::default();
        prefs
            .providers
            .push(local_provider("", Some(ApiKey::Env("HOME".into()))));

        let endpoint = settings.endpoint(&prefs, LOCAL_PROVIDER).unwrap();
        assert_eq!(endpoint.api_key, std::env::var("HOME").ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn for_turn_uses_the_listed_window_only_for_the_local_provider() {
        let dir = test_dir("for_turn");
        let mut settings = settings(&dir);
        settings.llm.listed = vec![ListedModel {
            id: "listed".into(),
            context_window: Some(64_000),
        }];

        let mut prefs = Preferences {
            selected: Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "listed".into(),
            }),
            ..Default::default()
        };
        assert_eq!(settings.llm.for_turn(&prefs, None).context_window, 64_000);

        prefs.selected = Some(ModelRef {
            provider: "other".into(),
            model: "listed".into(),
        });
        assert_eq!(settings.llm.for_turn(&prefs, None).context_window, 2048);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
