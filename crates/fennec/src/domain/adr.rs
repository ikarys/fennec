//! Architecture decision records (fennec's own): the MADR files of a
//! project's `docs/adr/`, the human's record of the choices that shape the
//! architecture. Not the agents' `decision.recorded`: those explain one
//! change, an ADR binds the project.

use serde::Serialize;

pub const ADR_DIR: &str = "docs/adr";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adr {
    pub number: u32,
    pub file: String,
    pub title: String,
    pub status: Option<String>,
    pub date: Option<String>,
    pub body: String,
}

pub fn parse(file: &str, content: &str) -> Option<Adr> {
    let number = parse_number(file)?;

    let mut lines = content.lines();
    let first = lines.next()?.trim_end_matches(['\r', ' ', '\t']);

    let (frontmatter, body) = if first == "---" {
        let mut closed = false;
        let mut fm = Vec::new();

        for line in &mut lines {
            if line.trim_end_matches(['\r', ' ', '\t']) == "---" {
                closed = true;
                break;
            }
            fm.push(line);
        }

        if closed {
            (Some(fm), lines.collect::<Vec<_>>().join("\n"))
        } else {
            (None, content.to_owned())
        }
    } else {
        (None, content.to_owned())
    };

    let mut status = None;
    let mut date = None;

    if let Some(fm) = frontmatter {
        for line in fm {
            if let Some((key, value)) = line.split_once(':') {
                let value = strip_quotes(value.trim());
                match key.trim() {
                    "status" => status = value,
                    "date" => date = value,
                    _ => {}
                }
            }
        }
    }

    let body = body.trim_start_matches(['\n', '\r', ' ', '\t']).to_owned();
    let title = body
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("# ").map(|t| t.trim().to_owned())
        })
        .unwrap_or_else(|| file.strip_suffix(".md").unwrap_or(file).to_owned());

    Some(Adr {
        number,
        file: file.to_owned(),
        title,
        status,
        date,
        body,
    })
}

fn parse_number(file: &str) -> Option<u32> {
    if !file.ends_with(".md") {
        return None;
    }

    let mut digits = String::new();
    let mut seen_hyphen = false;

    for ch in file.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if ch == '-' {
            seen_hyphen = true;
            break;
        } else {
            break;
        }
    }

    if digits.is_empty() || !seen_hyphen {
        return None;
    }

    digits.parse::<u32>().ok()
}

fn strip_quotes(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let unquoted = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value);

    Some(unquoted.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reads_frontmatter_and_body() {
        let content = "\
---
status: accepted
date: 2024-01-01
---

# Use SQLite

Body text.
";
        let adr = parse("0003-use-sqlite.md", content).unwrap();
        assert_eq!(adr.number, 3);
        assert_eq!(adr.title, "Use SQLite");
        assert_eq!(adr.status.as_deref(), Some("accepted"));
        assert_eq!(adr.date.as_deref(), Some("2024-01-01"));
        assert!(adr.body.contains("Body text."));
    }

    #[test]
    fn parse_without_frontmatter_uses_heading_title() {
        let content = "# Fallback\n\nBody";
        let adr = parse("0001-fallback.md", content).unwrap();
        assert_eq!(adr.status, None);
        assert_eq!(adr.title, "Fallback");
    }

    #[test]
    fn parse_rejects_non_adr_files_and_falls_back_to_stem() {
        assert!(parse("README.md", "# x").is_none());
        assert!(parse("0001-x.txt", "# x").is_none());

        let adr = parse("0002-no-heading.md", "body only").unwrap();
        assert_eq!(adr.title, "0002-no-heading");
    }

    #[test]
    fn parse_treats_unclosed_frontmatter_as_body() {
        let content = "---\nstatus: accepted\n# Title";
        let adr = parse("0004-unclosed.md", content).unwrap();
        assert_eq!(adr.status, None);
        assert_eq!(adr.title, "Title");
        assert!(adr.body.contains("status: accepted"));
    }
}
