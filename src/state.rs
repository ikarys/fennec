//! What the view shows: the lead's requests and projects, kept in files, plus
//! the runs fennec-os has recorded. Nothing here starts work; it only reads
//! and records.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fennec_os::world::Worlds;
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

    /// Adds a git repository; adding it twice is not an error.
    pub fn add_project(&self, path: &str) -> io::Result<()> {
        let path = std::fs::canonicalize(path)?;
        if !is_repository(&path) {
            return Err(io::Error::other("ce dossier n'est pas un dépôt git"));
        }
        let path = path.display().to_string();
        let mut projects = self.projects()?;
        if !projects.contains(&path) {
            projects.push(path);
            write_json(&self.root.join("projects.json"), &json!(projects))?;
        }
        Ok(())
    }

    pub fn add_request(&self, project: &str, text: &str) -> io::Result<String> {
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
        let record = json!({"project": project, "text": text.trim(), "status": "framing"});
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
    pub fn view(&self, worlds: &Worlds) -> io::Result<Value> {
        let runs = fennec_os::review::runs(worlds)?;
        let runs = runs.into_iter().map(|(_, r)| r).collect::<Vec<_>>();
        Ok(classify(&self.requests()?, &runs, &self.projects()?))
    }
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    // Write then rename: a reader never sees half a file.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
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

/// Sorts requests and fennec-os runs into the four columns. Pure, so the
/// mapping from internal states to what the human sees is tested here.
fn classify(requests: &[(String, Value)], runs: &[Value], projects: &[String]) -> Value {
    let (mut mine, mut ready, mut running, mut failed) = (vec![], vec![], vec![], vec![]);
    for (id, r) in requests {
        let card = |detail: &str| json!({"id": id, "project": name(r["project"].as_str().unwrap_or("")), "title": r["text"], "detail": detail});
        match r["status"].as_str() {
            Some("framing") => running.push(card("fennec prépare « terminé quand »")),
            Some("question") => {
                let done_when = r["done_when"].as_str().unwrap_or("");
                let mut c = card(&format!("Terminé quand : {done_when}"));
                c["checks"] = r["verify"].clone();
                c["ask"] = json!(true);
                mine.push(c);
            }
            Some("running") => running.push(card("fennec travaille")),
            // The tests passed; the product itself was not tried yet.
            Some("ready") => {
                let mut c = card("non vérifié : produit pas encore essayé");
                c["checks"] = r["verify"].clone();
                ready.push(c);
            }
            Some("failed") => failed.push(card(r["reason"].as_str().unwrap_or("raison inconnue"))),
            _ => {}
        }
    }
    // A request's run is shown through the request, not a second time.
    let owned: Vec<&Value> = requests
        .iter()
        .map(|(_, r)| &r["spec"])
        .filter(|s| s.is_string())
        .collect();
    for r in runs.iter().filter(|r| !owned.contains(&&r["spec"])) {
        let card = |detail: &str| json!({"project": name(r["project"].as_str().unwrap_or("")), "title": r["goal"], "detail": detail});
        match r["status"].as_str() {
            // fennec-os proves commands pass, not that the product works.
            Some("ready") => ready.push(card("non vérifié : produit pas encore essayé")),
            Some("failed") => {
                let why = r["attempts"]
                    .as_array()
                    .and_then(|a| a.iter().find_map(|t| t["reason"].as_str()))
                    .unwrap_or("raison inconnue");
                failed.push(card(why));
            }
            _ => {}
        }
    }
    let projects: Vec<Value> = projects
        .iter()
        .map(|p| json!({"path": p, "name": name(p)}))
        .collect();
    json!({"mine": mine, "ready": ready, "running": running, "failed": failed, "projects": projects})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_states_land_in_the_right_column() {
        let requests = [
            (
                "1".to_string(),
                json!({"project": "/p/a", "text": "x", "status": "framing"}),
            ),
            (
                "2".to_string(),
                json!({"project": "/p/a", "text": "y", "status": "question", "done_when": "Q", "spec": "s1"}),
            ),
        ];
        let runs = [
            json!({"project": "/p/b", "goal": "g1", "status": "ready"}),
            json!({"project": "/p/b", "goal": "g2", "status": "failed", "attempts": [{"passed": false}, {"reason": "timeout"}]}),
            json!({"project": "/p/b", "goal": "g3", "status": "landed"}),
            json!({"project": "/p/a", "goal": "g4", "status": "ready", "spec": "s1"}),
        ];
        let v = classify(&requests, &runs, &["/p/a".into()]);
        assert_eq!(v["running"][0]["title"], "x");
        assert_eq!(v["mine"][0]["detail"], "Terminé quand : Q");
        assert_eq!(v["mine"][0]["id"], "2");
        assert_eq!(v["ready"][0]["project"], "b");
        assert!(
            v["ready"][0]["detail"]
                .as_str()
                .unwrap()
                .starts_with("non vérifié")
        );
        assert_eq!(v["failed"][0]["detail"], "timeout");
        let shown = ["mine", "ready", "running", "failed"]
            .iter()
            .map(|k| v[*k].as_array().unwrap().len())
            .sum::<usize>();
        assert_eq!(
            shown, 4,
            "landed runs and runs owned by a request are not shown"
        );
        assert_eq!(v["projects"][0]["name"], "a");
    }
}
