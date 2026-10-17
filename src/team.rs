//! The team: roles that brief the agent at each stage. One role is one
//! Markdown file with a small front matter:
//!
//! ```text
//! ---
//! name: QA
//! stage: prove
//! ---
//! What this role cares about, in the agent's language.
//! ```
//!
//! The user's team is `~/.config/fennec/team/*.md`; when that directory does
//! not exist, the built-in team below is used. A project adds or overrides
//! roles by file name in `.fennec-os/team/*.md`. Read from the working tree,
//! like rules said in words: both are the human's files.

use std::io;
use std::path::Path;

use crate::rules::{self, HARNESS_DIR};

/// Where a role speaks: framing "done when", solving, proving.
pub const STAGES: [&str; 3] = ["frame", "solve", "prove"];

/// The team used until the human writes their own: copy these files to
/// `~/.config/fennec/team/` to change it.
const BUILT_IN: [(&str, &str); 3] = [
    (
        "project-manager",
        include_str!("../team/project-manager.md"),
    ),
    (
        "senior-developer",
        include_str!("../team/senior-developer.md"),
    ),
    ("qa", include_str!("../team/qa.md")),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Role {
    /// The file name without `.md`: a project file of the same name
    /// replaces the user's.
    pub key: String,
    pub name: String,
    pub stage: String,
    pub brief: String,
}

/// The whole team for a project, sorted by file name.
pub fn members(project: &Path) -> io::Result<Vec<Role>> {
    let user = rules::config_dir()?.join("team");
    let mut team = match read_dir(&user)? {
        Some(files) => files,
        None => BUILT_IN
            .iter()
            .map(|(key, text)| (key.to_string(), text.to_string()))
            .collect(),
    };
    for (key, text) in read_dir(&project.join(HARNESS_DIR).join("team"))?.unwrap_or_default() {
        team.retain(|(k, _)| *k != key);
        team.push((key, text));
    }
    team.sort();
    team.iter().map(|(key, text)| parse(key, text)).collect()
}

/// What the roles of `stage` add to the agent's context, and their names
/// for the human.
pub fn brief(project: &Path, stage: &str) -> io::Result<(String, Vec<String>)> {
    let roles: Vec<Role> = members(project)?
        .into_iter()
        .filter(|r| r.stage == stage)
        .collect();
    let mut text = String::new();
    for r in &roles {
        text.push_str(&format!("\n\nAs {} on the team:\n{}\n", r.name, r.brief));
    }
    Ok((text, roles.into_iter().map(|r| r.name).collect()))
}

/// `(file name without .md, content)` for each `.md` file, or `None` when the
/// directory does not exist.
fn read_dir(dir: &Path) -> io::Result<Option<Vec<(String, String)>>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "md")
            && let Some(key) = path.file_stem().and_then(|s| s.to_str())
        {
            files.push((key.to_string(), std::fs::read_to_string(&path)?));
        }
    }
    Ok(Some(files))
}

/// A wrong role file fails loudly, naming the file: a role silently left out
/// would look like a team that does not care.
fn parse(key: &str, text: &str) -> io::Result<Role> {
    let bad = |why: &str| io::Error::other(format!("team role {key}.md: {why}"));
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| bad("must start with a `---` front matter"))?;
    let (head, body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| bad("front matter is not closed by `---`"))?;
    let field = |name: &str| {
        head.lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let stage = field("stage").ok_or_else(|| bad("no `stage`"))?;
    if !STAGES.contains(&stage.as_str()) {
        return Err(bad(&format!(
            "unknown stage `{stage}`, expected one of {}",
            STAGES.join(", ")
        )));
    }
    Ok(Role {
        key: key.to_string(),
        name: field("name").unwrap_or_else(|| key.to_string()),
        stage,
        brief: body.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_team_covers_every_stage() {
        let team: Vec<Role> = BUILT_IN.iter().map(|(k, t)| parse(k, t).unwrap()).collect();
        for stage in STAGES {
            assert!(team.iter().any(|r| r.stage == stage), "{stage}");
        }
    }

    #[test]
    fn a_role_file_is_read_or_refused_with_its_name() {
        let r = parse("qa", "---\nname: QA\nstage: prove\n---\nTry it.\n").unwrap();
        assert_eq!(
            (r.name.as_str(), r.stage.as_str(), r.brief.as_str()),
            ("QA", "prove", "Try it.")
        );
        assert_eq!(parse("ux", "---\nstage: frame\n---\nx").unwrap().name, "ux");
        let e = parse("ops", "---\nstage: deploy\n---\nx")
            .unwrap_err()
            .to_string();
        assert!(
            e.starts_with("team role ops.md: unknown stage `deploy`"),
            "{e}"
        );
        assert!(parse("dev", "no front matter").is_err());
    }

    #[test]
    fn a_project_overrides_and_extends_the_team_by_file_name() {
        let project = std::env::temp_dir().join(format!("fennec-os-team-{}", std::process::id()));
        let dir = project.join(HARNESS_DIR).join("team");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("qa.md"),
            "---\nname: QA web\nstage: prove\n---\nClick.",
        )
        .unwrap();
        std::fs::write(
            dir.join("devsecops.md"),
            "---\nname: DevSecOps\nstage: solve\n---\nAudit.",
        )
        .unwrap();
        std::fs::write(dir.join("notes.txt"), "not a role").unwrap();
        // ponytail: reads the real ~/.config/fennec/team when it exists, as the
        // other tests read the real rules.md; only project files are asserted.
        let team = members(&project).unwrap();
        assert_eq!(team.iter().filter(|r| r.key == "qa").count(), 1);
        assert_eq!(team.iter().find(|r| r.key == "qa").unwrap().name, "QA web");
        let (text, names) = brief(&project, "solve").unwrap();
        assert!(names.contains(&"DevSecOps".to_string()));
        assert!(text.contains("As DevSecOps on the team:\nAudit."));
        std::fs::remove_dir_all(project).unwrap();
    }
}
