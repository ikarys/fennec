//! A session's skills folded from its event log: those loaded so far
//! (successful `load_skill` calls, `/<skill>` messages), those frozen into
//! the session's system prompt as always-active, and those still in the
//! model's view (not dropped by a compaction, not cut by a replay).
//! fennec's own feature; openfox has no such fold.

use super::compaction::model_view;
use super::conversation::frozen_prompt;
use super::event::{Event, Role};
use super::skill::{Skill, always_active_ids};
use super::slash;
use std::collections::HashSet;

/// What a session has loaded, what is frozen into its prompt as always-on,
/// and what is still in the model's view.
/// `loaded` and `active` are deduplicated in first-seen order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSkills {
    pub loaded: Vec<String>,
    pub active: Vec<String>,
    /// Skill ids whose bodies the first `TurnSnapshot`'s system prompt carries.
    pub always_on: Vec<String>,
    /// Whether the system prompt is frozen yet (a first `TurnSnapshot`).
    pub started: bool,
}

/// The session's skills: `loaded` over the whole log, `active` over the
/// model's view of it, `always_on` from the first frozen system prompt.
/// `skills` is the project's registry: the caller does the discovery, this
/// fold is pure.
pub fn fold_skills(events: &[Event], skills: &[Skill]) -> SessionSkills {
    let loaded = loads_in(events.iter(), skills);
    let active = loads_in(model_view(events), skills);
    SessionSkills {
        loaded,
        active,
        always_on: always_on_in(events),
        started: frozen_prompt(events).0.is_some(),
    }
}

/// The skills the human chose always-on (Settings) that a started session
/// lacks: chosen after its prompt froze, so only a `/<skill>` brings them in.
/// Ids the project's registry does not know are left out: nothing to load.
pub fn missed_always_on(
    folded: &SessionSkills,
    chosen: &[String],
    skills: &[Skill],
) -> Vec<String> {
    if !folded.started {
        return Vec::new();
    }
    chosen
        .iter()
        .filter(|id| skills.iter().any(|skill| &skill.id == *id))
        .filter(|id| !folded.always_on.contains(id) && !folded.loaded.contains(id))
        .cloned()
        .collect()
}

/// The ids the first frozen system prompt carries as always-active; none
/// when the log has no `TurnSnapshot` yet.
fn always_on_in(events: &[Event]) -> Vec<String> {
    frozen_prompt(events)
        .0
        .map(always_active_ids)
        .unwrap_or_default()
}

/// The skill ids a slice of the log loaded, in first-seen order: successful
/// `load_skill` calls (a failed call loads nothing, as a failed write changes
/// no file) and user-typed `/<skill>` messages.
fn loads_in<'a, I: IntoIterator<Item = &'a Event>>(events: I, skills: &[Skill]) -> Vec<String> {
    let events: Vec<&'a Event> = events.into_iter().collect();
    let succeeded: HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::ToolResult {
                tool_call_id,
                result,
                ..
            } if result.success => Some(tool_call_id.as_str()),
            _ => None,
        })
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for event in &events {
        let id = match event {
            Event::ToolCall { tool_call, .. }
                if tool_call.name == "load_skill" && succeeded.contains(tool_call.id.as_str()) =>
            {
                tool_call
                    .arguments
                    .get("skillId")
                    .and_then(serde_json::Value::as_str)
                    .filter(|id| skills.iter().any(|skill| skill.id == *id))
            }
            Event::MessageStart {
                role: Role::User,
                is_system_generated: false,
                content: Some(content),
                ..
            } => slash::logged_skill_id(content, skills),
            _ => None,
        };
        if let Some(id) = id
            && seen.insert(id.to_string())
        {
            out.push(id.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::ToolCall;
    use serde_json::json;
    use std::path::PathBuf;

    fn skill(id: &str) -> Skill {
        Skill {
            id: id.into(),
            description: format!("{id} description"),
            prompt: "prompt".into(),
            directory: Some(PathBuf::from(id)),
        }
    }

    fn skills() -> Vec<Skill> {
        vec![skill("alpha"), skill("beta"), skill("gamma")]
    }

    fn user(id: &str, content: &str, system_generated: bool) -> Event {
        Event::MessageStart {
            message_id: id.into(),
            role: Role::User,
            content: Some(content.into()),
            is_system_generated: system_generated,
            metadata: None,
            attachments: Vec::new(),
        }
    }

    fn load_call(id: &str, skill_id: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: "load_skill".into(),
                arguments: json!({"skillId": skill_id}),
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    fn result(id: &str, success: bool) -> Event {
        Event::ToolResult {
            message_id: "m".into(),
            tool_call_id: id.into(),
            result: if success {
                crate::domain::event::ToolResult::success("ok".into(), false)
            } else {
                crate::domain::event::ToolResult::failure("nope".into())
            },
        }
    }

    fn compacted() -> Event {
        Event::ContextCompacted {
            before_tokens: 1,
            after_tokens: 2,
            summary: "s".into(),
        }
    }

    #[test]
    fn loaded_and_active_match_without_cuts() {
        // A typed `/<skill>` is logged in its resolved form, not as raw text.
        let events = vec![
            load_call("c1", "alpha"),
            result("c1", true),
            user(
                "u1",
                "Load the skill \"gamma\" with load_skill and follow its instructions.\n\nfix it",
                false,
            ),
            load_call("c2", "beta"),
            result("c2", true),
        ];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, vec!["alpha", "gamma", "beta"]);
        assert_eq!(folded.active, vec!["alpha", "gamma", "beta"]);
    }

    #[test]
    fn a_compaction_drops_earlier_loads_from_active() {
        let events = vec![
            load_call("c1", "alpha"),
            result("c1", true),
            compacted(),
            load_call("c2", "beta"),
            result("c2", true),
        ];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, vec!["alpha", "beta"]);
        assert_eq!(folded.active, vec!["beta"]);
    }

    #[test]
    fn a_replay_cuts_later_loads_from_active() {
        let events = vec![
            user("u1", "first", false),
            load_call("c1", "alpha"),
            result("c1", true),
            user("u2", "second", false),
            load_call("c2", "beta"),
            result("c2", true),
            Event::SessionRewound {
                message_id: "u2".into(),
            },
            user("u3", "after", false),
        ];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, vec!["alpha", "beta"]);
        assert_eq!(folded.active, vec!["alpha"]);
    }

    #[test]
    fn a_failed_load_is_not_loaded() {
        let events = vec![load_call("c1", "alpha"), result("c1", false)];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, Vec::<String>::new());
        assert_eq!(folded.active, Vec::<String>::new());
    }

    #[test]
    fn an_in_flight_load_is_not_loaded_yet() {
        let events = vec![load_call("c1", "alpha")];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, Vec::<String>::new());
        assert_eq!(folded.active, Vec::<String>::new());
    }

    #[test]
    fn an_unknown_skill_is_not_loaded() {
        let events = vec![
            load_call("c1", "delta"),
            result("c1", true),
            user("u1", "/delta now", false),
        ];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, Vec::<String>::new());
        assert_eq!(folded.active, Vec::<String>::new());
    }

    #[test]
    fn compact_and_system_messages_do_not_load() {
        let events = vec![user("u1", "/compact", false), user("u2", "/alpha go", true)];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, Vec::<String>::new());
        assert_eq!(folded.active, Vec::<String>::new());
    }

    #[test]
    fn a_skill_named_compact_is_loaded_by_a_call_not_by_the_command() {
        let skills = vec![skill("compact")];
        let events = vec![
            user("u1", "/compact", false),
            load_call("c1", "compact"),
            result("c1", true),
        ];
        let folded = fold_skills(&events, &skills);
        assert_eq!(folded.loaded, vec!["compact"]);
        assert_eq!(folded.active, vec!["compact"]);
    }

    #[test]
    fn duplicates_keep_the_first_seen_order() {
        let events = vec![
            load_call("c1", "beta"),
            result("c1", true),
            load_call("c2", "alpha"),
            result("c2", true),
            load_call("c3", "beta"),
            result("c3", true),
            user(
                "u1",
                "Load the skill \"beta\" with load_skill and follow its instructions.\n\nagain",
                false,
            ),
        ];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.loaded, vec!["beta", "alpha"]);
        assert_eq!(folded.active, vec!["beta", "alpha"]);
    }

    #[test]
    fn a_turn_snapshot_freezes_always_on() {
        let prompt = "\n## ALWAYS-ACTIVE SKILLS\n\nThese skills' instructions apply to every task in this session. They are already loaded: do not call load_skill for them.\n\n<skill id=\"alpha\">\nA\n</skill>\n";
        let events = vec![
            Event::TurnSnapshot {
                cached_system_prompt: prompt.into(),
                cached_tools: None,
            },
            load_call("c1", "beta"),
            result("c1", true),
        ];
        let folded = fold_skills(&events, &skills());
        assert_eq!(folded.always_on, vec!["alpha"]);
        assert_eq!(folded.loaded, vec!["beta"]);
        assert_eq!(folded.active, vec!["beta"]);
    }
}
