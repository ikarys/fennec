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
mod tests;
