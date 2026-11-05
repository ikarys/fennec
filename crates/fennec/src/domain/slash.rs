//! What a typed `/compact` or `/<skill>` stands for. Workflows and commands are not resolved here.
//! It adds `/compact` and skills.

use crate::domain::skill::Skill;

/// What a message asks for once its slash command is resolved.
#[derive(Debug, PartialEq)]
pub enum TurnInput {
    Prompt(String),
    Compact,
}

/// What a slash command runs: a built-in of fennec or a skill.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SlashKind {
    Command,
    Skill,
}

impl SlashKind {
    pub fn wire(self) -> &'static str {
        match self {
            SlashKind::Command => "command",
            SlashKind::Skill => "skill",
        }
    }
}

/// A command the composer offers: `/compact` and one per skill.
#[derive(Debug, PartialEq)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub kind: SlashKind,
}

/// Parses a slash invocation: (id, rest of the text trimmed); None when
/// the text does not start with `/` or the id is empty. The id ends at the
/// first whitespace.
pub fn parse(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('/')?;
    let (id, args) = match rest.split_once(char::is_whitespace) {
        Some((id, args)) => (id, args.trim()),
        None => (rest, ""),
    };
    if id.is_empty() {
        return None;
    }
    Some((id, args))
}

/// The skill id a typed message asks to load, or None: `/<id>` with id a
/// skill. The built-in `compact` wins over a skill named `compact`.
pub fn skill_request_id<'a>(text: &'a str, skills: &[Skill]) -> Option<&'a str> {
    let (id, _) = parse(text)?;
    (id != "compact" && skills.iter().any(|skill| skill.id == id)).then_some(id)
}

/// The message a text stands for: `/compact` alone → Compact; `/<id> args`
/// with id a skill id → Prompt(skill_request(id, args)); anything else
/// (unknown id, `/compact extra`, a path like `/etc/hosts`) → Prompt(text)
/// unchanged.
pub fn resolve(text: &str, skills: &[Skill]) -> TurnInput {
    let Some((id, args)) = parse(text) else {
        return TurnInput::Prompt(text.to_string());
    };

    // The built-in wins over a skill named `compact`: the human's command
    // must stay reachable.
    if id == "compact" {
        return if args.is_empty() {
            TurnInput::Compact
        } else {
            TurnInput::Prompt(text.to_string())
        };
    }

    if skill_request_id(text, skills).is_some() {
        return TurnInput::Prompt(skill_request(id, args));
    }

    TurnInput::Prompt(text.to_string())
}

/// The commands to offer: compact first, then the skills in their order.
pub fn commands(skills: &[Skill]) -> Vec<SlashCommand> {
    let mut out = vec![SlashCommand {
        name: "compact".to_string(),
        description:
            "Compact the context: summarize the session and start a new window from the summary."
                .to_string(),
        kind: SlashKind::Command,
    }];
    for skill in skills {
        out.push(SlashCommand {
            name: skill.id.clone(),
            description: skill.description.clone(),
            kind: SlashKind::Skill,
        });
    }
    out
}

/// The prefix every resolved skill request starts with; the single source of
/// the format `skill_request` builds and `logged_skill_id` matches.
const SKILL_REQUEST_PREFIX: &str = "Load the skill \"";

fn skill_request(id: &str, args: &str) -> String {
    let base = format!("{SKILL_REQUEST_PREFIX}{id}\" with load_skill and follow its instructions.");
    if args.is_empty() {
        base
    } else {
        format!("{base}\n\n{args}")
    }
}

/// The skill id a logged skill-request message asks for, or None. A typed
/// `/<id>` is resolved to `skill_request(id, args)` before it is logged, so
/// the log never holds the raw `/<id>`; this matches the resolved form. The
/// id ends at the next `"`.
pub fn logged_skill_id<'a>(content: &'a str, skills: &[Skill]) -> Option<&'a str> {
    let rest = content.strip_prefix(SKILL_REQUEST_PREFIX)?;
    let id = rest.split('"').next()?;
    (skills.iter().any(|skill| skill.id == id)).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn skill(id: &str) -> Skill {
        Skill {
            id: id.into(),
            description: format!("{id} description"),
            prompt: "prompt".into(),
            directory: Some(PathBuf::from(id)),
        }
    }

    #[test]
    fn parse_plain_text_is_none() {
        assert_eq!(parse("hello"), None);
    }

    #[test]
    fn parse_bare_slash_is_none() {
        assert_eq!(parse("/"), None);
    }

    #[test]
    fn parse_id_only() {
        assert_eq!(parse("/x"), Some(("x", "")));
    }

    #[test]
    fn parse_trims_args() {
        assert_eq!(parse("/x  a b"), Some(("x", "a b")));
    }

    #[test]
    fn parse_leading_spaces_is_none() {
        assert_eq!(parse("  /x"), None);
    }

    #[test]
    fn resolve_compact() {
        assert_eq!(resolve("/compact", &[]), TurnInput::Compact);
    }

    #[test]
    fn resolve_compact_with_extra_text_stays_a_prompt() {
        assert_eq!(
            resolve("/compact extra", &[]),
            TurnInput::Prompt("/compact extra".into())
        );
    }

    #[test]
    fn resolve_known_skill_without_args() {
        let skills = vec![skill("alpha")];
        assert_eq!(
            resolve("/alpha", &skills),
            TurnInput::Prompt(
                "Load the skill \"alpha\" with load_skill and follow its instructions.".to_string()
            )
        );
    }

    #[test]
    fn resolve_known_skill_with_args() {
        let skills = vec![skill("alpha")];
        assert_eq!(
            resolve("/alpha fix it", &skills),
            TurnInput::Prompt(
                "Load the skill \"alpha\" with load_skill and follow its instructions.\n\nfix it"
                    .to_string()
            )
        );
    }

    #[test]
    fn resolve_unknown_id_stays_a_prompt() {
        let skills = vec![skill("alpha")];
        assert_eq!(resolve("/beta", &skills), TurnInput::Prompt("/beta".into()));
    }

    #[test]
    fn resolve_path_stays_a_prompt() {
        assert_eq!(
            resolve("/etc/hosts", &[]),
            TurnInput::Prompt("/etc/hosts".into())
        );
    }

    #[test]
    fn resolve_skill_named_compact_does_not_shadow_the_builtin() {
        let skills = vec![skill("compact")];
        assert_eq!(resolve("/compact", &skills), TurnInput::Compact);
        assert_eq!(
            resolve("/compact extra", &skills),
            TurnInput::Prompt("/compact extra".into())
        );
    }

    #[test]
    fn commands_are_compact_then_skills_in_order() {
        let skills = vec![skill("b"), skill("a")];
        let list = commands(&skills);
        assert_eq!(list[0].name, "compact");
        assert_eq!(list[0].kind, SlashKind::Command);
        assert_eq!(list[1].name, "b");
        assert_eq!(list[1].kind, SlashKind::Skill);
        assert_eq!(list[2].name, "a");
    }

    #[test]
    fn skill_request_id_is_the_known_skill_of_a_slash_message() {
        let skills = vec![skill("alpha")];
        assert_eq!(skill_request_id("/alpha", &skills), Some("alpha"));
        assert_eq!(skill_request_id("/alpha fix it", &skills), Some("alpha"));
    }

    #[test]
    fn skill_request_id_is_none_for_unknown_ids_and_plain_text() {
        let skills = vec![skill("alpha")];
        assert_eq!(skill_request_id("/beta", &skills), None);
        assert_eq!(skill_request_id("hello", &skills), None);
    }

    #[test]
    fn skill_request_id_is_none_for_the_builtin_compact() {
        let skills = vec![skill("alpha"), skill("compact")];
        assert_eq!(skill_request_id("/compact", &skills), None);
    }

    #[test]
    fn logged_skill_id_matches_the_resolved_form() {
        let skills = vec![skill("alpha")];
        assert_eq!(
            logged_skill_id(
                "Load the skill \"alpha\" with load_skill and follow its instructions.",
                &skills
            ),
            Some("alpha")
        );
        assert_eq!(
            logged_skill_id(
                "Load the skill \"alpha\" with load_skill and follow its instructions.\n\nfix it",
                &skills
            ),
            Some("alpha")
        );
    }

    #[test]
    fn logged_skill_id_is_none_for_raw_and_plain_text() {
        let skills = vec![skill("alpha")];
        assert_eq!(logged_skill_id("/alpha", &skills), None);
        assert_eq!(logged_skill_id("/alpha fix it", &skills), None);
        assert_eq!(logged_skill_id("hello", &skills), None);
    }

    #[test]
    fn logged_skill_id_is_none_for_an_unknown_id() {
        let skills = vec![skill("alpha")];
        assert_eq!(
            logged_skill_id(
                "Load the skill \"beta\" with load_skill and follow its instructions.",
                &skills
            ),
            None
        );
    }
}
