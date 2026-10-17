//! What the view shows: the lead's requests and projects, kept in files, plus
//! the runs fennec-os has recorded. Nothing here starts work; it only reads
//! and records.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

pub struct Store {
    pub root: PathBuf,
}

impl Store {
    /// `FENNEC_LEAD_HOME`, default `~/.local/share/fennec-lead`.
    pub fn from_env() -> io::Result<Self> {
        let root = match std::env::var_os("FENNEC_LEAD_HOME") {
            Some(v) => PathBuf::from(v),
            None => PathBuf::from(
                std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?,
            )
            .join(".local/share/fennec-lead"),
        };
        std::fs::create_dir_all(root.join("requests"))?;
        Ok(Self { root })
    }

    pub fn projects(&self) -> io::Result<Vec<String>> {
        match std::fs::read_to_string(self.root.join("projects.json")) {
            Ok(text) => serde_json::from_str(&text).map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Adds a git repository; adding it twice is not an error. An empty
    /// folder becomes a new project; see [`start`].
    /// Adds a project, or changes where fennec works in one already added:
    /// `here` (the folder itself) or `apart`.
    pub fn add_project(&self, path: &str, place: &str) -> io::Result<()> {
        let path = std::fs::canonicalize(path)?;
        start(&path)?;
        fennec_os::world::choose_place(&path, place)?;
        let path = path.display().to_string();
        let mut projects = self.projects()?;
        if !projects.contains(&path) {
            projects.push(path);
            write_json(&self.root.join("projects.json"), &json!(projects))?;
        }
        Ok(())
    }

    pub fn add_request(&self, project: &str, text: &str, off: &[String]) -> io::Result<String> {
        if !self.projects()?.iter().any(|p| p == project) {
            return Err(io::Error::other("projet inconnu"));
        }
        if text.trim().is_empty() {
            return Err(io::Error::other("demande vide"));
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        // Same width hex timestamps, as in fennec-os: lexical order is time order.
        let id = format!("{nanos:032x}");
        // Every request opens a conversation; a clear one ends at its first turn.
        let record = json!({"project": project, "text": text.trim(), "status": "thinking", "off": off,
                            "thread": [{"who": "human", "text": text.trim()}]});
        write_json(&self.request_path(&id)?, &record)?;
        Ok(id)
    }

    fn request_path(&self, id: &str) -> io::Result<PathBuf> {
        // Ids come from the page: hex only, so none can name another file.
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(io::Error::other("demande inconnue"));
        }
        Ok(self.root.join("requests").join(format!("{id}.json")))
    }

    /// The exchange of the step under way, rewritten after every model call.
    pub fn live_path(&self, id: &str) -> io::Result<PathBuf> {
        self.request_path(id)?;
        let dir = self.root.join("live");
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{id}.json")))
    }

    pub fn load(&self, id: &str) -> io::Result<Value> {
        let text = std::fs::read_to_string(self.request_path(id)?)
            .map_err(|_| io::Error::other("demande inconnue"))?;
        serde_json::from_str(&text).map_err(io::Error::other)
    }

    pub fn update(&self, id: &str, change: impl FnOnce(&mut Value)) -> io::Result<()> {
        let mut record = self.load(id)?;
        change(&mut record);
        write_json(&self.request_path(id)?, &record)
    }

    /// Every request, oldest first.
    pub fn requests(&self) -> io::Result<Vec<(String, Value)>> {
        let mut ids: Vec<String> = std::fs::read_dir(self.root.join("requests"))?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_suffix(".json")
                    .map(str::to_string)
            })
            .collect();
        ids.sort();
        ids.into_iter()
            .map(|id| self.load(&id).map(|r| (id, r)))
            .collect()
    }

    /// The single view: every project, sorted by what needs the human.
    /// Runs fennec-os made outside a request are not shown: nothing here
    /// could act on them.
    pub fn view(&self) -> io::Result<Value> {
        Ok(classify(&self.requests()?, &self.projects()?))
    }
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    // Write then rename: a reader never sees half a file.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

pub fn is_empty(path: &Path) -> io::Result<bool> {
    Ok(std::fs::read_dir(path)?.next().is_none())
}

/// fennec works from commits: a project gets a repository if it has none
/// (an empty folder; a folder with files is refused, committing someone's
/// files is their call) and an empty first commit if it has none (a `git init`
/// by hand). The commit is made from the empty tree, so what the human has
/// staged stays staged and nothing of theirs is committed.
pub fn start(path: &Path) -> io::Result<()> {
    let git = |args: &[&str]| -> io::Result<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            // Never a parent folder's repository (a home can be one).
            .env("GIT_CEILING_DIRECTORIES", path.parent().unwrap_or(path))
            .stdin(std::process::Stdio::null())
            .output()?;
        if !out.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    if !is_repository(path) {
        if !is_empty(path)? {
            return Err(io::Error::other(
                "ce dossier contient des fichiers mais n'est pas un dépôt git",
            ));
        }
        git(&["init", "-q"])?;
    }
    if git(&["rev-parse", "-q", "--verify", "HEAD"]).is_err() {
        let tree = git(&["mktree"])?;
        let commit = git(&["commit-tree", &tree, "-m", "chore: start project"])?;
        git(&["update-ref", "HEAD", &commit])?;
    }
    Ok(())
}

/// Whether a request's ready work sits in the project folder itself.
fn in_folder(request: &Value) -> bool {
    let worlds = fennec_os::world::Worlds::from_env();
    let world = request["run"]
        .as_str()
        .and_then(|run| worlds.ok()?.open(run).ok());
    let canonical = |p: &Path| std::fs::canonicalize(p).ok();
    world.is_some_and(|w| {
        canonical(&w.path) == canonical(Path::new(request["project"].as_str().unwrap_or("")))
    })
}

pub fn is_repository(path: &Path) -> bool {
    path.join(".git").exists()
}

fn name(project: &str) -> &str {
    Path::new(project)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(project)
}

/// What the proof could not show: the model's lines, then each role check
/// whose tool this machine lacks (rule 3: said, not left out).
fn unverified(proof: &Value) -> Value {
    let mut lines: Vec<Value> = proof["unverified"].as_array().cloned().unwrap_or_default();
    for s in proof["shots"].as_array().into_iter().flatten() {
        if s["missing"] == true {
            lines.push(json!(format!(
                "fennec n'a pas pu regarder {} : aucun navigateur n'est installé",
                s["url"].as_str().unwrap_or("la page")
            )));
        }
    }
    for c in proof["checks"].as_array().into_iter().flatten() {
        if c["missing"] == true {
            lines.push(json!(format!(
                "{} n'a pas pu lancer `{}` : l'outil n'est pas installé",
                c["role"].as_str().unwrap_or(""),
                c["command"].as_str().unwrap_or("")
            )));
        }
    }
    json!(lines)
}

/// Sorts requests into what needs the human, what fennec is doing, and what
/// was landed. Pure, so the mapping from internal states to what the human
/// sees is tested here. Each card carries fields, not prose: the page decides
/// what is shown at once and what waits behind "details".
fn classify(requests: &[(String, Value)], projects: &[String]) -> Value {
    let (mut todo, mut running, mut history) = (vec![], vec![], vec![]);
    for (id, r) in requests {
        let card = |kind: &str| {
            json!({"id": id, "kind": kind, "project": name(r["project"].as_str().unwrap_or("")),
                   "title": r["text"], "rules": r["rules"], "checks": r["verify"], "team": r["team"],
                   "thread": r["thread"], "stats": r["stats"]})
        };
        // Scenario 4: what the human said may become a rule, at one word.
        if let Some(rule) = r["offer"]["rule"].as_str() {
            let mut c = card("rule");
            c["rule"] = json!(rule);
            c["heard"] = r["offer"]["heard"].clone();
            todo.push((0, c));
        }
        match r["status"].as_str() {
            Some("thinking") => running.push(card_step(card("running"), "réfléchit à ta demande")),
            Some("talk") => todo.push((0, card("talk"))),
            Some("framing") => running.push(card_step(card("running"), "se met au travail")),
            Some("running") => running.push(card_step(card("running"), "travaille")),
            Some("proving") => running.push(card_step(card("running"), "essaie le produit")),
            Some("landing") => running.push(card_step(card("running"), "fait le commit")),
            Some("question") => {
                let mut c = card("validate");
                c["done_when"] = r["done_when"].clone();
                todo.push((0, c));
            }
            Some("blocked") => {
                let mut c = card("blocked");
                c["question"] = r["question"].clone();
                c["reason"] = r["reason"].clone();
                todo.push((0, c));
            }
            Some("ready") => {
                let mut c = card("ready");
                c["done_when"] = r["done_when"].clone();
                c["unverified"] = unverified(&r["proof"]);
                c["tries"] = r["proof"]["tries"].clone();
                c["role_checks"] = r["proof"]["checks"].clone();
                c["shots"] = r["proof"]["shots"].clone();
                c["land_error"] = r["land_error"].clone();
                c["here"] = json!(in_folder(r));
                c["amend_error"] = r["amend_error"].clone();
                c["summary"] = r["summary"].clone();
                c["files"] = r["files"].clone();
                // A run that predates the proof was never tried: say so.
                c["untried"] = json!(
                    r["proof"]["tries"].as_array().is_none_or(Vec::is_empty)
                        && r["proof"]["shots"].as_array().is_none_or(Vec::is_empty)
                );
                if r["run"].is_string() {
                    // Without a target, the click only prepares one.
                    let t = &r["target"];
                    c["land"] = json!({
                        "branch": t["branch"],
                        "remote": t["remote"],
                        "message": t["message"],
                        "committed": r["commit"].is_string(),
                    });
                }
                todo.push((1, c));
            }
            Some("failed") => {
                let mut c = card("failed");
                c["reason"] = r["reason"].clone();
                c["tries"] = r["proof"]["tries"].clone();
                c["role_checks"] = r["proof"]["checks"].clone();
                c["shots"] = r["proof"]["shots"].clone();
                todo.push((2, c));
            }
            // Finished one way or another: kept, so the human is never blind
            // to what was asked, done and proven.
            Some(outcome @ ("landed" | "passed" | "dropped")) => {
                let mut c = card(outcome);
                c["branch"] = r["target"]["branch"].clone();
                c["remote"] = r["target"]["remote"].clone();
                c["commit"] = r["commit"].clone();
                c["changed"] = json!(r["run"].is_string());
                for k in ["done_when", "summary", "files", "question", "reason"] {
                    c[k] = r[k].clone();
                }
                c["tries"] = r["proof"]["tries"].clone();
                c["role_checks"] = r["proof"]["checks"].clone();
                c["shots"] = r["proof"]["shots"].clone();
                c["unverified"] = unverified(&r["proof"]);
                history.push(c);
            }
            _ => {}
        }
    }
    // Answers unblock fennec, so they come first; then results, then failures.
    todo.sort_by_key(|(rank, _)| *rank);
    let todo: Vec<Value> = todo.into_iter().map(|(_, c)| c).collect();
    history.reverse();
    // ponytail: the last 50; older requests stay on disk, unseen.
    history.truncate(50);
    let projects: Vec<Value> = projects
        .iter()
        .map(|p| json!({"path": p, "name": name(p)}))
        .collect();
    json!({"todo": todo, "running": running, "history": history, "projects": projects})
}

fn card_step(mut card: Value, step: &str) -> Value {
    card["step"] = json!(step);
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_folder_becomes_a_project_and_loose_files_are_refused() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-new-{}", std::process::id()));
        let (new, loose) = (tmp.join("new"), tmp.join("loose"));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::create_dir_all(&loose).unwrap();
        std::fs::write(loose.join("notes.txt"), "mine").unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(&store.root).unwrap();
        store.add_project(new.to_str().unwrap(), "here").unwrap();
        // Where fennec works is the human's choice, among two.
        assert!(store.add_project(new.to_str().unwrap(), "nowhere").is_err());
        assert!(is_repository(&new));
        let head = std::process::Command::new("git")
            .args(["-C", new.to_str().unwrap(), "rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(head.status.success(), "a first commit to fork from");
        // A repository made by hand, with no commit yet and a file staged.
        let bare = tmp.join("bare");
        std::fs::create_dir_all(&bare).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(&bare)
                .args(args)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        std::fs::write(bare.join("staged.txt"), "mine").unwrap();
        git(&["add", "staged.txt"]);
        store.add_project(bare.to_str().unwrap(), "here").unwrap();
        assert_eq!(git(&["log", "--format=%s"]), "chore: start project");
        assert_eq!(
            git(&["ls-tree", "-r", "--name-only", "HEAD"]),
            "",
            "nothing of theirs committed"
        );
        assert_eq!(
            git(&["diff", "--cached", "--name-only"]),
            "staged.txt",
            "still staged"
        );
        start(&bare).unwrap();
        assert_eq!(git(&["rev-list", "--count", "HEAD"]), "1", "only once");
        assert!(store.add_project(loose.to_str().unwrap(), "here").is_err());
        assert!(!is_repository(&loose));
        assert_eq!(store.projects().unwrap().len(), 2);
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_tool_a_role_could_not_run_is_said_unverified() {
        let proof = json!({"unverified": ["le GUI"], "shots": [{"url": "http://127.0.0.1:8000/", "missing": true}], "checks": [
            {"role": "DevSecOps", "command": "gitleaks detect", "exit": 127, "missing": true},
            {"role": "DevSecOps", "command": "ok", "exit": 0, "missing": false}]});
        assert_eq!(
            unverified(&proof),
            json!([
                "le GUI",
                "fennec n'a pas pu regarder http://127.0.0.1:8000/ : aucun navigateur n'est installé",
                "DevSecOps n'a pas pu lancer `gitleaks detect` : l'outil n'est pas installé"
            ])
        );
    }

    #[test]
    fn what_needs_the_human_comes_first_answers_before_results() {
        let requests = [
            (
                "1".to_string(),
                json!({"project": "/p/a", "text": "x", "status": "framing"}),
            ),
            (
                "2".to_string(),
                json!({"project": "/p/a", "text": "z", "status": "ready", "run": "r1", "land_error": "conflit", "rules": ["use uv"], "offer": {"rule": "never touch develop", "heard": "pas develop !"}}),
            ),
            (
                "3".to_string(),
                json!({"project": "/p/a", "text": "y", "status": "question", "done_when": "Q"}),
            ),
            (
                "4".to_string(),
                json!({"project": "/p/a", "text": "f", "status": "failed", "reason": "timeout"}),
            ),
            (
                "5".to_string(),
                json!({"project": "/p/a", "text": "l", "status": "landed", "commit": "c1", "target": {"branch": "feat/l", "remote": "origin"}}),
            ),
            (
                "6".to_string(),
                json!({"project": "/p/a", "text": "d", "status": "dropped"}),
            ),
        ];
        let v = classify(&requests, &["/p/a".into()]);
        let kinds: Vec<&str> = v["todo"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["rule", "validate", "ready", "failed"]);
        assert_eq!(v["todo"][1]["done_when"], "Q");
        let ready = &v["todo"][2];
        assert_eq!(ready["untried"], true);
        assert_eq!(ready["land_error"], "conflit");
        assert_eq!(ready["rules"][0], "use uv");
        assert_eq!(ready["land"]["branch"], Value::Null);
        assert_eq!(v["running"][0]["step"], "se met au travail");
        let history: Vec<&str> = v["history"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["kind"].as_str().unwrap())
            .collect();
        assert_eq!(history, ["dropped", "landed"], "newest first, none lost");
        assert_eq!(v["history"][1]["branch"], "feat/l");
        assert_eq!(v["projects"][0]["name"], "a");
    }
}
