//! The project's knowledge: the architecture vision (`ARCHITECTURE.md`), the
//! sensitive zones (`.fennec-os/zones.json`) and the architecture decision
//! records (`docs/adr/NNNN-title.md`). Committed in the project, read at
//! HEAD, and part of the vision's zone, so a task's agent never writes them.
//!
//! A model only proposes a knowledge file's full new text; this module
//! checks a proposal and writes what the human accepted. What a model says
//! never picks a path outside the knowledge, nor an ADR's number.

use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::rules::matches;
use crate::world::git;
use crate::zones::{VISION_FILE, ZONES_FILE};

pub const ADR_DIR: &str = "docs/adr";

/// One decision record, as found at HEAD.
#[derive(Debug, Clone, PartialEq)]
pub struct Adr {
    pub number: u32,
    pub title: String,
    pub status: String,
    pub file: String,
}

/// Whether `path` is a knowledge file: the only paths a proposal may write.
pub fn is_knowledge(path: &str) -> bool {
    path == VISION_FILE
        || path == ZONES_FILE
        || path
            .strip_prefix(ADR_DIR)
            .and_then(|r| r.strip_prefix('/'))
            .is_some_and(|name| !name.contains('/') && name.ends_with(".md"))
}

/// The decision records at HEAD, by number. A file whose name does not start
/// with a number is not a record (a README in the folder).
pub fn adrs(project: &Path) -> io::Result<Vec<Adr>> {
    let Ok(names) = git(
        project,
        &["ls-tree", "--name-only", "HEAD", &format!("{ADR_DIR}/")],
    ) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for file in names.lines().filter(|f| f.ends_with(".md")) {
        let name = file.rsplit('/').next().unwrap_or(file);
        let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
        let Ok(number) = digits.parse::<u32>() else {
            continue;
        };
        let text = git(project, &["show", &format!("HEAD:{file}")]).unwrap_or_default();
        out.push(Adr {
            number,
            title: heading(&text).unwrap_or_else(|| name.to_string()),
            status: status(&text).unwrap_or_default(),
            file: file.to_string(),
        });
    }
    out.sort_by_key(|a| a.number);
    Ok(out)
}

/// What the agents are told of the decisions: one line each. The full text
/// is in the project, for their tools to read.
pub fn index(adrs: &[Adr]) -> String {
    if adrs.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\nArchitecture decisions (docs/adr, number. title — status):\n");
    for a in adrs {
        out.push_str(&format!(
            "  {}. {} — {} ({})\n",
            a.number, a.title, a.status, a.file
        ));
    }
    out
}

/// The heading's title, without the "N." a record numbers itself with.
fn heading(text: &str) -> Option<String> {
    let line = text.lines().find(|l| l.starts_with("# "))?;
    let title = line[2..].trim();
    let title = match title.split_once(". ") {
        Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => title,
    };
    Some(title.trim().to_string()).filter(|t| !t.is_empty())
}

fn status(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("Status:"))
        .map(|s| s.trim().to_string())
}

/// A proposal checked and made ready to write: `{"files": [{"file",
/// "content", "why"}], "checks": [{"ok", "text"}]}`. New records get the
/// next free numbers and their file names here; a record that supersedes
/// another marks the other superseded. Refused: a path outside the
/// knowledge, a zones file that does not parse.
pub fn check(project: &Path, proposals: &[Value]) -> io::Result<Value> {
    let known = adrs(project)?;
    let mut next = known.iter().map(|a| a.number).max().unwrap_or(0) + 1;
    let mut files: Vec<Value> = Vec::new();
    let mut checks: Vec<Value> = Vec::new();
    let tracked: Vec<String> = git(project, &["ls-files"])
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    for p in proposals {
        let file = p["file"].as_str().unwrap_or("").trim();
        let content = p["content"].as_str().unwrap_or("");
        let why = p["why"].clone();
        if !is_knowledge(file) {
            return Err(io::Error::other(format!(
                "{file}: not a knowledge file (only {VISION_FILE}, {ZONES_FILE} and {ADR_DIR}/*.md)"
            )));
        }
        if content.trim().is_empty() {
            return Err(io::Error::other(format!("{file}: empty")));
        }
        if file == ZONES_FILE {
            let zones = crate::zones::parse(content)?;
            checks.push(json!({"ok": true, "text": format!("{ZONES_FILE} is valid: {} zones", zones.len())}));
            for z in &zones {
                if !z
                    .paths
                    .iter()
                    .any(|pat| tracked.iter().any(|t| matches(pat, t)))
                {
                    checks.push(json!({"ok": false,
                        "text": format!("zone {} matches no file yet", z.name)}));
                }
            }
            files.push(json!({"file": file, "content": content, "why": why}));
            continue;
        }
        if file.starts_with(ADR_DIR) && !known.iter().any(|a| a.file == file) {
            // A new record: its number and name are the kernel's.
            let title = heading(content).unwrap_or_else(|| "Decision".to_string());
            let number = next;
            next += 1;
            let name = format!("{ADR_DIR}/{number:04}-{}.md", slug(&title));
            let text = numbered(content, number, &title);
            checks
                .push(json!({"ok": true, "text": format!("decision {number} recorded as {name}")}));
            if let Some(old) = status(&text)
                .and_then(|s| s.strip_prefix("supersedes").map(|n| n.trim().to_string()))
                .and_then(|n| n.parse::<u32>().ok())
            {
                match known.iter().find(|a| a.number == old) {
                    Some(a) => {
                        let before = git(project, &["show", &format!("HEAD:{}", a.file)])?;
                        files.push(json!({"file": a.file,
                            "content": restatus(&before, &format!("superseded by {number}")),
                            "why": format!("superseded by decision {number}")}));
                        checks.push(json!({"ok": true, "text": format!("decision {old} marked superseded by {number}")}));
                    }
                    None => checks.push(json!({"ok": false,
                        "text": format!("decision {number} supersedes {old}, which does not exist")})),
                }
            }
            files.push(json!({"file": name, "content": text, "why": why}));
            continue;
        }
        files.push(json!({"file": file, "content": content, "why": why}));
    }
    Ok(json!({"files": files, "checks": checks}))
}

/// The record's heading set to its number, and a date when it has none.
fn numbered(content: &str, number: u32, title: &str) -> String {
    let mut out = String::new();
    let mut headed = false;
    for line in content.lines() {
        if !headed && line.starts_with("# ") {
            out.push_str(&format!("# {number}. {title}\n"));
            headed = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !headed {
        out = format!("# {number}. {title}\n\n{out}");
    }
    if !out.lines().any(|l| l.starts_with("Date:")) {
        out = out.replacen('\n', &format!("\n\nDate: {}\n", today()), 1);
    }
    out
}

fn restatus(text: &str, to: &str) -> String {
    let mut out: String = text
        .lines()
        .map(|l| {
            if l.starts_with("Status:") {
                format!("Status: {to}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

fn slug(title: &str) -> String {
    let mut s = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s: String = s.trim_end_matches('-').chars().take(50).collect();
    if s.is_empty() {
        "decision".to_string()
    } else {
        s.trim_end_matches('-').to_string()
    }
}

/// Today as YYYY-MM-DD (UTC), from the days since the epoch.
fn today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Writes checked files under `dir` (the project folder, or a world before
/// it lands). Knowledge paths only, whatever the caller passes.
pub fn write(dir: &Path, files: &[Value]) -> io::Result<Vec<String>> {
    let mut written = Vec::new();
    for f in files {
        let file = f["file"].as_str().unwrap_or("");
        if !is_knowledge(file) {
            return Err(io::Error::other(format!("{file}: not a knowledge file")));
        }
        let path = dir.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, f["content"].as_str().unwrap_or(""))?;
        written.push(file.to_string());
    }
    Ok(written)
}

/// The human accepted these files: written in the project folder and
/// committed alone on its branch, the rest of the folder untouched. Refused
/// while a world holds the folder, or when the dev has uncommitted changes
/// on one of these files (they would be lost). Returns the commit.
pub fn commit(project: &Path, files: &[Value], message: &str) -> io::Result<String> {
    let branch = git(project, &["branch", "--show-current"])?;
    if branch.is_empty() {
        return Err(io::Error::other(
            "the project folder is on no branch (detached HEAD)",
        ));
    }
    if branch.starts_with("world/") {
        return Err(io::Error::other(
            "the project folder holds work in progress: commit or drop it first",
        ));
    }
    let paths: Vec<&str> = files.iter().filter_map(|f| f["file"].as_str()).collect();
    let mut args = vec!["status", "--porcelain", "--"];
    args.extend(&paths);
    if !git(project, &args)?.is_empty() {
        return Err(io::Error::other(
            "you have uncommitted changes on these files: commit or undo them first",
        ));
    }
    let written = write(project, files)?;
    let mut add = vec!["add", "--"];
    add.extend(written.iter().map(String::as_str));
    git(project, &add)?;
    let mut commit = vec!["commit", "-q", "-m", message, "--"];
    commit.extend(written.iter().map(String::as_str));
    git(project, &commit)?;
    git(project, &["rev-parse", "HEAD"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn project(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fennec-knowledge-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(ADR_DIR)).unwrap();
        std::fs::create_dir_all(dir.join("src/auth")).unwrap();
        std::fs::write(dir.join("src/auth/login.rs"), "").unwrap();
        std::fs::write(
            dir.join(ADR_DIR).join("0001-use-rust.md"),
            "# 1. Use Rust\n\nDate: 2026-01-01\nStatus: accepted\n",
        )
        .unwrap();
        std::fs::write(dir.join(ADR_DIR).join("README.md"), "how we write them").unwrap();
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.email", "t@t"],
            &["config", "user.name", "t"],
            &["-c", "user.email=t@t", "-c", "user.name=t", "add", "-A"],
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-qm",
                "init",
            ],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&dir)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        dir
    }

    #[test]
    fn only_knowledge_paths_are_knowledge() {
        assert!(is_knowledge("ARCHITECTURE.md"));
        assert!(is_knowledge(".fennec-os/zones.json"));
        assert!(is_knowledge("docs/adr/0002-x.md"));
        assert!(!is_knowledge("docs/adr/sub/0002-x.md"));
        assert!(!is_knowledge("docs/adr/../../src/main.rs"));
        assert!(!is_knowledge("src/main.rs"));
    }

    #[test]
    fn records_are_read_at_head_by_number() {
        let dir = project("read");
        let a = adrs(&dir).unwrap();
        assert_eq!(a.len(), 1, "the README is not a record");
        assert_eq!(
            (a[0].number, a[0].title.as_str(), a[0].status.as_str()),
            (1, "Use Rust", "accepted")
        );
        assert!(index(&a).contains("1. Use Rust — accepted"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_new_record_gets_the_kernels_number_and_supersedes_the_old() {
        let dir = project("new");
        let c = check(
            &dir,
            &[json!({"file": "docs/adr/0042-whatever.md",
                     "content": "# 9. Use Zig\n\nStatus: supersedes 1\n\n## Context\nfaster\n"})],
        )
        .unwrap();
        let files = c["files"].as_array().unwrap();
        assert_eq!(files[0]["file"], "docs/adr/0001-use-rust.md");
        assert!(
            files[0]["content"]
                .as_str()
                .unwrap()
                .contains("Status: superseded by 2")
        );
        assert_eq!(files[1]["file"], "docs/adr/0002-use-zig.md");
        let text = files[1]["content"].as_str().unwrap();
        assert!(text.starts_with("# 2. Use Zig\n"), "{text}");
        assert!(text.contains("\nDate: "), "a date is added: {text}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_proposal_outside_the_knowledge_or_a_broken_zones_file_is_refused() {
        let dir = project("refuse");
        assert!(check(&dir, &[json!({"file": "src/main.rs", "content": "x"})]).is_err());
        assert!(check(&dir, &[json!({"file": ZONES_FILE, "content": "{}"})]).is_err());
        let c = check(
            &dir,
            &[json!({"file": ZONES_FILE, "content":
                r#"{"zones": [{"name": "auth", "kind": "security", "paths": ["src/auth/**"]},
                              {"name": "pay", "paths": ["src/pay/**"]}]}"#})],
        )
        .unwrap();
        let checks = c["checks"].to_string();
        assert!(checks.contains("zone pay matches no file yet"), "{checks}");
        assert!(!checks.contains("zone auth matches"), "{checks}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn accepted_files_are_committed_alone() {
        let dir = project("commit");
        std::fs::write(dir.join("notes.txt"), "the dev's own work").unwrap();
        let files = vec![json!({"file": VISION_FILE, "content": "# vision\n"})];
        let head = commit(&dir, &files, "docs: adopt the project in fennec").unwrap();
        assert_eq!(head.len(), 40);
        let shown = git(&dir, &["show", "--name-only", "--format=%s", "HEAD"]).unwrap();
        assert!(
            shown.contains(VISION_FILE) && !shown.contains("notes.txt"),
            "{shown}"
        );
        assert!(
            dir.join("notes.txt").exists(),
            "the dev's file is untouched"
        );
        // The dev's own uncommitted change on a knowledge file is never overwritten.
        std::fs::write(dir.join(VISION_FILE), "mine").unwrap();
        assert!(commit(&dir, &files, "again").is_err());
        assert!(write(&dir, &[json!({"file": "src/x.rs", "content": ""})]).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn today_is_a_date() {
        let t = today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }
}
