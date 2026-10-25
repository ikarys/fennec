//! Ports openfox `tasks/slash.ts` `parseSlashInvocation`; openfox resolves
//! workflows and commands (not ported yet); fennec adds `/compact` and skills.

use crate::domain::skill::Skill;

/// What a message asks for once its slash command is resolved.
#[derive(Debug, PartialEq)]
pub enum TurnInput {
    Prompt(String),
    Compact,
}

/// A command the composer offers: `/compact` and one per skill.
#[derive(Debug, PartialEq)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
}

/// openfox `parseSlashInvocation`: (id, rest of the text trimmed); None when
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

    if skills.iter().any(|skill| skill.id == id) {
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
    }];
    for skill in skills {
        out.push(SlashCommand {
            name: skill.id.clone(),
            description: skill.description.clone(),
        });
    }
    out
}

fn skill_request(id: &str, args: &str) -> String {
    let base = format!("Load the skill \"{id}\" with load_skill and follow its instructions.");
    if args.is_empty() {
        base
    } else {
        format!("{base}\n\n{args}")
    }
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
            directory: PathBuf::from(id),
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
        assert_eq!(list[1].name, "b");
        assert_eq!(list[2].name, "a");
    }
}
