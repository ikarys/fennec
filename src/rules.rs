//! Project rules: properties that must hold after every run, decidable in
//! code. They are born from corrections (see `review`), never written in bulk,
//! and read from the project's HEAD, never from a world.
//!
//! Rules said in words (`Said`) cannot be decided in code: they are given to
//! every agent, which reports the ones its work applied. They live where
//! other tools read them too: the user's `~/.config/fennec/rules.md` and the
//! `## Rules` section of the project's AGENTS.md, one `- ` line each.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::provider::Model;
use crate::world::git;

/// Human-owned harness files in the project; no run may change them.
pub const HARNESS_DIR: &str = ".fennec-os";
pub const RULES_FILE: &str = ".fennec-os/rules.json";

pub struct Rule {
    pub id: String,
    pub kind: Kind,
    /// `block`: a violating attempt fails. `warn`: recorded only.
    pub block: bool,
}

pub enum Kind {
    /// No changed file under this pattern.
    ForbiddenPath(String),
    /// If a changed file matches `when`, one must match `then`.
    PairedChange { when: String, then: String },
    /// The command exits 0 in the world after the run.
    StandingCommand(String),
}

impl Rule {
    /// `{"id", "kind": "forbidden_path"|"paired_change"|"standing_command",
    ///   "path" | "when"+"then" | "command", "severity": "block"|"warn"}`
    pub fn from_json(v: &Value) -> io::Result<Self> {
        let field = |k: &str| {
            v[k].as_str()
                .map(str::to_string)
                .ok_or_else(|| io::Error::other(format!("rule needs a string `{k}`: {v}")))
        };
        let kind = match v["kind"].as_str() {
            Some("forbidden_path") => Kind::ForbiddenPath(field("path")?),
            Some("paired_change") => Kind::PairedChange {
                when: field("when")?,
                then: field("then")?,
            },
            Some("standing_command") => Kind::StandingCommand(field("command")?),
            _ => return Err(io::Error::other(format!("unknown rule kind: {v}"))),
        };
        let block = match v["severity"].as_str() {
            None | Some("block") => true,
            Some("warn") => false,
            Some(s) => return Err(io::Error::other(format!("unknown severity {s}"))),
        };
        Ok(Self {
            id: v["id"].as_str().unwrap_or("").to_string(),
            kind,
            block,
        })
    }

    /// Path rules judged on the changed files; commands are run by `solve`.
    pub fn violation(&self, changed: &[&str]) -> Option<String> {
        match &self.kind {
            Kind::ForbiddenPath(p) => changed
                .iter()
                .find(|f| matches(p, f))
                .map(|f| format!("changed {f}")),
            Kind::PairedChange { when, then } => {
                let trigger = changed.iter().find(|f| matches(when, f))?;
                (!changed.iter().any(|f| matches(then, f)))
                    .then(|| format!("changed {trigger} but nothing under {then}"))
            }
            Kind::StandingCommand(_) => None,
        }
    }

    pub fn describe(&self) -> String {
        match &self.kind {
            Kind::ForbiddenPath(p) => format!("do not change {p}"),
            Kind::PairedChange { when, then } => {
                format!("a change under {when} needs a change under {then}")
            }
            Kind::StandingCommand(c) => format!("`{c}` must exit 0"),
        }
    }
}

/// The project's rules at HEAD. A missing file means no rules.
pub fn load(project: &Path) -> io::Result<Vec<Rule>> {
    let Ok(text) = git(project, &["show", &format!("HEAD:{RULES_FILE}")]) else {
        return Ok(Vec::new());
    };
    let v: Value =
        serde_json::from_str(&text).map_err(|e| io::Error::other(format!("{RULES_FILE}: {e}")))?;
    v.as_array()
        .ok_or_else(|| io::Error::other(format!("{RULES_FILE} is not a list")))?
        .iter()
        .map(Rule::from_json)
        .collect()
}

/// Appends a rule to the project's working tree. It takes effect once the
/// human commits it: the harness reads rules from HEAD.
pub fn promote(project: &Path, rule: &Value, origin: &str) -> io::Result<String> {
    Rule::from_json(rule)?;
    let path = project.join(RULES_FILE);
    let mut rules: Vec<Value> = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(io::Error::other)?,
        Err(_) => Vec::new(),
    };
    let id = format!("r{}", rules.len() + 1);
    let mut rule = rule.clone();
    rule["id"] = json!(id);
    rule["origin"] = json!(origin);
    rules.push(rule);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&rules).map_err(io::Error::other)?;
    std::fs::write(&path, text + "\n")?;
    Ok(id)
}

/// A rule said in words. `scope` is `user` (every project) or `project`.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub scope: &'static str,
    pub text: String,
}

/// `$XDG_CONFIG_HOME/fennec`, default `~/.config/fennec`: what the human
/// configures for every project.
pub fn config_dir() -> io::Result<PathBuf> {
    let config = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(
            std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?,
        )
        .join(".config"),
    };
    Ok(config.join("fennec"))
}

/// `$XDG_CONFIG_HOME/fennec/rules.md`, default `~/.config/fennec/rules.md`.
pub fn user_file() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("rules.md"))
}

const SECTION: &str = "## Rules";

/// The user's rules, then the project's. Read from the working tree, not
/// HEAD: a rule the human just confirmed applies to the next run, committed
/// or not. Both files are the human's; no world writes them.
pub fn said(project: &Path) -> io::Result<Vec<Said>> {
    let read = |p: PathBuf| match std::fs::read_to_string(p) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    };
    let user = read(user_file()?)?;
    let agents = read(project.join("AGENTS.md"))?;
    let bullets = |text: &str| -> Vec<String> {
        text.lines()
            .filter_map(|l| l.strip_prefix("- "))
            .map(|l| l.trim().to_string())
            .collect()
    };
    let user = bullets(&user).into_iter().map(|text| Said {
        scope: "user",
        text,
    });
    let project = bullets(&section(&agents)).into_iter().map(|text| Said {
        scope: "project",
        text,
    });
    Ok(user.chain(project).collect())
}

/// The lines under `## Rules`, up to the next heading of the same level.
fn section(text: &str) -> String {
    text.lines()
        .skip_while(|l| l.trim_end() != SECTION)
        .skip(1)
        .take_while(|l| !l.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Writes a rule down once, for `user` or `project`. Saying it again is a
/// no-op. The project's AGENTS.md is committed only where worlds would wait
/// for it (`world::keep`).
pub fn say(project: &Path, scope: &str, text: &str) -> io::Result<()> {
    let text = text.trim().replace('\n', " ");
    if text.is_empty() {
        return Err(io::Error::other("empty rule"));
    }
    if said(project)?
        .iter()
        .any(|s| s.scope == scope && s.text == text)
    {
        return Ok(());
    }
    let path = match scope {
        "user" => user_file()?,
        "project" => project.join("AGENTS.md"),
        _ => return Err(io::Error::other(format!("unknown scope {scope}"))),
    };
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let new = if scope == "user" {
        format!("{old}- {text}\n")
    } else {
        add_to_section(&old, &text)
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, new)?;
    if scope == "project" {
        crate::world::keep(project, "AGENTS.md", "docs: add a rule for fennec")?;
    }
    Ok(())
}

/// Takes a rule back: its line goes, the rest of the file stays as written.
/// In the project's AGENTS.md only the `## Rules` section is touched.
pub fn unsay(project: &Path, scope: &str, text: &str) -> io::Result<()> {
    let path = match scope {
        "user" => user_file()?,
        "project" => project.join("AGENTS.md"),
        _ => return Err(io::Error::other(format!("unknown scope {scope}"))),
    };
    let old = match std::fs::read_to_string(&path) {
        Ok(old) => old,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    std::fs::write(&path, remove_line(&old, text.trim(), scope == "project"))?;
    if scope == "project" {
        crate::world::keep(project, "AGENTS.md", "docs: drop a rule for fennec")?;
    }
    Ok(())
}

fn remove_line(doc: &str, rule: &str, in_section: bool) -> String {
    let mut inside = !in_section;
    let mut out = String::new();
    for line in doc.split_inclusive('\n') {
        if in_section && line.starts_with("## ") {
            inside = line.trim_end() == SECTION;
        }
        if inside && line.strip_prefix("- ").is_some_and(|l| l.trim() == rule) {
            continue;
        }
        out.push_str(line);
    }
    out
}

fn add_to_section(doc: &str, rule: &str) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    let Some(start) = lines.iter().position(|l| l.trim_end() == SECTION) else {
        let sep = match doc {
            "" => "",
            d if d.ends_with("\n\n") => "",
            d if d.ends_with('\n') => "\n",
            _ => "\n\n",
        };
        return format!("{doc}{sep}{SECTION}\n\n- {rule}\n");
    };
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map_or(lines.len(), |i| start + 1 + i);
    // After the section's last non-blank line.
    let at = (start + 1..end)
        .rev()
        .find(|&i| !lines[i].trim().is_empty())
        .map_or(start + 1, |i| i + 1);
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    let bullet = format!("- {rule}");
    if at == start + 1 {
        out.splice(at..at, [String::new(), bullet]);
    } else {
        out.insert(at, bullet);
    }
    out.join("\n") + "\n"
}

/// Rules as given to an agent, numbered so it can say which ones it applied.
pub fn brief(said: &[Said]) -> String {
    if said.is_empty() {
        return String::new();
    }
    let mut text = "\n\nRules the human set; follow them. If your work applied some, add \
                    a last line to your `done` summary: `rules: <their numbers>`.\n"
        .to_string();
    for (n, s) in said.iter().enumerate() {
        text.push_str(&format!("  {}. {}\n", n + 1, s.text));
    }
    text
}

/// The rules an agent says it applied, from the `rules:` line of its summary.
// ponytail: the agent's word, not a check; a rule that can be decided in code
// belongs in rules.json, where a violation fails the run.
pub fn used(summary: &str, said: &[Said]) -> Vec<String> {
    let Some(line) = summary
        .lines()
        .rev()
        .find_map(|l| l.trim().trim_matches('`').strip_prefix("rules:"))
    else {
        return Vec::new();
    };
    let mut used: Vec<String> = line
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse::<usize>().ok())
        .filter_map(|n| said.get(n.checked_sub(1)?))
        .map(|s| s.text.clone())
        .collect();
    used.dedup();
    used
}

const PROPOSE: &str = "A human corrected an AI developer. Decide whether the correction states \
a lasting preference that would hold for other tasks too (a tool to use or avoid, an identity, \
a branch never to touch, a style), or only fixes this task. What the product should do (a \
feature, a default, a behaviour, a screen) only fixes this task. If it holds beyond the task, reply \
with one line: `rule: <the preference as a short imperative rule, in the human's language>`. \
Otherwise reply `rule: none`.";

/// A correction that holds beyond its task, restated as a rule for the human
/// to confirm; `None` when it only fixes this task. The model proposes, the
/// human decides: nothing is written here.
pub fn propose(model: &dyn Model, request: &str, correction: &str) -> io::Result<Option<String>> {
    let messages = [
        json!({"role": "system", "content": PROPOSE}),
        json!({"role": "user", "content": format!("Task: {request}\n\nCorrection: {correction}")}),
    ];
    let reply = model.chat(&messages, &json!([]))?;
    let content = reply["content"].as_str().unwrap_or("");
    let content = content.rsplit("</think>").next().unwrap_or(content);
    Ok(content
        .lines()
        .find_map(|l| l.trim().trim_matches('`').strip_prefix("rule:"))
        .map(|r| r.trim().trim_matches('`').trim().to_string())
        .filter(|r| !r.is_empty() && !r.eq_ignore_ascii_case("none")))
}

/// A pattern matches a path, or any directory above it: `tests` covers
/// `tests/a.rs`. `*` stays within a segment, `**` crosses them.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_end_matches('/');
    let mut prefix = path;
    loop {
        if glob(pattern.as_bytes(), prefix.as_bytes()) {
            return true;
        }
        match prefix.rfind('/') {
            Some(i) => prefix = &prefix[..i],
            None => return false,
        }
    }
}

fn glob(p: &[u8], s: &[u8]) -> bool {
    match p {
        [] => s.is_empty(),
        [b'*', b'*', rest @ ..] => {
            let rest = rest.strip_prefix(b"/").unwrap_or(rest);
            (0..=s.len()).any(|i| (i == 0 || s[i - 1] == b'/') && glob(rest, &s[i..]))
        }
        [b'*', rest @ ..] => (0..=s.len())
            .take_while(|&i| i == 0 || s[i - 1] != b'/')
            .any(|i| glob(rest, &s[i..])),
        [b'?', rest @ ..] => s.first().is_some_and(|&c| c != b'/') && glob(rest, &s[1..]),
        [c, rest @ ..] => s.first() == Some(c) && glob(rest, &s[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_go_in_their_section() {
        assert_eq!(add_to_section("", "use uv"), "## Rules\n\n- use uv\n");
        assert_eq!(
            add_to_section("# P\n", "use uv"),
            "# P\n\n## Rules\n\n- use uv\n"
        );
        let doc = "# P\n\n## Rules\n\n- a\n\n## Style\n\n- not a rule\n";
        let new = add_to_section(doc, "b");
        assert_eq!(
            new,
            "# P\n\n## Rules\n\n- a\n- b\n\n## Style\n\n- not a rule\n"
        );
        assert_eq!(section(&new), "\n- a\n- b\n");
        assert_eq!(
            add_to_section("## Rules\n## Style\n", "b"),
            "## Rules\n\n- b\n## Style\n"
        );
    }

    #[test]
    fn a_rule_taken_back_leaves_the_rest() {
        let doc = "# P\n\n- a\n\n## Rules\n\n- a\n- b\n\n## Style\n\n- a\n";
        assert_eq!(
            remove_line(doc, "a", true),
            "# P\n\n- a\n\n## Rules\n\n- b\n\n## Style\n\n- a\n"
        );
        assert_eq!(remove_line("- a\n- b", "b", false), "- a\n");
    }

    #[test]
    fn used_rules_come_from_the_summary() {
        let said = [
            Said {
                scope: "user",
                text: "use uv".into(),
            },
            Said {
                scope: "project",
                text: "never touch develop".into(),
            },
        ];
        let summary = "goal: x\nverify: y\nrules: 2, 9, 2";
        assert_eq!(used(summary, &said), ["never touch develop"]);
        assert!(used("done", &said).is_empty());
        assert!(brief(&said).contains("  2. never touch develop"));
    }

    #[test]
    fn patterns() {
        assert!(matches("tests", "tests/a.rs"));
        assert!(matches("tests/", "tests/sub/a.rs"));
        assert!(!matches("tests", "src/tests.rs"));
        assert!(matches("src/*.rs", "src/a.rs"));
        assert!(!matches("src/*.rs", "src/x/a.rs"));
        assert!(matches("src/**/*.rs", "src/x/y/a.rs"));
        assert!(matches("src/**/*.rs", "src/a.rs"));
        assert!(matches("**/*.sql", "db/m/1.sql"));
        assert!(matches(".fennec-os", ".fennec-os/rules.json"));
        assert!(!matches("a?c", "a/c"));
    }
}
