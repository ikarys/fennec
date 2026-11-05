//! Shared frontmatter parser for `*.agent.md` and `SKILL.md` files.
//!
//! It does not parse full YAML (anchors, maps, nested lists, multi-document)
//! or gray-matter options.

/// A frontmatter value: fennec reads scalars and lists of scalars only.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Scalar(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frontmatter {
    fields: Vec<(String, Value)>,
}

impl Frontmatter {
    /// The scalar under `key`; None when absent or a list.
    pub fn scalar(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .and_then(|(_, value)| match value {
                Value::Scalar(value) => Some(value.as_str()),
                Value::List(_) => None,
            })
    }

    /// The list under `key`; None when absent or a scalar.
    pub fn list(&self, key: &str) -> Option<&[String]> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .and_then(|(_, value)| match value {
                Value::List(value) => Some(value.as_slice()),
                Value::Scalar(_) => None,
            })
    }
}

/// Splits a document into its frontmatter and its trimmed body; None when the
/// document does not start with a `---` line or the block is not closed.
pub fn split(content: &str) -> Option<(Frontmatter, String)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let lines: Vec<&str> = content.lines().collect();
    let first = lines.first()?;
    if *first != "---" {
        return None;
    }

    let end = lines[1..].iter().position(|line| *line == "---")?;
    let end_index = end + 1;
    let body = lines[end_index + 1..].join("\n");
    let body = body.trim().to_string();

    let frontmatter = parse_fields(&lines[1..end_index]);
    Some((frontmatter, body))
}

/// The fields of a whole document of `key: value` lines (a settings file), no `---` block.
pub fn parse(content: &str) -> Frontmatter {
    // A BOM is not a field, so drop it before treating every line as data.
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let lines = content.lines().collect::<Vec<_>>();
    parse_fields(&lines)
}

fn parse_fields(lines: &[&str]) -> Frontmatter {
    let mut fields = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if line.is_empty() || line.starts_with(' ') {
            index += 1;
            continue;
        }

        let Some(colon) = line.find(':') else {
            index += 1;
            continue;
        };

        let key = line[..colon].trim();
        if key.is_empty() {
            index += 1;
            continue;
        }

        let value = line[colon + 1..].trim();
        let value = parse_value(value, lines, &mut index);
        fields.push((key.to_string(), value));
        index += 1;
    }

    Frontmatter { fields }
}

// ponytail: this is a YAML subset, a real parser if skills in the wild need more.
fn parse_value(value: &str, lines: &[&str], index: &mut usize) -> Value {
    if value.is_empty() {
        return parse_block_list(lines, index);
    }

    match value {
        ">" | ">-" | ">+" | "|" | "|-" | "|+" => {
            let mut block = Vec::new();
            let mut next = *index + 1;
            while next < lines.len() && (lines[next].is_empty() || lines[next].starts_with(' ')) {
                block.push(lines[next]);
                next += 1;
            }
            *index = next - 1;
            Value::Scalar(parse_block_scalar(value, &block))
        }
        _ if value.starts_with('[') && value.ends_with(']') => {
            Value::List(parse_inline_list(value))
        }
        _ => Value::Scalar(parse_scalar(value)),
    }
}

fn parse_block_list(lines: &[&str], index: &mut usize) -> Value {
    let mut items = Vec::new();
    let mut next = *index + 1;

    while next < lines.len() && (lines[next].is_empty() || lines[next].starts_with(' ')) {
        let trimmed = lines[next].trim();
        if let Some(item) = trimmed.strip_prefix("- ") {
            items.push(parse_scalar(item));
        }
        next += 1;
    }

    *index = next - 1;

    if items.is_empty() {
        Value::Scalar(String::new())
    } else {
        Value::List(items)
    }
}

fn parse_inline_list(value: &str) -> Vec<String> {
    let Some(inner) = value.trim().strip_prefix('[') else {
        return Vec::new();
    };
    let Some(inner) = inner.strip_suffix(']') else {
        return Vec::new();
    };
    if inner.trim().is_empty() {
        return Vec::new();
    }

    split_inline_items(inner)
        .into_iter()
        .map(|item| parse_scalar(&item))
        .collect()
}

fn split_inline_items(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for ch in value.chars() {
        if in_double {
            if escaped {
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
                current.push(ch);
            } else if ch == '"' {
                in_double = false;
                current.push(ch);
            } else {
                current.push(ch);
            }
        } else if in_single {
            current.push(ch);
            if ch == '\'' {
                in_single = false;
            }
        } else {
            match ch {
                '\'' => {
                    in_single = true;
                    current.push(ch);
                }
                '"' => {
                    in_double = true;
                    current.push(ch);
                }
                ',' => {
                    items.push(current.trim().to_string());
                    current.clear();
                }
                _ => current.push(ch),
            }
        }
    }

    if !current.trim().is_empty() {
        items.push(current.trim().to_string());
    }

    items
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
    fn parse_reads_a_settings_line() {
        let fields = super::parse("port: 8080\n");
        assert_eq!(fields.scalar("port"), Some("8080"));
    }

    #[test]
    fn parses_quoted_values() {
        let (fields, body) =
            split("---\nname: \"test\"\ndescription: 'it''s'\n---\nbody\n").unwrap();
        assert_eq!(fields.scalar("name"), Some("test"));
        assert_eq!(fields.scalar("description"), Some("it's"));
        assert_eq!(body, "body");

        let (fields, _) =
            split("---\nname: test\ndescription: \"a \\\" b \\\\ c\"\n---\nbody\n").unwrap();
        assert_eq!(fields.scalar("description"), Some("a \" b \\ c"));
    }

    #[test]
    fn parses_folded_scalar() {
        let (fields, _) =
            split("---\ndescription: >\n  line one\n  line two\n---\nbody\n").unwrap();
        assert_eq!(fields.scalar("description"), Some("line one line two"));
    }

    #[test]
    fn parses_literal_scalar() {
        let (fields, _) =
            split("---\ndescription: |\n  line one\n\n  line two\n---\nbody\n").unwrap();
        assert_eq!(fields.scalar("description"), Some("line one\n\nline two"));
    }

    #[test]
    fn parses_crlf_content() {
        let (fields, body) =
            split("---\r\nname: test\r\ndescription: desc\r\n---\r\nbody\r\n").unwrap();
        assert_eq!(fields.scalar("name"), Some("test"));
        assert_eq!(fields.scalar("description"), Some("desc"));
        assert_eq!(body, "body");
    }

    #[test]
    fn missing_frontmatter_is_none() {
        assert_eq!(split("name: test\ndescription: desc\nbody"), None);
    }

    #[test]
    fn unclosed_frontmatter_is_none() {
        assert_eq!(split("---\nname: test\ndescription: desc\nbody"), None);
    }

    #[test]
    fn ignores_nested_map() {
        let (fields, body) =
            split("---\nname: test\ndescription: desc\nmetadata:\n  foo: bar\n---\nbody\n")
                .unwrap();
        assert_eq!(fields.scalar("name"), Some("test"));
        assert_eq!(fields.scalar("description"), Some("desc"));
        assert_eq!(fields.scalar("metadata"), Some(""));
        assert_eq!(body, "body");
    }

    #[test]
    fn parses_block_list() {
        let (fields, _) =
            split("---\nallowedTools:\n  - read_file\n  - ask_user\n  - load_skill\n---\nbody\n")
                .unwrap();
        assert_eq!(
            fields.list("allowedTools").map(|tools| tools.to_vec()),
            Some(vec![
                "read_file".to_string(),
                "ask_user".to_string(),
                "load_skill".to_string()
            ])
        );
    }

    #[test]
    fn parses_inline_list() {
        let (fields, _) =
            split("---\nallowedTools: [read_file, ask_user, 'load_skill']\n---\nbody\n").unwrap();
        assert_eq!(
            fields.list("allowedTools").map(|tools| tools.to_vec()),
            Some(vec![
                "read_file".to_string(),
                "ask_user".to_string(),
                "load_skill".to_string()
            ])
        );
    }

    #[test]
    fn parses_quoted_list_items() {
        let (fields, _) = split("---\ntools: [\"a \\\" b\", 'it''s']\n---\nbody\n").unwrap();
        assert_eq!(
            fields.list("tools").map(|tools| tools.to_vec()),
            Some(vec!["a \" b".to_string(), "it's".to_string()])
        );
    }

    #[test]
    fn empty_value_is_empty_scalar() {
        let (fields, _) = split("---\nempty:\n---\nbody\n").unwrap();
        assert_eq!(fields.scalar("empty"), Some(""));
        assert_eq!(fields.list("empty"), None);
    }

    #[test]
    fn empty_inline_list_is_empty_list() {
        let (fields, _) = split("---\ntools: []\n---\nbody\n").unwrap();
        assert_eq!(
            fields.list("tools").map(|tools| tools.to_vec()),
            Some(Vec::new())
        );
        assert_eq!(fields.scalar("tools"), None);
    }
}
