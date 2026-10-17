//! The team: roles that brief the agent at each stage. One role is one
//! Markdown file with a small front matter:
//!
//! ```text
//! ---
//! name: QA
//! stage: prove
//! check: gitleaks detect --no-git
//! ---
//! What this role cares about, in the agent's language.
//! ```
//!
//! `check` lines (prove roles only, one command each) are the role's veto:
//! the harness runs them on every proof and a failure blocks "ready". The
//! brief only asks; a check decides, by exit code.
//!
//! A team is a named set of roles, because different work needs different
//! people: software is not a study. The human's teams are folders,
//! `~/.config/fennec/teams/<team>/*.md`; built-in teams are lists of roles
//! from the built-in catalogue, and a folder of the same name replaces one.
//! A project picks its team in `.fennec-os/team.json`
//! (`{"team": "Études", "remove": ["qa"]}`, default team `Développement`) and adds or
//! overrides roles by file name in `.fennec-os/team/*.md`. Read from the
//! working tree, like rules said in words: these are the human's files.

use std::io;
use std::path::Path;

use serde_json::Value;

use crate::rules::{self, HARNESS_DIR};

/// Where a role speaks: framing "done when", solving, proving.
pub const STAGES: [&str; 3] = ["frame", "solve", "prove"];

/// The team of a project that has not picked one: small on purpose.
pub const DEFAULT_TEAM: &str = "Développement";

/// Every built-in role, in or out of a built-in team: what the human picks
/// from to build their own.
const CATALOG: [(&str, &str); 14] = [
    ("architect", include_str!("../catalog/architect.md")),
    (
        "critical-reviewer",
        include_str!("../catalog/critical-reviewer.md"),
    ),
    ("devops", include_str!("../catalog/devops.md")),
    ("devsecops", include_str!("../catalog/devsecops.md")),
    (
        "frontend-designer",
        include_str!("../catalog/frontend-designer.md"),
    ),
    ("maintainer", include_str!("../catalog/maintainer.md")),
    (
        "project-manager",
        include_str!("../catalog/project-manager.md"),
    ),
    ("qa", include_str!("../catalog/qa.md")),
    ("research-lead", include_str!("../catalog/research-lead.md")),
    ("researcher", include_str!("../catalog/researcher.md")),
    (
        "senior-developer",
        include_str!("../catalog/senior-developer.md"),
    ),
    (
        "sources-specialist",
        include_str!("../catalog/sources-specialist.md"),
    ),
    ("ux-specialist", include_str!("../catalog/ux-specialist.md")),
    ("writer", include_str!("../catalog/writer.md")),
];

/// Built-in teams, by catalogue key. Each covers every stage.
const TEAMS: [(&str, &[&str]); 2] = [
    (
        "Développement",
        &["project-manager", "senior-developer", "qa"],
    ),
    (
        "Études",
        &[
            "research-lead",
            "researcher",
            "writer",
            "critical-reviewer",
            "sources-specialist",
        ],
    ),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Role {
    /// The file name without `.md`: a project file of the same name
    /// replaces the user's.
    pub key: String,
    pub name: String,
    pub stage: String,
    pub brief: String,
    /// Commands run at every proof; see the module doc.
    pub checks: Vec<String>,
}

/// Every built-in role.
pub fn catalog() -> io::Result<Vec<Role>> {
    CATALOG.iter().map(|(key, text)| parse(key, text)).collect()
}

/// The teams a project can pick: the human's folders, then the built-in
/// teams they do not replace.
pub fn teams() -> io::Result<Vec<String>> {
    let mut names: Vec<String> = match std::fs::read_dir(rules::config_dir()?.join("teams")) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| valid_name(n))
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    names.sort();
    for (name, _) in TEAMS {
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

/// A project's team as the human sets it up: the team picked, the roles it
/// leaves out, and every role before any is left out (so a left-out role can
/// be shown, and taken back). A team that cannot be found (deleted, or named
/// by a project cloned from someone else) is reported in `missing` instead of
/// failing, so the human can pick another; the work itself still fails.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub team: String,
    pub remove: Vec<String>,
    pub roles: Vec<Role>,
    pub missing: Option<String>,
}

pub fn setup(project: &Path) -> io::Result<Setup> {
    let harness = project.join(HARNESS_DIR);
    let (team, remove) = choice(&harness)?;
    let (mut files, missing) = match team_files(&team) {
        Ok(files) => (files, None),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };
    for (key, text) in read_dir(&harness.join("team"))?.unwrap_or_default() {
        files.retain(|(k, _)| *k != key);
        files.push((key, text));
    }
    files.sort();
    let roles = files
        .iter()
        .map(|(key, text)| parse(key, text))
        .collect::<io::Result<_>>()?;
    Ok(Setup {
        team,
        remove,
        roles,
        missing,
    })
}

/// The roles that work on a project, sorted by file name.
pub fn members(project: &Path) -> io::Result<Vec<Role>> {
    let Setup {
        remove,
        roles,
        missing,
        ..
    } = setup(project)?;
    if let Some(why) = missing {
        return Err(io::Error::other(why));
    }
    Ok(roles
        .into_iter()
        .filter(|r| !remove.contains(&r.key))
        .collect())
}

/// The human picks a project's team and the roles it leaves out. An unknown
/// team fails before anything is written. The file is left uncommitted, like
/// a rule written to AGENTS.md.
pub fn choose(project: &Path, team: &str, remove: &[String]) -> io::Result<()> {
    team_files(team)?;
    let harness = project.join(HARNESS_DIR);
    std::fs::create_dir_all(&harness)?;
    let json = serde_json::json!({"team": team, "remove": remove});
    std::fs::write(
        harness.join("team.json"),
        serde_json::to_string_pretty(&json).map_err(io::Error::other)? + "\n",
    )
}

/// A catalogue role joins a project's team as a file of its own, which the
/// human can then edit. A role left out before is taken back.
pub fn add(project: &Path, key: &str) -> io::Result<()> {
    let Some((_, text)) = CATALOG.iter().find(|(k, _)| *k == key) else {
        return Err(io::Error::other(format!("unknown role `{key}`")));
    };
    let harness = project.join(HARNESS_DIR);
    let dir = harness.join("team");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{key}.md"));
    if !path.exists() {
        std::fs::write(path, text)?;
    }
    let (team, mut remove) = choice(&harness)?;
    if remove.iter().any(|k| k == key) {
        remove.retain(|k| k != key);
        choose(project, &team, &remove)?;
    }
    Ok(())
}

/// The human writes a role for a project: an edit of role `key`, or a new
/// role when `key` is `None` (its key is made from the name). Returns the key.
pub fn save_role(
    project: &Path,
    key: Option<&str>,
    name: &str,
    stage: &str,
    brief: &str,
    checks: &[String],
) -> io::Result<String> {
    let (name, brief) = (name.trim(), brief.trim());
    if name.is_empty() || brief.is_empty() {
        return Err(io::Error::other("a role needs a name and a brief"));
    }
    let key = match key {
        Some(k) if setup(project)?.roles.iter().any(|r| r.key == k) => k.to_string(),
        Some(k) => return Err(io::Error::other(format!("unknown role `{k}`"))),
        None => {
            let k = slug(name);
            if k.is_empty() || setup(project)?.roles.iter().any(|r| r.key == k) {
                return Err(io::Error::other(format!(
                    "a role named `{name}` exists already"
                )));
            }
            k
        }
    };
    // Each check is one front matter line.
    if checks.iter().any(|c| c.contains(['\n', '\r'])) {
        return Err(io::Error::other("a check is one line"));
    }
    let role = Role {
        key: key.clone(),
        name: name.to_string(),
        stage: stage.to_string(),
        brief: brief.to_string(),
        checks: checks
            .iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect(),
    };
    // Checked as any role file is, before it is written.
    parse(&key, &to_md(&role))?;
    let dir = project.join(HARNESS_DIR).join("team");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{key}.md")), to_md(&role))?;
    Ok(key)
}

/// The project's working roles become team `name` of the human's own, for
/// every project, and the project switches to it. Its role files, now in
/// the team, leave the project. An existing team is never overwritten.
pub fn save_team(project: &Path, name: &str) -> io::Result<()> {
    let name = name.trim();
    if !valid_name(name) {
        return Err(io::Error::other(format!("invalid team name `{name}`")));
    }
    if teams()?.iter().any(|t| t == name) {
        return Err(io::Error::other(format!("team `{name}` exists already")));
    }
    let roles = members(project)?;
    let dir = rules::config_dir()?.join("teams").join(name);
    std::fs::create_dir_all(&dir)?;
    for r in &roles {
        std::fs::write(dir.join(format!("{}.md", r.key)), to_md(r))?;
    }
    choose(project, name, &[])?;
    for r in &roles {
        let file = project
            .join(HARNESS_DIR)
            .join("team")
            .join(format!("{}.md", r.key));
        if file.exists() {
            std::fs::remove_file(file)?;
        }
    }
    Ok(())
}

fn to_md(r: &Role) -> String {
    let checks: String = r.checks.iter().map(|c| format!("check: {c}\n")).collect();
    format!(
        "---\nname: {}\nstage: {}\n{checks}---\n{}\n",
        r.name, r.stage, r.brief
    )
}

/// A file name from a role's name: `Spécialiste UX` → `specialiste-ux`.
fn slug(name: &str) -> String {
    let folded: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    folded
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// The project's team and the roles it leaves out, from `team.json`.
fn choice(harness: &Path) -> io::Result<(String, Vec<String>)> {
    let path = harness.join("team.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok((DEFAULT_TEAM.to_string(), Vec::new()));
        }
        Err(e) => return Err(e),
    };
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| io::Error::other(format!("{}: {e}", path.display())))?;
    let name = v["team"].as_str().unwrap_or(DEFAULT_TEAM).to_string();
    let remove = v["remove"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect();
    Ok((name, remove))
}

/// A team name is one folder under `teams/`: the project's file names it,
/// and a cloned project must not point fennec at another directory.
fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\'])
}

/// The role files of team `name`: the human's folder, else the built-in team.
fn team_files(name: &str) -> io::Result<Vec<(String, String)>> {
    if !valid_name(name) {
        return Err(io::Error::other(format!("invalid team name `{name}`")));
    }
    if let Some(files) = read_dir(&rules::config_dir()?.join("teams").join(name))? {
        return Ok(files);
    }
    let Some((_, keys)) = TEAMS.iter().find(|(n, _)| *n == name) else {
        return Err(io::Error::other(format!(
            "unknown team `{name}`; known: {}",
            teams()?.join(", ")
        )));
    };
    Ok(CATALOG
        .iter()
        .filter(|(key, _)| keys.contains(key))
        .map(|(key, text)| (key.to_string(), text.to_string()))
        .collect())
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

/// The commands the project's prove roles run on every proof:
/// `(role name, command)`.
pub fn checks(project: &Path) -> io::Result<Vec<(String, String)>> {
    Ok(members(project)?
        .into_iter()
        .flat_map(|r| r.checks.into_iter().map(move |c| (r.name.clone(), c)))
        .collect())
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
    let checks: Vec<String> = head
        .lines()
        .filter_map(|l| l.strip_prefix("check:"))
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    // Only a proof runs them: elsewhere they would be a veto that never acts.
    if !checks.is_empty() && stage != "prove" {
        return Err(bad("only a `prove` role can have `check` lines"));
    }
    Ok(Role {
        key: key.to_string(),
        name: field("name").unwrap_or_else(|| key.to_string()),
        stage,
        brief: body.trim().to_string(),
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_team_covers_every_stage_from_the_catalogue() {
        let catalog = catalog().unwrap();
        for (name, keys) in TEAMS {
            let team: Vec<&Role> = catalog
                .iter()
                .filter(|r| keys.contains(&r.key.as_str()))
                .collect();
            assert_eq!(
                team.len(),
                keys.len(),
                "{name}: a key is not in the catalogue"
            );
            for stage in STAGES {
                assert!(team.iter().any(|r| r.stage == stage), "{name}: {stage}");
            }
        }
    }

    #[test]
    fn a_team_name_cannot_leave_the_teams_folder() {
        for bad in ["", "..", "../x", ".hidden", "a/b"] {
            assert!(team_files(bad).is_err(), "{bad}");
        }
        let e = team_files("nope-no-such-team").unwrap_err().to_string();
        assert!(
            e.contains("unknown team") && e.contains("Développement") && e.contains("Études"),
            "{e}"
        );
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
        let sec =
            "---\nname: Sec\nstage: prove\ncheck: gitleaks detect\ncheck: bandit -r .\n---\nx";
        let r = parse("sec", sec).unwrap();
        assert_eq!(r.checks, ["gitleaks detect", "bandit -r ."]);
        assert_eq!(parse("sec", &to_md(&r)).unwrap(), r, "written as read");
        assert!(parse("dev", "---\nstage: solve\ncheck: x\n---\nx").is_err());
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
        // ponytail: reads the real ~/.config/fennec/teams when it exists, as
        // the other tests read the real rules.md; only project files are asserted.
        let team = members(&project).unwrap();
        assert_eq!(team.iter().filter(|r| r.key == "qa").count(), 1);
        assert_eq!(team.iter().find(|r| r.key == "qa").unwrap().name, "QA web");
        let (text, names) = brief(&project, "solve").unwrap();
        assert!(names.contains(&"DevSecOps".to_string()));
        assert!(text.contains("As DevSecOps on the team:\nAudit."));

        // Another team, minus one of its roles; project roles still apply.
        let choose =
            |json: &str| std::fs::write(project.join(HARNESS_DIR).join("team.json"), json).unwrap();
        choose(r#"{"team": "Études", "remove": ["writer"]}"#);
        let keys: Vec<String> = members(&project)
            .unwrap()
            .into_iter()
            .map(|r| r.key)
            .collect();
        assert!(
            keys.contains(&"researcher".to_string()) && keys.contains(&"devsecops".to_string())
        );
        assert!(
            !keys.contains(&"writer".to_string())
                && !keys.contains(&"senior-developer".to_string())
        );
        choose(r#"{"team": "../../etc"}"#);
        assert!(members(&project).is_err());
        // The work fails, but the setup still opens so the human can repair it.
        choose(r#"{"team": "gone"}"#);
        assert!(members(&project).is_err());
        let s = setup(&project).unwrap();
        assert!(s.missing.unwrap().contains("unknown team `gone`"));
        assert!(
            s.roles.iter().any(|r| r.key == "devsecops"),
            "project roles still shown"
        );
        std::fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn the_human_picks_a_team_leaves_a_role_out_and_takes_it_back() {
        let project = std::env::temp_dir().join(format!("fennec-os-choose-{}", std::process::id()));
        std::fs::create_dir_all(&project).unwrap();
        assert!(super::choose(&project, "no-such-team", &[]).is_err());
        assert!(
            !project.join(HARNESS_DIR).exists(),
            "nothing written on error"
        );

        super::choose(&project, "Études", &["writer".to_string()]).unwrap();
        let s = setup(&project).unwrap();
        assert_eq!(
            (s.team.as_str(), s.remove.clone()),
            ("Études", vec!["writer".to_string()])
        );
        assert!(
            s.roles.iter().any(|r| r.key == "writer"),
            "shown, though left out"
        );
        assert!(!members(&project).unwrap().iter().any(|r| r.key == "writer"));

        add(&project, "writer").unwrap();
        add(&project, "devsecops").unwrap();
        assert!(add(&project, "nobody").is_err());
        let keys: Vec<String> = members(&project)
            .unwrap()
            .into_iter()
            .map(|r| r.key)
            .collect();
        assert!(keys.contains(&"writer".to_string()) && keys.contains(&"devsecops".to_string()));
        assert!(project.join(HARNESS_DIR).join("team/devsecops.md").exists());
        std::fs::remove_dir_all(project).unwrap();
    }

    // ponytail: save_team writes the real ~/.config/fennec/teams, so it is
    // checked through the UI against an isolated XDG_CONFIG_HOME, not here.
    #[test]
    fn the_human_edits_a_role_and_writes_a_new_one() {
        let project = std::env::temp_dir().join(format!("fennec-os-role-{}", std::process::id()));
        std::fs::create_dir_all(&project).unwrap();
        super::choose(&project, "Développement", &[]).unwrap();

        save_role(
            &project,
            Some("qa"),
            "QA web",
            "prove",
            "Click everything.",
            &[],
        )
        .unwrap();
        let qa = members(&project)
            .unwrap()
            .into_iter()
            .find(|r| r.key == "qa")
            .unwrap();
        assert_eq!(
            (qa.name.as_str(), qa.brief.as_str()),
            ("QA web", "Click everything.")
        );

        let key = save_role(
            &project,
            None,
            "Spécialiste données",
            "solve",
            "Check the data.",
            &[],
        )
        .unwrap();
        assert_eq!(key, "specialiste-donnees");
        assert!(
            save_role(&project, None, "Spécialiste données", "solve", "x", &[]).is_err(),
            "taken"
        );
        assert!(save_role(&project, Some("nobody"), "x", "solve", "x", &[]).is_err());
        assert!(
            save_role(&project, None, "Ops", "deploy", "x", &[]).is_err(),
            "unknown stage"
        );
        assert!(
            save_role(&project, None, "Ops", "solve", " ", &[]).is_err(),
            "empty brief"
        );
        let veto = ["gitleaks detect".to_string()];
        save_role(&project, Some("qa"), "QA", "prove", "Try.", &veto).unwrap();
        assert_eq!(
            checks(&project).unwrap(),
            [("QA".to_string(), veto[0].clone())]
        );
        let two = ["a\nb".to_string()];
        assert!(save_role(&project, Some("qa"), "QA", "prove", "Try.", &two).is_err());
        assert_eq!(slug("  Dev -- Senior! "), "dev-senior");
        std::fs::remove_dir_all(project).unwrap();
    }
}
