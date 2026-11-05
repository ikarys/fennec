//! Tells the model that the session's working directory moved after the
//! system prompt was frozen.
//!
//! The frozen prompt cannot be rewritten to preserve prompt caching, so a
//! workflow's worktree move is announced with an appended user message.

use crate::domain::compaction::model_view;
use crate::domain::conversation::frozen_prompt;
use crate::domain::event::Event;
use std::path::Path;

const NOTICE_PREFIX: &str = "<system-reminder>\nYour working directory is now ";

/// The working directory the model was last told: the last notice in its
/// view, else the frozen prompt's `Working directory:` line.
pub fn stated_workdir(events: &[Event]) -> Option<String> {
    for event in model_view(events).iter().rev() {
        if let Event::MessageStart {
            content: Some(content),
            is_system_generated: true,
            ..
        } = event
            && let Some(rest) = content.strip_prefix(NOTICE_PREFIX)
            && let Some(line) = rest.lines().next()
        {
            return Some(line.strip_suffix('.').unwrap_or(line).to_string());
        }
    }

    frozen_prompt(events).0.and_then(|prompt| {
        prompt
            .lines()
            .find_map(|line| line.strip_prefix("Working directory: ").map(str::to_string))
    })
}

/// The appended message announcing the move from `old` to `new`.
pub fn notice(old: &str, new: &Path) -> String {
    format!(
        "<system-reminder>\nYour working directory is now {}.\nIt was {}: that folder is the human's own checkout. Read, write and run commands in your working directory only, with paths relative to it or under it; paths under {} are redirected to it.\n</system-reminder>",
        new.display(),
        old,
        old
    )
}

/// The notice to append when the model's stated workdir is not `workdir`.
pub fn needed(events: &[Event], workdir: &Path) -> Option<String> {
    stated_workdir(events).and_then(|old| {
        if Path::new(old.as_str()) != workdir {
            Some(notice(&old, workdir))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::Role;

    fn snapshot(workdir: &str) -> Event {
        Event::TurnSnapshot {
            cached_system_prompt: format!("System\nWorking directory: {workdir}\nPlatform: x"),
            cached_tools: None,
            mcp_tool_names: Vec::new(),
        }
    }

    #[test]
    fn no_snapshot_needs_no_notice() {
        let events: Vec<Event> = Vec::new();
        assert_eq!(needed(&events, Path::new("/p")), None);
    }

    #[test]
    fn same_workdir_needs_no_notice() {
        let events = vec![snapshot("/p")];
        assert_eq!(needed(&events, Path::new("/p")), None);
    }

    #[test]
    fn other_workdir_needs_notice() {
        let events = vec![snapshot("/p")];
        let notice = needed(&events, Path::new("/w")).expect("notice");
        assert!(notice.contains("/p"));
        assert!(notice.contains("/w"));
    }

    #[test]
    fn stated_notice_satisfies_new_workdir() {
        let events = vec![
            snapshot("/p"),
            Event::MessageStart {
                message_id: "m".into(),
                role: Role::User,
                content: Some(notice("/p", Path::new("/w"))),
                is_system_generated: true,
                metadata: None,
                attachments: Vec::new(),
            },
        ];
        assert_eq!(needed(&events, Path::new("/w")), None);
    }
}
