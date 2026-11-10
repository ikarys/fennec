//! What the human chose on the Settings page, stored as JSON.
//!
//! Every field has a serde default so a JSON written by an older version
//! (missing fields) still loads.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::editor::Editor;
use super::lang::Lang;
use super::mcp::McpServer;
pub use llm_providers_connector::{
    ApiKey, ModelParams, ModelPrefs, ModelRef, Provider, ReasoningEffort,
};

/// The provider of the environment's endpoint (`FENNEC_LLM_URL`); always there,
/// even with no stored entry.
pub const LOCAL_PROVIDER: &str = "local";

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    /// The model new turns use; `None` means the environment's default.
    pub selected: Option<ModelRef>,
    /// The models the composer offers first, in the human's order.
    pub favorites: Vec<ModelRef>,
    /// The providers and each one's per-model overrides; an entry with id
    /// `local` overrides only what it sets of the environment's endpoint.
    pub providers: Vec<Provider>,
    /// Whether a model's spend counts toward the projects' budgets, keyed `provider/model`
    /// (a provider id has no `/`); a model not listed follows `pricing::billed_by_default`.
    pub billing: BTreeMap<String, bool>,
    pub display: Display,
    /// `globalInstructions`: the human's instructions for every new
    /// session; `None` or blank adds nothing.
    pub global_instructions: Option<String>,
    /// The web_search tool's SearXNG instance.
    pub search: SearchPrefs,
    /// How commands run: in bwrap or on the host.
    pub sandbox: SandboxPrefs,
    /// The ideas queue's caps when it runs with the human away.
    pub autonomy: AutonomyPrefs,
    /// Whether agents may hand a task to another agent (`delegate`), and how often per turn.
    pub delegation: DelegationPrefs,
    /// What fennec writes in the integration commit.
    pub integration: IntegrationPrefs,
    /// Skill ids whose instructions every new session gets in its system prompt.
    pub always_on_skills: Vec<String>,
    /// Opt-in embedded workflows (`workflows::opt_in_defaults`) the human enabled.
    pub enabled_workflows: Vec<String>,
    /// MCP servers, configured on the Settings page.
    pub mcp: Vec<McpServer>,
    /// The human's keyboard shortcut overrides, action id → combo (`shortcut::ACTIONS`); an
    /// action not listed keeps its default, an empty combo disables it.
    pub shortcuts: BTreeMap<String, String>,
    /// Before providers (one endpoint): read, moved by `migrate`, never written.
    #[serde(rename = "model", skip_serializing)]
    pub(crate) legacy_model: Option<String>,
    #[serde(rename = "models", skip_serializing)]
    pub(crate) legacy_models: BTreeMap<String, ModelPrefs>,
}

/// `search.searxngUrl` / `search.searxngApiKey`; `webFetch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchPrefs {
    pub searxng_url: Option<String>,
    pub searxng_api_key: Option<String>,
    /// fennec's own: lets the model fetch the URLs it chooses (web_fetch); on by default.
    pub web_fetch: bool,
}

impl Default for SearchPrefs {
    fn default() -> Self {
        Self {
            searxng_url: None,
            searxng_api_key: None,
            web_fetch: true,
        }
    }
}

/// The bwrap world of commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SandboxPrefs {
    /// Off runs commands on the host: the human's call.
    pub enabled: bool,
    /// Lets commands reach the network (package installs, a dev server on the host).
    pub network: bool,
}

impl Default for SandboxPrefs {
    fn default() -> Self {
        SandboxPrefs {
            enabled: true,
            network: false,
        }
    }
}

/// `integration.modelTrailers`: one `Assisted-by` trailer per model that wrote the work; on by
/// default, the human may keep their commits free of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IntegrationPrefs {
    pub model_trailers: bool,
}

impl Default for IntegrationPrefs {
    fn default() -> Self {
        Self {
            model_trailers: true,
        }
    }
}

/// fennec's own: what one idea may spend in the ideas queue's night mode
/// before fennec stops it and goes on with the next. 0 means no cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutonomyPrefs {
    pub max_minutes_per_idea: u32,
    /// Uncached prompt tokens plus completion tokens: what a paid model bills.
    pub max_tokens_per_idea: u64,
}

impl Default for AutonomyPrefs {
    fn default() -> Self {
        AutonomyPrefs {
            max_minutes_per_idea: 120,
            max_tokens_per_idea: 2_000_000,
        }
    }
}

/// `delegation.enabled` / `delegation.maxPerTurn`: the `delegate` tool, on by default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DelegationPrefs {
    pub enabled: bool,
    /// The delegations one turn may start; 0: no limit.
    pub max_per_turn: u32,
}

impl Default for DelegationPrefs {
    fn default() -> Self {
        DelegationPrefs {
            enabled: true,
            max_per_turn: 5,
        }
    }
}

/// The UI's base font size in pixels (the CSS body size); the page zooms around it.
pub const DEFAULT_FONT_SIZE: u16 = 14;
/// The sizes the Settings page offers.
pub const FONT_SIZE_RANGE: std::ops::RangeInclusive<u16> = 13..=16;

fn default_font_size() -> u16 {
    DEFAULT_FONT_SIZE
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Display {
    pub reasoning_open: bool,
    pub tool_results_open: bool,
    #[serde(default = "default_font_size")]
    pub font_size: u16,
    /// The UI's display language.
    pub language: Lang,
    /// The IDE the "open in editor" links open.
    pub editor: Editor,
    /// Short sounds when something new needs the human's attention.
    pub sounds: bool,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            reasoning_open: true,
            tool_results_open: false,
            font_size: DEFAULT_FONT_SIZE,
            language: Lang::En,
            editor: Editor::default(),
            sounds: true,
        }
    }
}

impl Preferences {
    /// Moves the one-endpoint fields into the `local` provider.
    pub fn migrate(&mut self) {
        if let Some(model) = self.legacy_model.take()
            && self.selected.is_none()
        {
            self.selected = Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model,
            });
        }
        if self.legacy_models.is_empty() {
            return;
        }
        let legacy = std::mem::take(&mut self.legacy_models);
        let local = match self.providers.iter().position(|p| p.id == LOCAL_PROVIDER) {
            Some(index) => &mut self.providers[index],
            None => {
                self.providers.push(Provider {
                    id: LOCAL_PROVIDER.to_string(),
                    ..Default::default()
                });
                self.providers.last_mut().expect("just pushed")
            }
        };
        for (model, prefs) in legacy {
            local.models.entry(model).or_insert(prefs);
        }
    }

    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// A stored `Plain` key would have to be sealed before saving.
    pub fn has_plain_keys(&self) -> bool {
        self.providers
            .iter()
            .any(|p| matches!(p.api_key, Some(ApiKey::Plain(_))))
    }

    pub fn seal_keys(&mut self, seal: impl Fn(&str) -> String) {
        for provider in &mut self.providers {
            if let Some(ApiKey::Plain(key)) = &provider.api_key {
                provider.api_key = Some(ApiKey::Sealed(seal(key)));
            }
        }
    }

    /// Rejects values that would make a turn impossible to run.
    /// Drops the favourites of providers no longer stored: removing a provider
    /// must not make the whole page unsaveable.
    pub fn forget_orphan_favorites(&mut self) {
        let known: Vec<String> = self.providers.iter().map(|p| p.id.clone()).collect();
        self.favorites
            .retain(|f| f.provider == LOCAL_PROVIDER || known.contains(&f.provider));
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(selected) = &self.selected {
            if selected.model.is_empty() {
                return Err("selected model must not be empty".into());
            }
            if selected.provider != LOCAL_PROVIDER && self.provider(&selected.provider).is_none() {
                return Err(format!("unknown provider {}", selected.provider));
            }
        }

        let mut seen_favorites = Vec::new();
        for favorite in &self.favorites {
            if favorite.provider != LOCAL_PROVIDER && self.provider(&favorite.provider).is_none() {
                return Err(format!("unknown provider {}", favorite.provider));
            }
            if seen_favorites.contains(favorite) {
                return Err(format!(
                    "duplicate favorite {}/{}",
                    favorite.provider, favorite.model
                ));
            }
            seen_favorites.push(favorite.clone());
        }

        let mut seen = Vec::new();
        for provider in &self.providers {
            if seen.contains(&provider.id) {
                return Err(format!("duplicate provider {}", provider.id));
            }
            seen.push(provider.id.clone());

            provider.validate()?;

            let http = provider.base_url.starts_with("http://")
                || provider.base_url.starts_with("https://");
            if provider.id == LOCAL_PROVIDER {
                if !provider.base_url.is_empty() && !http {
                    return Err(
                        "the local provider's base_url must start with http:// or https://".into(),
                    );
                }
            } else if !http {
                return Err(format!(
                    "provider {}'s base_url must start with http:// or https://",
                    provider.id
                ));
            }
        }

        if !FONT_SIZE_RANGE.contains(&self.display.font_size) {
            return Err(format!("font_size must be in {FONT_SIZE_RANGE:?}"));
        }
        if let Some(url) = self.search.searxng_url.as_deref().map(str::trim)
            && !url.is_empty()
            && !url.starts_with("http://")
            && !url.starts_with("https://")
        {
            return Err("searxng_url must start with http:// or https://".into());
        }
        let mut seen_mcp = Vec::new();
        for server in &self.mcp {
            if seen_mcp.contains(&server.name) {
                return Err(format!("duplicate mcp server {}", server.name));
            }
            seen_mcp.push(server.name.clone());
            if let Some(problem) = server.problems().into_iter().next() {
                return Err(problem);
            }
        }
        super::shortcut::check(&self.shortcuts)?;
        Ok(())
    }

    /// The model a session runs on until the human picks one: `selected`,
    /// else the environment's default, else the first model a provider lists
    /// (without an environment, a provider's model is all the human named).
    /// An empty `selected` is ignored.
    /// Favourites only fill the composer's menu: they are not a default.
    pub fn default_model(&self, defaults: &ModelParams) -> ModelRef {
        self.selected
            .as_ref()
            .filter(|m| !m.model.is_empty())
            .cloned()
            .or_else(|| {
                defaults
                    .model
                    .is_empty()
                    .then(|| self.first_model())
                    .flatten()
            })
            .unwrap_or_else(|| ModelRef {
                provider: defaults.provider.clone(),
                model: defaults.model.clone(),
            })
    }

    fn first_model(&self) -> Option<ModelRef> {
        self.providers.iter().find_map(|provider| {
            provider.models.keys().next().map(|model| ModelRef {
                provider: provider.id.clone(),
                model: model.clone(),
            })
        })
    }

    /// The parameters of the model a turn runs on: a session's chosen model
    /// wins over the default model. An empty chosen model is ignored. Per model: Settings page > environment >
    /// built-in default; the context window falls back to the endpoint's
    /// current listing before the built-in default.
    pub fn resolve(
        &self,
        chosen: Option<&ModelRef>,
        defaults: &ModelParams,
        listed_window: impl Fn(&ModelRef) -> Option<u64>,
    ) -> ModelParams {
        let selected = match chosen {
            Some(chosen) if !chosen.model.is_empty() => chosen.clone(),
            _ => self.default_model(defaults),
        };
        let prefs = self
            .provider(&selected.provider)
            .and_then(|p| p.models.get(&selected.model));
        let window = listed_window(&selected);
        ModelParams::resolve(defaults, selected, prefs, window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mcp::McpTool;

    #[test]
    fn preferences_stored_before_shortcuts_load_with_none() {
        assert!(
            serde_json::from_str::<Preferences>("{}")
                .unwrap()
                .shortcuts
                .is_empty()
        );
    }

    #[test]
    fn preferences_validate_rejects_bad_shortcuts() {
        let mut prefs = serde_json::from_str::<Preferences>("{}").unwrap();
        prefs.shortcuts.insert("palette".into(), "KeyK".into());
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn search_prefs_stored_before_web_fetch_allow_it() {
        let search: SearchPrefs = serde_json::from_str(r#"{"searxngUrl":"http://s"}"#).unwrap();
        assert!(search.web_fetch);
    }

    #[test]
    fn preferences_stored_before_delegation_allow_it() {
        let prefs: Preferences = serde_json::from_str("{}").unwrap();
        assert!(prefs.delegation.enabled);
        assert_eq!(prefs.delegation.max_per_turn, 5);
    }

    #[test]
    fn preferences_stored_before_model_trailers_keep_them_on() {
        let prefs: Preferences = serde_json::from_str("{}").unwrap();
        assert!(prefs.integration.model_trailers);
    }

    fn defaults() -> ModelParams {
        ModelParams {
            provider: LOCAL_PROVIDER.into(),
            model: "default".into(),
            max_tokens: 32_000,
            context_window: 128_000,
            compact_at: 0.85,
            reasoning_effort: None,
        }
    }

    fn local_provider(model: &str, prefs: ModelPrefs) -> Provider {
        let mut models = BTreeMap::new();
        models.insert(model.to_string(), prefs);
        Provider {
            id: LOCAL_PROVIDER.to_string(),
            name: String::new(),
            base_url: String::new(),
            api_key: None,
            models,
        }
    }

    fn valid() -> Preferences {
        Preferences {
            selected: Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "m".into(),
            }),
            providers: vec![local_provider(
                "m",
                ModelPrefs {
                    price: None,
                    max_tokens: Some(100),
                    context_window: Some(2048),
                    compaction_threshold: Some(0.5),
                    reasoning_effort: Some(ReasoningEffort::High),
                },
            )],
            ..Default::default()
        }
    }

    fn local_model<'a>(prefs: &'a mut Preferences, model: &str) -> &'a mut ModelPrefs {
        let local = prefs
            .providers
            .iter_mut()
            .find(|p| p.id == LOCAL_PROVIDER)
            .unwrap();
        local.models.get_mut(model).unwrap()
    }

    #[test]
    fn empty_preferences_resolve_to_the_defaults() {
        let resolved = Preferences::default().resolve(None, &defaults(), |_| None);
        assert_eq!(resolved, defaults());
    }

    #[test]
    fn per_model_preferences_override_the_defaults() {
        let resolved = valid().resolve(None, &defaults(), |_| None);
        assert_eq!(
            resolved,
            ModelParams {
                provider: LOCAL_PROVIDER.to_string(),
                model: "m".into(),
                max_tokens: 100,
                context_window: 2048,
                compact_at: 0.5,
                reasoning_effort: Some(ReasoningEffort::High),
            }
        );
    }

    #[test]
    fn a_reasoning_effort_resolves_per_model() {
        let mut p = valid();
        p.selected = Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "a".into(),
        });
        p.providers[0].models.insert(
            "a".into(),
            ModelPrefs {
                reasoning_effort: Some(ReasoningEffort::Minimal),
                ..Default::default()
            },
        );
        assert_eq!(
            p.resolve(None, &defaults(), |_| None).reasoning_effort,
            Some(ReasoningEffort::Minimal)
        );

        p.selected = Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "b".into(),
        });
        assert_eq!(
            p.resolve(None, &defaults(), |_| None).reasoning_effort,
            None
        );

        let with_default = ModelParams {
            reasoning_effort: Some(ReasoningEffort::Low),
            ..defaults()
        };
        assert_eq!(
            p.resolve(None, &with_default, |_| None).reasoning_effort,
            Some(ReasoningEffort::Low)
        );
    }

    #[test]
    fn a_model_preference_does_not_leak_to_another_model() {
        let mut p = Preferences::default();
        p.providers.push(local_provider(
            "a",
            ModelPrefs {
                price: None,
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.5),
                reasoning_effort: None,
            },
        ));

        p.selected = Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "a".into(),
        });
        let a = p.resolve(None, &defaults(), |_| None);
        assert_eq!(a.max_tokens, 100);
        assert_eq!(a.context_window, 2048);
        assert_eq!(a.compact_at, 0.5);

        p.selected = Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "b".into(),
        });
        let b = p.resolve(None, &defaults(), |_| None);
        assert_eq!(b.max_tokens, 32_000);
        assert_eq!(b.context_window, 128_000);
        assert_eq!(b.compact_at, 0.85);
    }

    #[test]
    fn a_model_without_preferences_uses_the_listed_window() {
        let p = Preferences {
            selected: Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "listed".into(),
            }),
            ..Default::default()
        };
        let resolved = p.resolve(None, &defaults(), |ref_| {
            (ref_.provider == LOCAL_PROVIDER && ref_.model == "listed").then_some(64_000)
        });
        assert_eq!(
            resolved,
            ModelParams {
                provider: LOCAL_PROVIDER.to_string(),
                model: "listed".into(),
                max_tokens: 32_000,
                context_window: 64_000,
                compact_at: 0.85,
                reasoning_effort: None,
            }
        );
    }

    #[test]
    fn a_chosen_model_overrides_the_selected_one() {
        let mut p = valid();
        p.providers[0].models.insert(
            "other".into(),
            ModelPrefs {
                price: None,
                max_tokens: Some(200),
                context_window: Some(4096),
                compaction_threshold: Some(0.75),
                reasoning_effort: None,
            },
        );
        let chosen = ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "other".into(),
        };
        let resolved = p.resolve(Some(&chosen), &defaults(), |_| None);
        assert_eq!(
            resolved,
            ModelParams {
                provider: LOCAL_PROVIDER.to_string(),
                model: "other".into(),
                max_tokens: 200,
                context_window: 4096,
                compact_at: 0.75,
                reasoning_effort: None,
            }
        );
    }

    #[test]
    fn an_empty_chosen_model_falls_back_to_the_selected_one() {
        let p = valid();
        let chosen = ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: String::new(),
        };
        let resolved = p.resolve(Some(&chosen), &defaults(), |_| None);
        assert_eq!(resolved.model, "m");
    }

    #[test]
    fn validate_rejects_bad_fields() {
        let mut p = valid();
        local_model(&mut p, "m").max_tokens = Some(0);
        let err = p.validate().unwrap_err();
        assert!(err.contains("m"));
        assert!(err.contains("max_tokens"));

        let mut p = valid();
        local_model(&mut p, "m").context_window = Some(1023);
        let err = p.validate().unwrap_err();
        assert!(err.contains("context_window"));

        let mut p = valid();
        local_model(&mut p, "m").compaction_threshold = Some(1.0001);
        let err = p.validate().unwrap_err();
        assert!(err.contains("compaction_threshold"));

        let mut p = valid();
        local_model(&mut p, "m").compaction_threshold = Some(f64::NAN);
        assert!(p.validate().is_err());

        let mut p = valid();
        p.providers[0]
            .models
            .insert(String::new(), ModelPrefs::default());
        assert!(p.validate().unwrap_err().contains("model id"));

        let mut p = valid();
        p.selected = Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: String::new(),
        });
        assert!(p.validate().unwrap_err().contains("model"));

        let mut p = valid();
        p.selected = Some(ModelRef {
            provider: "missing".into(),
            model: "m".into(),
        });
        assert!(p.validate().unwrap_err().contains("unknown provider"));

        let mut p = valid();
        p.providers.push(Provider {
            id: "other".into(),
            name: "Other".into(),
            base_url: "ftp://example.com".into(),
            api_key: None,
            models: BTreeMap::new(),
        });
        assert!(p.validate().unwrap_err().contains("base_url"));

        let mut p = valid();
        p.providers[0].base_url = "ftp://example.com".into();
        assert!(p.validate().unwrap_err().contains("base_url"));

        let mut p = valid();
        p.providers.push(Provider {
            id: LOCAL_PROVIDER.to_string(),
            name: "Local again".into(),
            base_url: String::new(),
            api_key: None,
            models: BTreeMap::new(),
        });
        assert!(p.validate().unwrap_err().contains("duplicate provider"));

        let mut p = valid();
        p.display.font_size = 12;
        assert!(p.validate().unwrap_err().contains("font_size"));

        let mut p = valid();
        p.display.font_size = 17;
        assert!(p.validate().unwrap_err().contains("font_size"));

        let mut p = valid();
        p.search.searxng_url = Some("ftp://example.com".into());
        assert!(p.validate().unwrap_err().contains("searxng_url"));
    }

    #[test]
    fn a_provider_model_is_the_default_when_nothing_else_names_one() {
        let mut p = valid();
        p.selected = None;
        p.providers[0].models.clear();
        p.providers[0]
            .models
            .insert("listed".into(), ModelPrefs::default());
        let no_environment = ModelParams {
            model: String::new(),
            ..defaults()
        };

        let chosen = p.default_model(&no_environment);
        assert_eq!(chosen.model, "listed");
        assert_eq!(chosen.provider, p.providers[0].id);

        assert_eq!(p.default_model(&defaults()).model, defaults().model);
    }

    #[test]
    fn orphan_favorites_are_forgotten() {
        let mut p = valid();
        let kept = ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "m".into(),
        };
        p.favorites = vec![
            kept.clone(),
            ModelRef {
                provider: "removed".into(),
                model: "m".into(),
            },
        ];
        p.forget_orphan_favorites();
        assert_eq!(p.favorites, vec![kept]);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn validate_rejects_unknown_and_duplicate_favorites() {
        let mut p = valid();
        p.favorites.push(ModelRef {
            provider: "missing".into(),
            model: "m".into(),
        });
        assert!(p.validate().unwrap_err().contains("unknown provider"));

        let mut p = valid();
        p.favorites.push(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "m".into(),
        });
        p.favorites.push(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "m".into(),
        });
        assert!(p.validate().unwrap_err().contains("duplicate favorite"));
    }

    #[test]
    fn validate_rejects_duplicate_mcp_server_names() {
        let mut p = valid();
        p.mcp = vec![
            McpServer {
                name: "fs".into(),
                command: Some("fs-mcp".into()),
                ..Default::default()
            },
            McpServer {
                name: "fs".into(),
                command: Some("fs-mcp".into()),
                ..Default::default()
            },
        ];
        assert!(p.validate().unwrap_err().contains("duplicate mcp server"));
    }

    #[test]
    fn validate_rejects_an_mcp_tool_that_collides_with_a_builtin() {
        let mut p = valid();
        p.mcp = vec![McpServer {
            name: "read".into(),
            command: Some("read-mcp".into()),
            cached_tools: vec![McpTool {
                annotations: None,
                name: "file".into(),
                description: None,
                input_schema: serde_json::json!({}),
            }],
            ..Default::default()
        }];
        assert!(
            p.validate()
                .unwrap_err()
                .contains("collides with a built-in tool")
        );
    }

    #[test]
    fn validate_accepts_a_well_formed_mcp_server() {
        let mut p = valid();
        p.mcp = vec![McpServer {
            name: "fs".into(),
            command: Some("fs-mcp".into()),
            ..Default::default()
        }];
        assert!(p.validate().is_ok());
    }

    #[test]
    fn preferences_round_trip_and_empty_json_loads() {
        let mut models = BTreeMap::new();
        models.insert(
            "m".into(),
            ModelPrefs {
                price: None,
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.25),
                reasoning_effort: Some(ReasoningEffort::XHigh),
            },
        );
        let mut p = Preferences {
            selected: Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "m".into(),
            }),
            providers: vec![Provider {
                id: LOCAL_PROVIDER.to_string(),
                name: "Local".into(),
                base_url: String::new(),
                api_key: Some(ApiKey::Env("FENNEC_API_KEY".into())),
                models,
            }],
            ..Default::default()
        };
        p.display.reasoning_open = false;
        p.display.font_size = 15;
        p.display.language = Lang::Fr;
        p.global_instructions = Some("Be terse.".into());
        p.sandbox = SandboxPrefs {
            enabled: false,
            network: true,
        };

        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"providers\""));
        assert!(json.contains("\"selected\""));
        assert!(json.contains("\"maxTokens\""));
        assert!(json.contains("\"reasoningEffort\""));
        assert!(json.contains("\"fontSize\""));
        assert!(json.contains("\"language\""));
        assert!(json.contains("\"globalInstructions\""));
        assert!(json.contains("\"sandbox\""));
        assert_eq!(serde_json::from_str::<Preferences>(&json).unwrap(), p);
        assert_eq!(
            serde_json::from_str::<Preferences>("{}").unwrap(),
            Preferences::default()
        );
    }

    #[test]
    fn a_preferences_json_without_the_new_fields_loads() {
        let mut p: Preferences = serde_json::from_str(
            r#"{"models":{"m":{"maxTokens":100}},"display":{"reasoningOpen":true,"toolResultsOpen":false}}"#,
        )
        .unwrap();
        p.migrate();
        assert_eq!(p.display.font_size, DEFAULT_FONT_SIZE);
        assert_eq!(p.display.language, Lang::En);
        assert!(p.display.sounds);
        assert_eq!(
            p.provider(LOCAL_PROVIDER)
                .unwrap()
                .models
                .get("m")
                .unwrap()
                .reasoning_effort,
            None
        );
        assert_eq!(p.sandbox, SandboxPrefs::default());
        assert!(p.sandbox.enabled);
        assert!(!p.sandbox.network);
        assert!(p.validate().is_ok());
    }

    /// The wire model id and the parameters of a session from before providers
    /// must not change, or its prompt cache is lost.
    #[test]
    fn legacy_preferences_resolve_to_the_same_parameters() {
        let mut p: Preferences = serde_json::from_str(
            r#"{"model":"qwen3.8-27b","models":{"qwen3.8-27b":{"maxTokens":100,"contextWindow":2048,"compactionThreshold":0.5,"reasoningEffort":"high"}}}"#,
        )
        .unwrap();
        p.migrate();
        assert_eq!(
            p.resolve(None, &defaults(), |_| None),
            ModelParams {
                provider: LOCAL_PROVIDER.to_string(),
                model: "qwen3.8-27b".into(),
                max_tokens: 100,
                context_window: 2048,
                compact_at: 0.5,
                reasoning_effort: Some(ReasoningEffort::High),
            }
        );
    }

    #[test]
    fn legacy_preferences_without_a_model_resolve_to_the_defaults() {
        for json in ["{}", r#"{"models":{}}"#] {
            let mut p: Preferences = serde_json::from_str(json).unwrap();
            p.migrate();
            assert_eq!(p.resolve(None, &defaults(), |_| None), defaults());
        }
    }

    #[test]
    fn migration_is_idempotent_and_writes_no_legacy_field() {
        let mut p: Preferences =
            serde_json::from_str(r#"{"model":"m","models":{"m":{"maxTokens":100}}}"#).unwrap();
        p.migrate();
        let once = p.clone();
        p.migrate();
        assert_eq!(p, once);

        let value = serde_json::to_value(&p).unwrap();
        assert!(value.get("model").is_none());
        assert!(value.get("models").is_none());

        let mut reloaded: Preferences = serde_json::from_value(value).unwrap();
        reloaded.migrate();
        assert_eq!(reloaded, once);
    }

    #[test]
    fn migration_keeps_newer_values() {
        let mut p: Preferences =
            serde_json::from_str(r#"{"model":"old","models":{"old":{"maxTokens":1}}}"#).unwrap();
        p.selected = Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "new".into(),
        });

        let mut models = BTreeMap::new();
        models.insert(
            "new".into(),
            ModelPrefs {
                max_tokens: Some(2),
                ..Default::default()
            },
        );
        p.providers.push(Provider {
            id: LOCAL_PROVIDER.to_string(),
            name: String::new(),
            base_url: String::new(),
            api_key: None,
            models,
        });

        p.migrate();
        assert_eq!(
            p.selected,
            Some(ModelRef {
                provider: LOCAL_PROVIDER.to_string(),
                model: "new".into()
            })
        );
        let local = p.provider(LOCAL_PROVIDER).unwrap();
        assert_eq!(local.models.get("new").unwrap().max_tokens, Some(2));
        assert_eq!(local.models.get("old").unwrap().max_tokens, Some(1));
    }

    #[test]
    fn the_listed_window_is_asked_for_the_selected_provider_and_model() {
        let p = Preferences {
            selected: Some(ModelRef {
                provider: "other".into(),
                model: "m".into(),
            }),
            ..Default::default()
        };
        let resolved = p.resolve(None, &defaults(), |selected| {
            (selected.provider == "other" && selected.model == "m").then_some(64_000)
        });
        assert_eq!(resolved.context_window, 64_000);
    }

    #[test]
    fn seal_keys_seals_only_plain_keys() {
        let mut p = Preferences::default();
        p.providers.push(Provider {
            id: "a".into(),
            name: "A".into(),
            base_url: "http://a".into(),
            api_key: Some(ApiKey::Plain("plain".into())),
            models: BTreeMap::new(),
        });
        p.providers.push(Provider {
            id: "b".into(),
            name: "B".into(),
            base_url: "http://b".into(),
            api_key: Some(ApiKey::Env("B_KEY".into())),
            models: BTreeMap::new(),
        });

        assert!(p.has_plain_keys());
        p.seal_keys(|key| format!("sealed:{key}"));
        assert!(!p.has_plain_keys());
        assert_eq!(
            p.providers[0].api_key,
            Some(ApiKey::Sealed("sealed:plain".into()))
        );
        assert_eq!(p.providers[1].api_key, Some(ApiKey::Env("B_KEY".into())));
    }

    #[test]
    fn an_old_stored_tariff_is_ignored() {
        let p: Preferences = serde_json::from_str(r#"{"tariff":{"inputPerM":1}}"#).unwrap();
        assert!(p.providers.is_empty());
    }
}
