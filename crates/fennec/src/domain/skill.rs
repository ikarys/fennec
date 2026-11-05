//! Skill definitions and `SKILL.md` parsing for portable and embedded
//! packages.
//!
//! It does not support legacy `.skill.md` files, selected directories,
//! enable/disable settings, name warnings, installer, HTTP routes/UI,
//! edit/delete.

use std::path::PathBuf;

use crate::domain::frontmatter::split;

/// `SkillDefinition` for portable and embedded packages.
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

/// The text `load_skill` returns for a skill.
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

/// The always-active skills block fennec puts in a new session's system
/// prompt: each skill's full body, so no `load_skill` call is needed.
pub fn always_active_section(skills: &[&Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut section = "\n## ALWAYS-ACTIVE SKILLS\n\nThese skills' instructions apply to every task in this session. They are already loaded: do not call load_skill for them.\n\n".to_string();
    for skill in skills {
        section.push_str(&format!(
            "<skill id=\"{}\">\n{}\n</skill>\n\n",
            skill.id,
            skill.prompt.trim()
        ));
    }

    format!("{}\n", section.trim_end_matches('\n'))
}

/// The skill ids an `always_active_section` wrote, in order. Only the
/// section's own header opens the scan, so ids elsewhere in the prompt are
/// ignored.
pub fn always_active_ids(prompt: &str) -> Vec<String> {
    const HEADER: &str = "\n## ALWAYS-ACTIVE SKILLS\n";
    let Some(start) = prompt.find(HEADER) else {
        return Vec::new();
    };
    let after = &prompt[start + HEADER.len()..];
    after
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("<skill id=\"")?;
            rest.find("\">").map(|end| rest[..end].to_string())
        })
        .collect()
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

    #[test]
    fn always_active_section_round_trips_ids() {
        let alpha = Skill {
            id: "alpha".into(),
            description: String::new(),
            prompt: "A\n".into(),
            directory: None,
        };
        let beta = Skill {
            id: "beta".into(),
            description: String::new(),
            prompt: " B \n".into(),
            directory: None,
        };
        let section = always_active_section(&[&alpha, &beta]);

        assert!(section.starts_with("\n## ALWAYS-ACTIVE SKILLS\n"));
        assert!(section.contains("<skill id=\"alpha\">\nA\n</skill>"));
        assert!(section.contains("<skill id=\"beta\">\nB\n</skill>"));
        assert!(section.ends_with("</skill>\n"));
        assert_eq!(always_active_ids(&section), vec!["alpha", "beta"]);
    }

    #[test]
    fn an_empty_always_active_section_has_no_ids() {
        assert_eq!(always_active_section(&[]), "");
        assert_eq!(always_active_ids(""), Vec::<String>::new());
    }

    #[test]
    fn a_prompt_without_the_header_has_no_always_active_ids() {
        let prompt = "<skill id=\"alpha\">\nA\n</skill>\n";
        assert_eq!(always_active_ids(prompt), Vec::<String>::new());
    }
}
