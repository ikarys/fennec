//! Port of openfox `skills/types.ts` and the `SKILL.md` parsing of
//! `skills/registry.ts` (gray-matter there), for portable packages.
//!
//! Not ported: bundled defaults, openfox's own config/`.openfox/skills`
//! dirs, legacy `.skill.md` files, selected directories, enable/disable
//! settings, name warnings, installer, HTTP routes/UI, edit/delete.

use std::path::PathBuf;

/// openfox `SkillDefinition`, portable packages only.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub id: String,
    pub description: String,
    pub prompt: String,
    pub directory: PathBuf,
}

/// Parses a SKILL.md: (name, description, body), None when the frontmatter
/// is missing or one of them is empty.
pub fn parse_skill_md(content: &str) -> Option<(String, String, String)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let lines: Vec<&str> = content.lines().collect();
    let first = lines.first()?;
    if *first != "---" {
        return None;
    }

    let end = lines[1..].iter().position(|line| *line == "---")?;
    let end_index = end + 1;
    let body = lines[end_index + 1..].join("\n");
    let body = body.trim();
    if body.is_empty() {
        return None;
    }

    let frontmatter = &lines[1..end_index];
    let mut name = String::new();
    let mut description = String::new();
    let mut index = 0;

    while index < frontmatter.len() {
        let line = frontmatter[index];
        if line.is_empty() || line.starts_with(' ') {
            index += 1;
            continue;
        }

        let Some(colon) = line.find(':') else {
            index += 1;
            continue;
        };

        let key = line[..colon].trim();
        let value = line[colon + 1..].trim();
        match key {
            "name" => name = parse_value(value, frontmatter, &mut index),
            "description" => description = parse_value(value, frontmatter, &mut index),
            _ => {}
        }
        index += 1;
    }

    if name.is_empty() || description.is_empty() {
        return None;
    }

    Some((name, description, body.to_string()))
}

/// openfox `formatSkillPrompt` for a portable skill.
pub fn format_skill_prompt(skill: &Skill) -> String {
    format!(
        "Skill package directory: {}\nResolve relative paths in these instructions from that directory.\n\n{}",
        skill.directory.display(),
        skill.prompt
    )
}

// ponytail: this is a YAML subset, a real parser if skills in the wild need more.
fn parse_value(value: &str, lines: &[&str], index: &mut usize) -> String {
    match value {
        ">" | ">-" | ">+" | "|" | "|-" | "|+" => {
            let mut block = Vec::new();
            let mut next = *index + 1;
            while next < lines.len() && (lines[next].is_empty() || lines[next].starts_with(' ')) {
                block.push(lines[next]);
                next += 1;
            }
            *index = next - 1;
            parse_block_scalar(value, &block)
        }
        _ => parse_scalar(value),
    }
}

fn parse_scalar(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value[1..value.len() - 1].replace("''", "'");
    }
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return unescape_double(&value[1..value.len() - 1]);
    }
    value.to_string()
}

fn unescape_double(inner: &str) -> String {
    let mut out = String::new();
    let mut chars = inner.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.peek() {
                match next {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    _ => {
                        out.push('\\');
                        out.push(*next);
                    }
                }
                chars.next();
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }

    out
}

fn parse_block_scalar(indicator: &str, lines: &[&str]) -> String {
    let folded = indicator.starts_with('>');
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    let mut stripped = Vec::with_capacity(lines.len());
    for line in lines {
        if line.trim().is_empty() {
            stripped.push(String::new());
        } else {
            stripped.push(line.get(indent..).unwrap_or(line).to_string());
        }
    }

    if folded {
        stripped
            .into_iter()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        stripped.join("\n")
    }
    .trim()
    .to_string()
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
    fn parses_quoted_values() {
        let content = "---\nname: \"test\"\ndescription: 'it''s'\n---\nbody\n";
        assert_eq!(
            parse_skill_md(content),
            Some(("test".to_string(), "it's".to_string(), "body".to_string()))
        );

        let content = "---\nname: test\ndescription: \"a \\\" b \\\\ c\"\n---\nbody\n";
        assert_eq!(
            parse_skill_md(content),
            Some((
                "test".to_string(),
                "a \" b \\ c".to_string(),
                "body".to_string()
            ))
        );
    }

    #[test]
    fn parses_folded_description() {
        let content = "---\nname: test\ndescription: >\n  line one\n  line two\n---\nbody\n";
        assert_eq!(
            parse_skill_md(content),
            Some((
                "test".to_string(),
                "line one line two".to_string(),
                "body".to_string()
            ))
        );
    }

    #[test]
    fn parses_literal_description() {
        let content = "---\nname: test\ndescription: |\n  line one\n\n  line two\n---\nbody\n";
        assert_eq!(
            parse_skill_md(content),
            Some((
                "test".to_string(),
                "line one\n\nline two".to_string(),
                "body".to_string()
            ))
        );
    }

    #[test]
    fn ignores_metadata_block() {
        let content = "---\nname: test\ndescription: desc\nmetadata:\n  foo: bar\n---\nbody\n";
        assert_eq!(
            parse_skill_md(content),
            Some(("test".to_string(), "desc".to_string(), "body".to_string()))
        );
    }

    #[test]
    fn missing_frontmatter_is_none() {
        assert_eq!(parse_skill_md("name: test\ndescription: desc\nbody"), None);
    }

    #[test]
    fn empty_body_is_none() {
        assert_eq!(
            parse_skill_md("---\nname: test\ndescription: desc\n---\n"),
            None
        );
    }

    #[test]
    fn parses_crlf_content() {
        let content = "---\r\nname: test\r\ndescription: desc\r\n---\r\nbody\r\n";
        assert_eq!(
            parse_skill_md(content),
            Some(("test".to_string(), "desc".to_string(), "body".to_string()))
        );
    }

    #[test]
    fn format_skill_prompt_is_exact() {
        let skill = Skill {
            id: "test".to_string(),
            description: "desc".to_string(),
            prompt: "Run scripts/check.sh.".to_string(),
            directory: PathBuf::from("/shared skills/test"),
        };
        assert_eq!(
            format_skill_prompt(&skill),
            "Skill package directory: /shared skills/test\nResolve relative paths in these instructions from that directory.\n\nRun scripts/check.sh."
        );
    }
}
