//! What a model's turn will run with, display-ready for the settings page.
//!
//! Mirrors the precedence of `llm_providers::ModelParams::resolve`
//! (per-model > listed window > defaults) but annotates each value with its
//! source and whether it goes on the wire (see `llm_providers::chat::body`).
//! The UI shows these; it computes none.

use llm_providers::{ModelParams, ModelPrefs, ReasoningEffort};
use serde::Serialize;

/// Where a resolved value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// The per-model preference set on the settings page.
    PerModel,
    /// The endpoint's current listing (`max_model_len`).
    Listed,
    /// The environment/built-in default.
    Default,
}

/// One resolved parameter: its value, where it came from, and whether a call
/// sends it to the provider (`wire`) or only uses it locally.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Value<T: Serialize> {
    pub value: T,
    pub source: Source,
    pub wire: bool,
}

/// The resolved `reasoning_effort`; `value` null means nothing is sent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Effort {
    pub value: Option<ReasoningEffort>,
    pub source: Source,
    pub wire: bool,
}

/// The concrete parameters one turn of a model runs with, with provenance.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    /// Always sent on the wire.
    pub max_tokens: Value<u32>,
    /// Used locally for compaction; never sent.
    pub context_window: Value<u64>,
    /// The local compaction threshold; never sent.
    pub compact_at: Value<f64>,
    /// Sent on the wire only when set.
    pub reasoning_effort: Effort,
}

/// Resolves a model's parameters the way a turn would, annotating source and
/// wire. `listed` is the endpoint's `max_model_len` for the model (if any).
pub fn resolved(
    defaults: &ModelParams,
    prefs: Option<&ModelPrefs>,
    listed: Option<u64>,
) -> Resolved {
    let fallback = ModelPrefs::default();
    let prefs = prefs.unwrap_or(&fallback);
    Resolved {
        max_tokens: pick(prefs.max_tokens, defaults.max_tokens, true),
        context_window: match (prefs.context_window, listed) {
            (Some(window), _) => Value {
                value: window,
                source: Source::PerModel,
                wire: false,
            },
            (None, Some(window)) => Value {
                value: window,
                source: Source::Listed,
                wire: false,
            },
            (None, None) => Value {
                value: defaults.context_window,
                source: Source::Default,
                wire: false,
            },
        },
        compact_at: pick(prefs.compaction_threshold, defaults.compact_at, false),
        reasoning_effort: {
            let (value, source) = match (prefs.reasoning_effort, defaults.reasoning_effort) {
                (Some(effort), _) => (Some(effort), Source::PerModel),
                (None, Some(effort)) => (Some(effort), Source::Default),
                (None, None) => (None, Source::Default),
            };
            Effort {
                value,
                source,
                wire: value.is_some(),
            }
        },
    }
}

/// Per-model value if set, else the default.
fn pick<T: Copy + Serialize>(per_model: Option<T>, default: T, wire: bool) -> Value<T> {
    match per_model {
        Some(value) => Value {
            value,
            source: Source::PerModel,
            wire,
        },
        None => Value {
            value: default,
            source: Source::Default,
            wire,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_providers::ModelParams;

    fn defaults() -> ModelParams {
        ModelParams {
            provider: "local".into(),
            model: "m".into(),
            max_tokens: 8_000,
            context_window: 32_000,
            compact_at: 0.85,
            reasoning_effort: None,
        }
    }

    #[test]
    fn no_prefs_falls_back_to_defaults() {
        let r = resolved(&defaults(), None, None);
        assert_eq!(r.max_tokens.value, 8_000);
        assert_eq!(r.max_tokens.source, Source::Default);
        assert!(r.max_tokens.wire);
        assert_eq!(r.context_window.value, 32_000);
        assert_eq!(r.context_window.source, Source::Default);
        assert!(!r.context_window.wire);
        assert_eq!(r.compact_at.value, 0.85);
        assert!(!r.compact_at.wire);
        assert_eq!(r.reasoning_effort.value, None);
        assert!(!r.reasoning_effort.wire);
    }

    #[test]
    fn listed_window_wins_over_default_but_not_per_model() {
        let none = resolved(&defaults(), None, Some(131_072));
        assert_eq!(none.context_window.value, 131_072);
        assert_eq!(none.context_window.source, Source::Listed);

        let per_model = ModelPrefs {
            context_window: Some(64_000),
            ..Default::default()
        };
        let r = resolved(&defaults(), Some(&per_model), Some(131_072));
        assert_eq!(r.context_window.value, 64_000);
        assert_eq!(r.context_window.source, Source::PerModel);
    }

    #[test]
    fn per_model_overrides_and_effort_is_sent_when_set() {
        let d = defaults();
        let per_model = ModelPrefs {
            max_tokens: Some(16_000),
            compaction_threshold: Some(0.5),
            reasoning_effort: Some(ReasoningEffort::High),
            ..Default::default()
        };
        let r = resolved(&d, Some(&per_model), None);
        assert_eq!(r.max_tokens.value, 16_000);
        assert_eq!(r.max_tokens.source, Source::PerModel);
        assert_eq!(r.compact_at.value, 0.5);
        assert_eq!(r.reasoning_effort.value, Some(ReasoningEffort::High));
        assert_eq!(r.reasoning_effort.source, Source::PerModel);
        assert!(r.reasoning_effort.wire);
    }

    #[test]
    fn matches_model_params_resolve() {
        // The provenance must not drift from what a turn actually runs with.
        let d = defaults();
        let per_model = ModelPrefs {
            price: None,
            max_tokens: Some(4_000),
            context_window: Some(12_000),
            compaction_threshold: Some(0.7),
            reasoning_effort: Some(ReasoningEffort::Low),
        };
        let concrete = ModelParams::resolve(
            &d,
            llm_providers::ModelRef {
                provider: "local".into(),
                model: "m".into(),
            },
            Some(&per_model),
            Some(131_072),
        );
        let r = resolved(&d, Some(&per_model), Some(131_072));
        assert_eq!(r.max_tokens.value, concrete.max_tokens);
        assert_eq!(r.context_window.value, concrete.context_window);
        assert_eq!(r.compact_at.value, concrete.compact_at);
        assert_eq!(r.reasoning_effort.value, concrete.reasoning_effort);

        let concrete = ModelParams::resolve(
            &d,
            llm_providers::ModelRef {
                provider: "local".into(),
                model: "m".into(),
            },
            None,
            Some(131_072),
        );
        let r = resolved(&d, None, Some(131_072));
        assert_eq!(r.context_window.value, concrete.context_window);
    }
}
