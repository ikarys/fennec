//! Skills: know-how a person already keeps as Agent Skills (a folder with a
//! `SKILL.md` whose front matter says `name` and `description`), read where
//! it lives: `~/.config/fennec/skills/`, Claude's (`~/.claude/skills/`, its
//! plugins) and the project's own `.claude/skills/`. The person enables the
//! ones any project's work may use, once for all (`~/.config/fennec/skills.json`,
//! like `model.json`). The agent sees each enabled skill's name and
//! description and reads the rest when the task calls for it: the folder is
//! mounted read-only in the sandbox under `~/.skills/<name>/`.

use std::io;
use std::path::{Path, PathBuf};

use crate::world::{World, Worlds, git};

#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub dir: PathBuf,
}

/// Every skill this machine and, if named, `project` offer, by name; the
/// project's own come first and win over a skill of the same name elsewhere.
pub fn catalog(project: Option<&Path>) -> io::Result<Vec<Skill>> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let roots: Vec<PathBuf> = match project {
        Some(project) => vec![project.join(".claude/skills")],
        None => vec![],
    }
    .into_iter()
    .chain([
        config.join("fennec/skills"),
        home.join(".claude/skills"),
        home.join(".claude/plugins/marketplaces"),
    ])
    .collect();
    let mut found = Vec::new();
    for root in roots {
        walk(&root, 6, &mut found);
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|s: &Skill| seen.insert(s.name.clone()));
    Ok(found)
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<Skill>) {
    if let Some(skill) = read(dir) {
        found.push(skill);
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            !n.starts_with('.') && !matches!(n.as_ref(), "cache" | "node_modules")
        })
        .map(|e| e.path())
        .collect();
    dirs.sort();
    if depth > 0 {
        dirs.iter().for_each(|d| walk(d, depth - 1, found));
    }
}

/// The skill in `dir`, from its `SKILL.md` front matter.
fn read(dir: &Path) -> Option<Skill> {
    let text = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let (name, description) = front_matter(&text);
    let name = name.or_else(|| Some(dir.file_name()?.to_string_lossy().into_owned()))?;
    Some(Skill {
        name,
        description: description.unwrap_or_default(),
        dir: dir.to_path_buf(),
    })
}

/// `name` and `description` from YAML front matter: one line each, or a
/// `>`/`|` block of indented lines.
/// ponytail: these two keys only, no YAML parser.
fn front_matter(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let body: Vec<&str> = lines.take_while(|l| l.trim() != "---").collect();
    let value = |key: &str| {
        let at = body
            .iter()
            .position(|l| l.strip_prefix(key).is_some_and(|r| r.starts_with(':')))?;
        let first = body[at][key.len() + 1..].trim();
        let text = if matches!(first, ">" | "|" | ">-" | "|-") {
            body[at + 1..]
                .iter()
                .take_while(|l| l.starts_with(' ') || l.is_empty())
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join(" ")
        } else if let Some(q) = first.strip_prefix('"').and_then(|f| f.strip_suffix('"')) {
            // YAML double quotes escape their own quote and backslash.
            q.replace("\\\"", "\"").replace("\\\\", "\\")
        } else if let Some(q) = first.strip_prefix('\'').and_then(|f| f.strip_suffix('\'')) {
            q.replace("''", "'")
        } else {
            first.to_string()
        };
        Some(text.trim().to_string()).filter(|t| !t.is_empty())
    };
    (value("name"), value("description"))
}

/// Where the person's global choice of skills lives: one list for every
/// project (`~/.config/fennec/skills.json`, like `model.json`).
fn store() -> io::Result<PathBuf> {
    Ok(crate::rules::config_dir()?.join("skills.json"))
}

/// The skill names the person enabled, for every project; none if the file
/// is not there yet.
pub fn enabled() -> io::Result<Vec<String>> {
    let text = match std::fs::read_to_string(store()?) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let names: Vec<String> = serde_json::from_str(&text)
        .map_err(|e| io::Error::other(format!("skills.json est illisible : {e}")))?;
    Ok(names)
}

/// The skills the person enabled, in the catalogue; an enabled one that is
/// gone from the machine is left out.
pub fn chosen(project: Option<&Path>) -> io::Result<Vec<Skill>> {
    let names = enabled()?;
    Ok(catalog(project)?
        .into_iter()
        .filter(|s| names.contains(&s.name))
        .collect())
}

/// Enables or disables a skill, for every project.
pub fn choose(name: &str, on: bool) -> io::Result<()> {
    let mut names = enabled()?;
    match (on, names.iter().any(|n| n == name)) {
        (true, false) => names.push(name.to_string()),
        (false, true) => names.retain(|n| n != name),
        _ => return Ok(()),
    }
    let path = store()?;
    std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
    std::fs::write(&path, serde_json::to_string_pretty(&names)?)?;
    Ok(())
}

/// The old per-project choice (`git config fennec.skill`) moves to the
/// global list, once: the union is kept, the per-project values are dropped.
/// Reading and dropping the old values is lenient; only writing the list is
/// not, so a run over again changes nothing.
pub fn migrate(projects: &[&Path]) -> io::Result<()> {
    let mut names = enabled()?;
    for project in projects {
        let old = git(project, &["config", "--get-all", "fennec.skill"]).unwrap_or_default();
        if old.is_empty() {
            continue;
        }
        for line in old.lines() {
            if !names.iter().any(|n| n == line) {
                names.push(line.to_string());
            }
        }
        git(project, &["config", "--unset-all", "fennec.skill"]).ok();
    }
    let text = serde_json::to_string_pretty(&names)?;
    let path = store()?;
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
        std::fs::write(&path, text)?;
    }
    Ok(())
}

/// What the agent is told about the chosen skills.
pub fn brief(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut text = String::from(
        "\n\nSkills the person keeps for this kind of work. When the task calls for one, read its \
         instructions first (`cat ~/.skills/<name>/SKILL.md`) and follow them; its other files are \
         in the same folder:\n",
    );
    for s in skills {
        text.push_str(&format!("  - {}: {}\n", s.name, s.description));
    }
    text
}

/// Mounts `skills` in `world`'s sandbox (`Worlds::sandbox_command` reads this).
pub fn give(worlds: &Worlds, world: &World, skills: &[Skill]) -> io::Result<()> {
    if skills.is_empty() {
        return Ok(());
    }
    let dir = worlds.root.join("skills");
    std::fs::create_dir_all(&dir)?;
    let lines: String = skills
        .iter()
        .map(|s| format!("{}\t{}\n", s.name, s.dir.display()))
        .collect();
    std::fs::write(dir.join(&world.id), lines)
}

/// The skills mounted in world `id`: name and host folder.
pub fn mounted(worlds: &Worlds, id: &str) -> Vec<(String, PathBuf)> {
    std::fs::read_to_string(worlds.root.join("skills").join(id))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(n, d)| (n.to_string(), PathBuf::from(d)))
        .collect()
}

/// The skills a run read, from its tool calls: what the journal and the
/// card say was used.
pub fn used(messages: &[serde_json::Value]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for m in messages {
        for call in m["tool_calls"].as_array().into_iter().flatten() {
            let args = call["function"]["arguments"].as_str().unwrap_or("");
            for part in args.split(".skills/").skip(1) {
                let name: String = part
                    .chars()
                    .take_while(|c| !matches!(c, '/' | '"' | ' ' | '\'' | '\\'))
                    .collect();
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::{choose, chosen, enabled, front_matter, migrate, used};
    use serde_json::json;
    use std::sync::Mutex;

    // The global list and the skill roots read process-wide variables; the
    // tests move them, so they hold the lock while doing so.
    static ENV: Mutex<()> = Mutex::new(());

    /// A home and a config dir of the test, empty; the lock stays held until
    /// the test ends.
    fn sandbox() -> (std::sync::MutexGuard<'static, ()>, std::path::PathBuf) {
        let held = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("fennec-os-skill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("HOME", &dir);
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
        }
        (held, dir)
    }

    /// A skill folder the catalogue finds, under the config dir.
    fn make_skill(dir: &std::path::Path, name: &str) {
        let folder = dir.join("config").join("fennec/skills").join(name);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: The {name} skill.\n---\nbody"),
        )
        .unwrap();
    }

    #[test]
    fn front_matter_reads_one_line_and_block_values() {
        let (n, d) = front_matter("---\nname: pdf\ndescription: \"Read PDFs.\"\n---\nbody");
        assert_eq!(
            (n.as_deref(), d.as_deref()),
            (Some("pdf"), Some("Read PDFs."))
        );
        let (n, d) = front_matter("---\nname: x\ndescription: >\n  Two\n  lines.\nother: y\n---\n");
        assert_eq!(
            (n.as_deref(), d.as_deref()),
            (Some("x"), Some("Two lines."))
        );
        let (_, d) = front_matter("---\nname: b\ndescription: \"the \\\"own\\\" one\"\n---\n");
        assert_eq!(d.as_deref(), Some("the \"own\" one"));
        let (_, d) = front_matter("---\nname: c\ndescription: 'it''s'\n---\n");
        assert_eq!(d.as_deref(), Some("it's"));
        assert_eq!(front_matter("no front matter"), (None, None));
    }

    #[test]
    fn used_skills_come_from_the_paths_read() {
        let messages = [json!({"tool_calls": [
            {"function": {"arguments": "{\"command\":\"cat ~/.skills/pdf/SKILL.md\"}"}},
            {"function": {"arguments": "{\"path\":\"/home/x/.skills/docx/scripts/a.py\"}"}},
            {"function": {"arguments": "{\"command\":\"cat ~/.skills/pdf/forms.md\"}"}}
        ]})];
        assert_eq!(used(&messages), ["pdf", "docx"]);
    }

    #[test]
    fn enable_and_disable_write_the_global_list() {
        let (_held, dir) = sandbox();
        assert_eq!(enabled().unwrap(), Vec::<String>::new());
        choose("pdf", true).unwrap();
        choose("docx", true).unwrap();
        choose("docx", true).unwrap();
        assert_eq!(enabled().unwrap(), vec!["pdf", "docx"]);
        choose("pdf", false).unwrap();
        choose("pdf", false).unwrap();
        assert_eq!(enabled().unwrap(), vec!["docx"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chosen_reads_the_global_list_against_the_catalogue() {
        let (_held, dir) = sandbox();
        make_skill(&dir, "pdf");
        make_skill(&dir, "docx");
        choose("pdf", true).unwrap();
        let names: Vec<String> = chosen(None).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["pdf"]);
        choose("pdf", false).unwrap();
        assert!(chosen(None).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_moves_the_per_project_choice_to_the_global_list() {
        let (_held, dir) = sandbox();
        let project = dir.join("proj");
        std::fs::create_dir_all(&project).unwrap();
        crate::world::git(&project, &["init", "-q"]).unwrap();
        crate::world::git(&project, &["config", "--add", "fennec.skill", "pdf"]).unwrap();
        crate::world::git(&project, &["config", "--add", "fennec.skill", "docx"]).unwrap();
        migrate(&[project.as_path()]).unwrap();
        assert_eq!(enabled().unwrap(), vec!["pdf", "docx"]);
        let left = crate::world::git(&project, &["config", "--get-all", "fennec.skill"])
            .unwrap_or_default();
        assert_eq!(left, "");
        // Run again over the now-empty project: nothing moves.
        migrate(&[project.as_path()]).unwrap();
        assert_eq!(enabled().unwrap(), vec!["pdf", "docx"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
