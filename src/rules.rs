//! Project rules: properties that must hold after every run, decidable in
//! code. They are born from corrections (see `review`), never written in bulk,
//! and read from the project's HEAD, never from a world.

use std::io;
use std::path::Path;

use serde_json::{Value, json};

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
    use super::matches;

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
