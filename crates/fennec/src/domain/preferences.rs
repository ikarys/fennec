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
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelPrefs {
    pub max_tokens: Option<u32>,
    /// openfox `contextWindow`; overrides what the endpoint lists.
    pub context_window: Option<u64>,
    /// openfox `compactionThreshold`; 0 disables compaction.
    pub compaction_threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Display {
    pub reasoning_open: bool,
    pub tool_results_open: bool,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            reasoning_open: true,
            tool_results_open: false,
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
            }
        );
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
            },
        );
        p.display.reasoning_open = false;

        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"maxTokens\""));
        assert_eq!(serde_json::from_str::<Preferences>(&json).unwrap(), p);
        assert_eq!(
            serde_json::from_str::<Preferences>("{}").unwrap(),
            Preferences::default()
        );
    }
}
