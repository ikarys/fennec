//! What the human chose on the Settings page, stored as JSON.
//!
//! Every field has a serde default so a JSON written by an older version
//! (missing fields) still loads.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    /// The model new turns use; `None` means the environment's default.
    pub model: Option<String>,
    /// Per-model overrides, keyed by model id (openfox `ModelConfig` subset).
    pub models: BTreeMap<String, ModelPrefs>,
    pub display: Display,
    /// openfox `globalInstructions`: the human's instructions for every new
    /// session; `None` or blank adds nothing.
    pub global_instructions: Option<String>,
    /// The web_search tool's SearXNG instance.
    pub search: SearchPrefs,
    /// How commands run: in bwrap or on the host.
    pub sandbox: SandboxPrefs,
}

/// openfox `search.searxngUrl` / `search.searxngApiKey`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchPrefs {
    pub searxng_url: Option<String>,
    pub searxng_api_key: Option<String>,
}

/// fennec's own (openfox runs commands on the host): the bwrap world of commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SandboxPrefs {
    /// Off runs commands on the host, as openfox does: the human's call.
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

/// openfox `ReasoningEffort`: how much a model may think.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelPrefs {
    pub max_tokens: Option<u32>,
    /// openfox `contextWindow`; overrides what the endpoint lists.
    pub context_window: Option<u64>,
    /// openfox `compactionThreshold`; 0 disables compaction.
    pub compaction_threshold: Option<f64>,
    /// openfox `reasoningEffort`; `None` sends no `reasoning_effort` field.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// The UI's base font size in pixels (the CSS body size); the page zooms around it.
pub const DEFAULT_FONT_SIZE: u16 = 14;
/// The sizes the Settings page offers.
pub const FONT_SIZE_RANGE: std::ops::RangeInclusive<u16> = 13..=16;
/// The most context panel pages the UI can store.
pub const MAX_PANEL_PAGES: usize = 12;
/// The longest panel page name, in characters.
pub const MAX_PANEL_PAGE_NAME: usize = 40;

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
    /// `None` means the UI's default pages.
    pub panel_pages: Option<Vec<PanelPage>>,
}

/// fennec's own: a named list of the context panel's blocks; the UI owns the block ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelPage {
    pub name: String,
    pub blocks: Vec<String>,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            reasoning_open: true,
            tool_results_open: false,
            font_size: DEFAULT_FONT_SIZE,
            panel_pages: None,
        }
    }
}

/// The concrete parameters one turn runs with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelParams {
    pub model: String,
    pub max_tokens: u32,
    pub context_window: u64,
    pub compact_at: f64,
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl Preferences {
    /// Rejects values that would make a turn impossible to run.
    pub fn validate(&self) -> Result<(), String> {
        if self.model.as_deref() == Some("") {
            return Err("model must not be empty".into());
        }
        for (id, prefs) in &self.models {
            if id.is_empty() {
                return Err("model id must not be empty".into());
            }
            if let Some(max_tokens) = prefs.max_tokens
                && max_tokens < 1
            {
                return Err(format!("model {id}: max_tokens must be >= 1"));
            }
            if let Some(context_window) = prefs.context_window
                && context_window < 1024
            {
                return Err(format!("model {id}: context_window must be >= 1024"));
            }
            if let Some(threshold) = prefs.compaction_threshold
                && !(0.0..=1.0).contains(&threshold)
            {
                return Err(format!(
                    "model {id}: compaction_threshold must be in 0.0..=1.0"
                ));
            }
        }
        if !FONT_SIZE_RANGE.contains(&self.display.font_size) {
            return Err(format!("font_size must be in {FONT_SIZE_RANGE:?}"));
        }
        if let Some(pages) = &self.display.panel_pages {
            Self::validate_panel_pages(pages)?;
        }
        if let Some(url) = self.search.searxng_url.as_deref().map(str::trim)
            && !url.is_empty()
            && !url.starts_with("http://")
            && !url.starts_with("https://")
        {
            return Err("searxng_url must start with http:// or https://".into());
        }
        Ok(())
    }

    fn validate_panel_pages(pages: &[PanelPage]) -> Result<(), String> {
        if pages.is_empty() {
            return Err("panel pages must not be empty".into());
        }
        if pages.len() > MAX_PANEL_PAGES {
            return Err(format!("panel pages must be <= {MAX_PANEL_PAGES}"));
        }

        let mut seen = Vec::new();
        for page in pages {
            let name = page.name.trim();
            if name.is_empty() {
                return Err("panel page name must not be empty".into());
            }
            if name.chars().count() > MAX_PANEL_PAGE_NAME {
                return Err(format!(
                    "panel page name must be <= {MAX_PANEL_PAGE_NAME} characters"
                ));
            }
            if seen.iter().any(|existing| existing == name) {
                return Err("panel page names must be unique".into());
            }
            seen.push(name.to_owned());

            for block in &page.blocks {
                if block.trim().is_empty() {
                    return Err("panel page block id must not be empty".into());
                }
            }
        }
        Ok(())
    }

    /// Resolves the parameters for the next turn.
    ///
    /// Precedence: Settings page > environment > built-in default.
    /// The context window additionally falls back to the endpoint's current
    /// listing before the built-in default.
    pub fn resolve(
        &self,
        defaults: &ModelParams,
        listed_window: impl Fn(&str) -> Option<u64>,
    ) -> ModelParams {
        let model = self
            .model
            .clone()
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| defaults.model.clone());
        let prefs = self.models.get(&model);
        ModelParams {
            max_tokens: prefs
                .and_then(|p| p.max_tokens)
                .unwrap_or(defaults.max_tokens),
            context_window: prefs
                .and_then(|p| p.context_window)
                .or_else(|| listed_window(&model))
                .unwrap_or(defaults.context_window),
            compact_at: prefs
                .and_then(|p| p.compaction_threshold)
                .unwrap_or(defaults.compact_at),
            reasoning_effort: prefs
                .and_then(|p| p.reasoning_effort)
                .or(defaults.reasoning_effort),
            model,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ModelParams {
        ModelParams {
            model: "default".into(),
            max_tokens: 32_000,
            context_window: 128_000,
            compact_at: 0.85,
            reasoning_effort: None,
        }
    }

    fn valid() -> Preferences {
        let mut p = Preferences {
            model: Some("m".into()),
            ..Default::default()
        };
        p.models.insert(
            "m".into(),
            ModelPrefs {
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.5),
                reasoning_effort: Some(ReasoningEffort::High),
            },
        );
        p
    }

    #[test]
    fn empty_preferences_resolve_to_the_defaults() {
        let resolved = Preferences::default().resolve(&defaults(), |_| None);
        assert_eq!(resolved, defaults());
    }

    #[test]
    fn per_model_preferences_override_the_defaults() {
        let resolved = valid().resolve(&defaults(), |_| None);
        assert_eq!(
            resolved,
            ModelParams {
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
        let mut p = Preferences {
            model: Some("a".into()),
            ..Default::default()
        };
        p.models.insert(
            "a".into(),
            ModelPrefs {
                reasoning_effort: Some(ReasoningEffort::Minimal),
                ..Default::default()
            },
        );
        assert_eq!(
            p.resolve(&defaults(), |_| None).reasoning_effort,
            Some(ReasoningEffort::Minimal)
        );

        p.model = Some("b".into());
        assert_eq!(p.resolve(&defaults(), |_| None).reasoning_effort, None);

        let with_default = ModelParams {
            reasoning_effort: Some(ReasoningEffort::Low),
            ..defaults()
        };
        assert_eq!(
            p.resolve(&with_default, |_| None).reasoning_effort,
            Some(ReasoningEffort::Low)
        );
    }

    #[test]
    fn reasoning_effort_uses_openfox_wire_names() {
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::XHigh).unwrap(),
            "\"xhigh\""
        );
        assert_eq!(
            serde_json::from_str::<ReasoningEffort>("\"xhigh\"").unwrap(),
            ReasoningEffort::XHigh
        );
        assert_eq!(
            serde_json::to_string(&ReasoningEffort::Minimal).unwrap(),
            "\"minimal\""
        );
    }

    #[test]
    fn a_model_preference_does_not_leak_to_another_model() {
        let mut p = Preferences::default();
        p.models.insert(
            "a".into(),
            ModelPrefs {
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.5),
                reasoning_effort: None,
            },
        );

        p.model = Some("a".into());
        let a = p.resolve(&defaults(), |_| None);
        assert_eq!(a.max_tokens, 100);
        assert_eq!(a.context_window, 2048);
        assert_eq!(a.compact_at, 0.5);

        p.model = Some("b".into());
        let b = p.resolve(&defaults(), |_| None);
        assert_eq!(b.max_tokens, 32_000);
        assert_eq!(b.context_window, 128_000);
        assert_eq!(b.compact_at, 0.85);
    }

    #[test]
    fn a_model_without_preferences_uses_the_listed_window() {
        let p = Preferences {
            model: Some("listed".into()),
            ..Default::default()
        };
        let resolved = p.resolve(&defaults(), |id| (id == "listed").then_some(64_000));
        assert_eq!(
            resolved,
            ModelParams {
                model: "listed".into(),
                max_tokens: 32_000,
                context_window: 64_000,
                compact_at: 0.85,
                reasoning_effort: None,
            }
        );
    }

    #[test]
    fn validate_rejects_bad_fields() {
        let mut p = valid();
        p.models.get_mut("m").unwrap().max_tokens = Some(0);
        let err = p.validate().unwrap_err();
        assert!(err.contains("m"));
        assert!(err.contains("max_tokens"));

        let mut p = valid();
        p.models.get_mut("m").unwrap().context_window = Some(1023);
        let err = p.validate().unwrap_err();
        assert!(err.contains("context_window"));

        let mut p = valid();
        p.models.get_mut("m").unwrap().compaction_threshold = Some(1.0001);
        let err = p.validate().unwrap_err();
        assert!(err.contains("compaction_threshold"));

        let mut p = valid();
        p.models.get_mut("m").unwrap().compaction_threshold = Some(f64::NAN);
        assert!(p.validate().is_err());

        let mut p = valid();
        p.model = Some(String::new());
        assert!(p.validate().unwrap_err().contains("model"));

        let mut p = valid();
        p.models.insert(String::new(), ModelPrefs::default());
        assert!(p.validate().unwrap_err().contains("model id"));

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
    fn preferences_round_trip_and_empty_json_loads() {
        let mut p = Preferences {
            model: Some("m".into()),
            ..Default::default()
        };
        p.models.insert(
            "m".into(),
            ModelPrefs {
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.25),
                reasoning_effort: Some(ReasoningEffort::XHigh),
            },
        );
        p.display.reasoning_open = false;
        p.display.font_size = 15;
        p.global_instructions = Some("Be terse.".into());
        p.sandbox = SandboxPrefs {
            enabled: false,
            network: true,
        };

        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"maxTokens\""));
        assert!(json.contains("\"reasoningEffort\""));
        assert!(json.contains("\"fontSize\""));
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
        let p: Preferences = serde_json::from_str(
            r#"{"models":{"m":{"maxTokens":100}},"display":{"reasoningOpen":true,"toolResultsOpen":false}}"#,
        )
        .unwrap();
        assert_eq!(p.display.font_size, DEFAULT_FONT_SIZE);
        assert_eq!(p.models.get("m").unwrap().reasoning_effort, None);
        assert_eq!(p.sandbox, SandboxPrefs::default());
        assert!(p.sandbox.enabled);
        assert!(!p.sandbox.network);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn a_preferences_json_without_panel_pages_loads_none() {
        let p: Preferences = serde_json::from_str(
            r#"{"display":{"reasoningOpen":true,"toolResultsOpen":false,"fontSize":14}}"#,
        )
        .unwrap();
        assert_eq!(p.display.panel_pages, None);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn valid_panel_pages_pass_and_round_trip() {
        let mut p = valid();
        p.display.panel_pages = Some(vec![PanelPage {
            name: "main".into(),
            blocks: vec!["file".into(), "terminal".into()],
        }]);
        assert!(p.validate().is_ok());
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"panelPages\""));
        assert_eq!(serde_json::from_str::<Preferences>(&json).unwrap(), p);
    }

    #[test]
    fn validate_rejects_an_empty_panel_page_name() {
        let mut p = valid();
        p.display.panel_pages = Some(vec![PanelPage {
            name: "   ".into(),
            blocks: vec!["block".into()],
        }]);
        assert!(p.validate().unwrap_err().contains("panel page name"));
    }

    #[test]
    fn validate_rejects_duplicate_panel_page_names() {
        let mut p = valid();
        p.display.panel_pages = Some(vec![
            PanelPage {
                name: "a".into(),
                blocks: vec!["x".into()],
            },
            PanelPage {
                name: "  a  ".into(),
                blocks: vec!["y".into()],
            },
        ]);
        assert!(
            p.validate()
                .unwrap_err()
                .contains("panel page names must be unique")
        );
    }

    #[test]
    fn validate_rejects_an_empty_panel_page_list() {
        let mut p = valid();
        p.display.panel_pages = Some(vec![]);
        assert!(p.validate().unwrap_err().contains("panel pages"));
    }

    #[test]
    fn validate_rejects_too_many_panel_pages() {
        let mut p = valid();
        p.display.panel_pages = Some(
            (0..=MAX_PANEL_PAGES)
                .map(|i| PanelPage {
                    name: format!("{i}"),
                    blocks: vec!["b".into()],
                })
                .collect(),
        );
        assert!(p.validate().unwrap_err().contains("panel pages"));
    }

    #[test]
    fn validate_rejects_a_long_panel_page_name() {
        let mut p = valid();
        p.display.panel_pages = Some(vec![PanelPage {
            name: "a".repeat(MAX_PANEL_PAGE_NAME + 1),
            blocks: vec!["block".into()],
        }]);
        assert!(p.validate().unwrap_err().contains("panel page name"));
    }

    #[test]
    fn validate_rejects_an_empty_panel_page_block_id() {
        let mut p = valid();
        p.display.panel_pages = Some(vec![PanelPage {
            name: "main".into(),
            blocks: vec!["  ".into()],
        }]);
        assert!(p.validate().unwrap_err().contains("panel page block id"));
    }
}
