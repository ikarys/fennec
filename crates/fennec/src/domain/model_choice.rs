//! The model a session runs on and the models it has used, read from its log.

use super::event::Event;
use super::preferences::ModelRef;

/// The model of the last `model.changed`, `None` if the session never chose one.
pub fn chosen_model(events: &[Event]) -> Option<ModelRef> {
    events.iter().rev().find_map(|event| match event {
        Event::ModelChanged { model } => Some(model.clone()),
        _ => None,
    })
}

/// The distinct models that answered in the session, in order of first use.
///
/// `None` stats and blank model ids are skipped so only real answers count.
pub fn models_used(events: &[Event]) -> Vec<String> {
    let mut models: Vec<String> = Vec::new();

    for event in events {
        if let Event::MessageDone {
            stats: Some(stats), ..
        } = event
        {
            // Trim so whitespace-only ids are dropped and near-duplicates collapse.
            let model = stats.model.trim();
            if !model.is_empty() && !models.iter().any(|m| m == model) {
                models.push(model.to_string());
            }
        }
    }

    models
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preferences::LOCAL_PROVIDER;
    use crate::domain::stats::MessageStats;

    fn stats(model: &str) -> MessageStats {
        MessageStats {
            model: model.to_string(),
            total_time: 0.0,
            prefill_tokens: 0,
            cached_prompt_tokens: 0,
            generation_tokens: 0,
            prefill_speed: 0.0,
            generation_speed: 0.0,
        }
    }

    #[test]
    fn no_choice_is_none() {
        assert_eq!(chosen_model(&[]), None);
    }

    #[test]
    fn the_last_model_changed_wins() {
        let events = vec![
            Event::ModeChanged {
                mode: "a".into(),
                auto: false,
                reason: None,
            },
            Event::ModelChanged {
                model: ModelRef {
                    provider: LOCAL_PROVIDER.into(),
                    model: "first".into(),
                },
            },
            Event::ModelChanged {
                model: ModelRef {
                    provider: LOCAL_PROVIDER.into(),
                    model: "second".into(),
                },
            },
        ];
        assert_eq!(
            chosen_model(&events),
            Some(ModelRef {
                provider: LOCAL_PROVIDER.into(),
                model: "second".into()
            })
        );
    }

    #[test]
    fn events_after_the_choice_do_not_hide_it() {
        let events = vec![
            Event::ModelChanged {
                model: ModelRef {
                    provider: LOCAL_PROVIDER.into(),
                    model: "m".into(),
                },
            },
            Event::ModeChanged {
                mode: "b".into(),
                auto: false,
                reason: None,
            },
        ];
        assert_eq!(
            chosen_model(&events),
            Some(ModelRef {
                provider: LOCAL_PROVIDER.into(),
                model: "m".into()
            })
        );
    }

    #[test]
    fn no_answer_is_no_models() {
        assert!(models_used(&[]).is_empty());
    }

    #[test]
    fn one_model_repeated_is_reported_once() {
        let events = vec![
            Event::MessageDone {
                message_id: "m1".into(),
                stats: Some(stats("qwen")),
                partial: false,
            },
            Event::MessageDone {
                message_id: "m2".into(),
                stats: Some(stats("qwen")),
                partial: false,
            },
        ];
        assert_eq!(models_used(&events), vec!["qwen".to_string()]);
    }

    #[test]
    fn a_model_switch_is_reported_in_order() {
        let events = vec![
            Event::MessageDone {
                message_id: "m1".into(),
                stats: Some(stats("qwen")),
                partial: false,
            },
            Event::ModelChanged {
                model: ModelRef {
                    provider: LOCAL_PROVIDER.into(),
                    model: "llama".into(),
                },
            },
            Event::MessageDone {
                message_id: "m2".into(),
                stats: Some(stats("llama")),
                partial: false,
            },
        ];
        assert_eq!(
            models_used(&events),
            vec!["qwen".to_string(), "llama".to_string()]
        );
    }

    #[test]
    fn blank_and_none_stats_are_skipped() {
        let events = vec![
            Event::MessageDone {
                message_id: "m1".into(),
                stats: Some(stats("   ")),
                partial: false,
            },
            Event::MessageDone {
                message_id: "m2".into(),
                stats: None,
                partial: false,
            },
            Event::MessageDone {
                message_id: "m3".into(),
                stats: Some(stats("qwen")),
                partial: false,
            },
        ];
        assert_eq!(models_used(&events), vec!["qwen".to_string()]);
    }
}
