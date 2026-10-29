//! The model a session runs on, read from its log.

use super::event::Event;
use super::preferences::ModelRef;

/// The model of the last `model.changed`, `None` if the session never chose one.
pub fn chosen_model(events: &[Event]) -> Option<ModelRef> {
    events.iter().rev().find_map(|event| match event {
        Event::ModelChanged { model } => Some(model.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::preferences::LOCAL_PROVIDER;

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
}
