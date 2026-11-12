//! Tells the framing agent that the human edited the task sheet since it
//! last saw it.
//!
//! The frozen prompt cannot be rewritten to preserve prompt caching, so a
//! changed sheet is announced with an appended user message.

use crate::domain::compaction::model_view;
use crate::domain::event::{Event, Role};

const NOTICE_PREFIX: &str = "<system-reminder>\nThe task sheet now reads:\n";

/// The appended message announcing the sheet's current contents.
pub fn notice(sheet_markdown: &str) -> String {
    format!(
        "{NOTICE_PREFIX}{}\n</system-reminder>",
        sheet_markdown.trim_end()
    )
}

/// The notice to append when the model's view does not carry `sheet_markdown`.
pub fn needed(events: &[Event], sheet_markdown: &str) -> Option<String> {
    let notice = notice(sheet_markdown);
    let view = model_view(events);
    let last_notice = view.iter().rev().find_map(|event| match event {
        Event::MessageStart {
            content: Some(content),
            is_system_generated: true,
            ..
        } if content.starts_with(NOTICE_PREFIX) => Some(content),
        _ => None,
    });
    if let Some(told) = last_notice {
        return (told != &notice).then_some(notice);
    }
    if let Some(Event::MessageStart { content, .. }) = view.iter().copied().find(|e| is_human(e)) {
        let carried = content
            .as_deref()
            .is_some_and(|content| content.contains(sheet_markdown.trim_end()));
        return (!carried).then_some(notice);
    }
    // No human message in the view: compacted or rewound away, or the first turn, whose
    // prompt carries the sheet.
    events.iter().any(is_human).then_some(notice)
}

fn is_human(event: &Event) -> bool {
    matches!(
        event,
        Event::MessageStart {
            role: Role::User,
            is_system_generated: false,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human(content: &str) -> Event {
        Event::MessageStart {
            message_id: "m".into(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: false,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    fn system(content: &str) -> Event {
        Event::MessageStart {
            message_id: "m".into(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: true,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn no_notice_when_the_first_message_carries_the_sheet() {
        let sheet = "# Plan\n- step";
        let events = vec![human(sheet)];
        assert_eq!(needed(&events, sheet), None);
    }

    #[test]
    fn a_changed_sheet_gets_a_notice() {
        let events = vec![human("# Plan\n- old")];
        let sheet = "# Plan\n- new";
        assert_eq!(needed(&events, sheet), Some(notice(sheet)));
    }

    #[test]
    fn the_same_sheet_twice_gets_one_notice() {
        let sheet = "# Plan\n- step";
        let events = vec![system(notice(sheet).as_str())];
        assert_eq!(needed(&events, sheet), None);

        let other = "# Plan\n- other";
        assert_eq!(needed(&events, other), Some(notice(other)));
    }

    #[test]
    fn a_compaction_resets_what_the_model_knows() {
        let sheet = "# Plan\n- step";
        let events = vec![
            human(sheet),
            Event::ContextCompacted {
                before_tokens: 100,
                after_tokens: 10,
                summary: "summary".into(),
            },
        ];
        assert_eq!(needed(&events, sheet), Some(notice(sheet)));
    }

    #[test]
    fn the_first_turn_needs_no_notice() {
        let events: Vec<Event> = Vec::new();
        assert_eq!(needed(&events, "# Plan"), None);
    }
}
