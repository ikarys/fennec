//! Port of openfox `skills/types.ts` and the `SKILL.md` parsing of
//! `skills/registry.ts` (gray-matter there), for portable and embedded
//! packages.
//!
//! Not ported: openfox's own config/`.openfox/skills`
//! dirs, legacy `.skill.md` files, selected directories, enable/disable
//! settings, name warnings, installer, HTTP routes/UI, edit/delete.

use std::path::PathBuf;

use crate::domain::frontmatter::split;

/// openfox `SkillDefinition`, portable and embedded packages.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub id: String,
    pub description: String,
    pub prompt: String,
    /// The package directory; None for a skill embedded in fennec (no files beside it).
    pub directory: Option<PathBuf>,
}

/// Parses a SKILL.md: (name, description, body), None when the frontmatter
/// is missing or one of them is empty.
pub fn parse_skill_md(content: &str) -> Option<(String, String, String)> {
    let (fields, body) = split(content)?;
    if body.is_empty() {
        return None;
    }

    let name = fields.scalar("name").filter(|value| !value.is_empty())?;
    let description = fields
        .scalar("description")
        .filter(|value| !value.is_empty())?;

    Some((name.to_string(), description.to_string(), body))
}

/// openfox `formatSkillPrompt` for a skill.
pub fn format_skill_prompt(skill: &Skill) -> String {
    match &skill.directory {
        Some(directory) => format!(
            "Skill package directory: {}\nResolve relative paths in these instructions from that directory.\n\n{}",
            directory.display(),
            skill.prompt
        ),
        None => skill.prompt.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_frontmatter() {
        let content = "---\nname: test\ndescription: desc\n---\nbody\n";
        assert_eq!(
            parse_skill_md(content),
            Some(("test".to_string(), "desc".to_string(), "body".to_string()))
        );
    }

    #[test]
    fn empty_body_is_none() {
        assert_eq!(
            parse_skill_md("---\nname: test\ndescription: desc\n---\n"),
            None
        );
    }

    #[test]
    fn format_skill_prompt_is_exact() {
        let skill = Skill {
            id: "test".to_string(),
            description: "desc".to_string(),
            prompt: "Run scripts/check.sh.".to_string(),
            directory: Some(PathBuf::from("/shared skills/test")),
        };
        assert_eq!(
            format_skill_prompt(&skill),
            "Skill package directory: /shared skills/test\nResolve relative paths in these instructions from that directory.\n\nRun scripts/check.sh."
        );
    }

    #[test]
    fn embedded_skill_formats_to_prompt_alone() {
        let skill = Skill {
            id: "test".to_string(),
            description: "desc".to_string(),
            prompt: "Run scripts/check.sh.".to_string(),
            directory: None,
        };
        assert_eq!(format_skill_prompt(&skill), "Run scripts/check.sh.");
    }
}
