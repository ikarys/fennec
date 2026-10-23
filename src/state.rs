//! What the view shows: the lead's requests and projects, kept in files, plus
//! the runs fennec-os has recorded. Nothing here starts work; it only reads
//! and records.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Held by every `Store::update`: a request's read-modify-write is atomic
/// against the other threads (workers, plugins) and the HTTP handlers.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    /// The projects in view: the archived ones are out of the way.
    pub fn projects(&self) -> io::Result<Vec<String>> {
        Ok(self
            .projects_all()?
            .into_iter()
            .filter(|p| p["archived"] != true)
            .map(|p| p["path"].as_str().unwrap_or_default().to_string())
            .collect())
    }

    /// Every project, archived or not. `projects.json` used to be a plain
    /// list of paths: those entries read as not archived.
    pub fn projects_all(&self) -> io::Result<Vec<Value>> {
        match std::fs::read_to_string(self.root.join("projects.json")) {
            Ok(text) => {
                let v: Value = serde_json::from_str(&text).map_err(io::Error::other)?;
                Ok(v.as_array()
                    .map(|items| {
                        items
                            .iter()
                            .map(|p| match p {
                                Value::String(path) => json!({"path": path, "archived": false}),
                                other => other.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
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
        let mut all = self.projects_all()?;
        if !all.iter().any(|p| p["path"] == path) {
            all.push(json!({"path": path, "archived": false}));
            write_json(&self.root.join("projects.json"), &json!(all))?;
        }
        Ok(())
    }

    /// Out of view, or back: the folder and its requests are untouched, only
    /// the list the page reads moves. A project fennec works on stays put.
    pub fn set_project_archived(&self, path: &str, on: bool) -> io::Result<()> {
        let mut all = self.projects_all()?;
        let Some(p) = all.iter_mut().find(|p| p["path"] == path) else {
            return Err(io::Error::other("unknown project"));
        };
        if on
            && self
                .requests()?
                .iter()
                .any(|(_, r)| r["project"] == path && is_working(r))
        {
            return Err(io::Error::other("fennec still works on this project"));
        }
        p["archived"] = json!(on);
        write_json(&self.root.join("projects.json"), &json!(all))
    }

    /// Unregistered from fennec; the folder, its git and its team files stay.
    /// Its requests are its own: none may be left behind.
    pub fn remove_project(&self, path: &str) -> io::Result<()> {
        let mut all = self.projects_all()?;
        if !all.iter().any(|p| p["path"] == path) {
            return Err(io::Error::other("unknown project"));
        }
        if self.requests()?.iter().any(|(_, r)| r["project"] == path) {
            return Err(io::Error::other(
                "this project has tasks: delete them first",
            ));
        }
        all.retain(|p| p["path"] != path);
        write_json(&self.root.join("projects.json"), &json!(all))
    }

    /// A finished request, out of the lists; it comes back when asked. A
    /// running one is not hidden: fennec would work in the dark.
    pub fn archive_request(&self, id: &str, on: bool) -> io::Result<()> {
        let request = self.load(id)?;
        match request["status"].as_str() {
            Some("later" | "landed" | "passed" | "dropped") => {}
            _ => {
                return Err(io::Error::other("fennec still works on this task"));
            }
        }
        self.update(id, |r| r["archived"] = json!(on))
    }

    /// A new request; `later` keeps it in the backlog, nothing started.
    pub fn add_request(
        &self,
        project: &str,
        text: &str,
        title: &str,
        off: &[String],
        later: bool,
    ) -> io::Result<String> {
        if !self.projects()?.iter().any(|p| p == project) {
            return Err(io::Error::other("unknown project"));
        }
        if text.trim().is_empty() {
            return Err(io::Error::other("empty task"));
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        // Same width hex timestamps, as in fennec-os: lexical order is time order.
        let id = format!("{nanos:032x}");
        // Every request opens a conversation; a clear one ends at its first turn.
        let status = if later { "later" } else { "thinking" };
        let mut record = json!({"project": project, "text": text.trim(), "status": status, "off": off,
                            "since": nanos / 1_000_000_000, "thread": [{"who": "human", "text": text.trim()}]});
        // A short label of its own, apart from the task's content.
        let title = title.trim();
        if !title.is_empty() {
            record["title"] = json!(title);
        }
        write_json(&self.request_path(&id)?, &record)?;
        Ok(id)
    }

    /// A request from the backlog, worked on now; the caller starts the work.
    /// An image the human attached, kept under `images/` and named after its
    /// content. The text carries it as a line of its own, which fennec-os
    /// sends to the model as an image (`provider::with_images`).
    pub fn save_image(&self, data_url: &str) -> io::Result<String> {
        let (head, data) = data_url
            .split_once(";base64,")
            .ok_or_else(|| io::Error::other("image illisible"))?;
        let ext = match head.strip_prefix("data:image/") {
            Some("png") => "png",
            Some("jpeg") => "jpg",
            Some("gif") => "gif",
            Some("webp") => "webp",
            _ => {
                return Err(io::Error::other(
                    "image en png, jpeg, gif ou webp seulement",
                ));
            }
        };
        let bytes = unbase64(data).ok_or_else(|| io::Error::other("image illisible"))?;
        let mut hash = std::hash::DefaultHasher::new();
        std::hash::Hash::hash(&bytes, &mut hash);
        let dir = self.root.join("images");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{:016x}.{ext}", std::hash::Hasher::finish(&hash)));
        std::fs::write(&path, &bytes)?;
        Ok(format!("![image](file://{})", path.display()))
    }

    /// An attached image, by the name `save_image` gave it.
    pub fn image(&self, name: &str) -> io::Result<Vec<u8>> {
        let valid = name.split_once('.').is_some_and(|(stem, ext)| {
            stem.len() == 16
                && stem.bytes().all(|b| b.is_ascii_hexdigit())
                && ["png", "jpg", "gif", "webp"].contains(&ext)
        });
        if !valid {
            return Err(io::Error::other("bad name"));
        }
        std::fs::read(self.root.join("images").join(name))
    }

    pub fn launch(&self, id: &str) -> io::Result<()> {
        self.later(id)?;
        self.update(id, |r| r["status"] = json!("thinking"))
    }

    /// The backlog's task, said again before it starts. The content and its
    /// short label are separate: rewording the content re-frames, relabelling
    /// does not.
    pub fn rewrite(&self, id: &str, text: &str, title: &str) -> io::Result<()> {
        self.later(id)?;
        let before = self.load(id)?;
        if text.trim().is_empty() {
            return Err(io::Error::other("empty task"));
        }
        let words_changed = before["text"]
            .as_str()
            .map(|t| t.trim() != text.trim())
            .unwrap_or(true);
        self.update(id, |r| {
            r["text"] = json!(text.trim());
            r["thread"] = json!([{"who": "human", "text": text.trim()}]);
            // The label of its own, apart from the content.
            let title = title.trim();
            if title.is_empty() {
                r.as_object_mut().map(|m| m.remove("title"));
            } else {
                r["title"] = json!(title);
            }
            // Framed ahead on the old words: framed again on the new ones.
            if words_changed {
                let Some(m) = r.as_object_mut() else {
                    return;
                };
                for k in [
                    "ahead",
                    "asked",
                    "ahead_failed",
                    "done_when",
                    "split_error",
                    "picked",
                    "roles",
                ] {
                    m.remove(k);
                }
            }
        })
    }

    /// Out of the backlog for good: nothing was done, nothing to keep. A
    /// finished request goes the same way; a passed one had kept its work
    /// for the commit, so that goes too.
    pub fn forget(&self, id: &str) -> io::Result<()> {
        let request = self.load(id)?;
        match request["status"].as_str() {
            Some("later") => {}
            Some("landed" | "passed" | "dropped") => {
                let worlds = fennec_os::world::Worlds::from_env()?;
                if let Some(run) = request["run"].as_str()
                    && worlds.open(run).is_ok()
                {
                    fennec_os::review::reject(&worlds, run, None)?;
                }
                if let Some(s) = request["spec"].as_str()
                    && worlds.open(s).is_ok()
                {
                    fennec_os::spec::reject(&worlds, s, None)?;
                }
            }
            _ => {
                return Err(io::Error::other("fennec still works on this task"));
            }
        }
        // The step's exchange too: a forgotten request leaves no trace.
        let _ = std::fs::remove_file(self.root.join("live").join(format!("{id}.json")));
        std::fs::remove_file(self.request_path(id)?)
    }

    fn later(&self, id: &str) -> io::Result<()> {
        if self.load(id)?["status"] != "later" {
            return Err(io::Error::other("this task is no longer for later"));
        }
        Ok(())
    }

    fn request_path(&self, id: &str) -> io::Result<PathBuf> {
        // Ids come from the page: hex only, so none can name another file.
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(io::Error::other("unknown task"));
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
            .map_err(|_| io::Error::other("unknown task"))?;
        serde_json::from_str(&text).map_err(io::Error::other)
    }

    pub fn update(&self, id: &str, change: impl FnOnce(&mut Value)) -> io::Result<()> {
        let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut record = self.load(id)?;
        let was = record["status"].clone();
        change(&mut record);
        let moved = record["status"] != was;
        // Why a task failed or stopped means nothing once it works again: a
        // landed card kept a failed commit's error otherwise.
        if moved
            && !matches!(
                record["status"].as_str(),
                Some("failed" | "blocked" | "dropped")
            )
            && let Some(m) = record.as_object_mut()
        {
            m.remove("reason");
        }
        // When the current step began: a running line says for how long.
        if moved {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            record["since"] = json!(secs);
        }
        write_json(&self.request_path(id)?, &record)?;
        // Only a change of status is news; the Journal written after a
        // plugin failed is not, or it would call the plugin again.
        if let Some(e) = event(was.as_str().unwrap_or(""), id, &record).filter(|_| moved) {
            self.tell(id, e);
        }
        Ok(())
    }

    /// The enabled plugins hear of a status change, in their own thread: a
    /// slow or broken one never holds the work. A failure is written in the
    /// request's Journal.
    fn tell(&self, id: &str, event: Value) {
        // Tests never reach the person's real plugins.
        if cfg!(test) {
            return;
        }
        let (store, id) = (
            Store {
                root: self.root.clone(),
            },
            id.to_string(),
        );
        std::thread::spawn(move || {
            let failed: Vec<Value> = match fennec_os::plugin::emit(&event) {
                Ok(said) => said
                    .into_iter()
                    .filter_map(|(name, r)| r.err().map(|e| (name, e)))
                    .map(|(name, e)| json!({"stage": "plugin", "name": name, "error": e}))
                    .collect(),
                Err(e) => vec![json!({"stage": "plugin", "name": "", "error": e.to_string()})],
            };
            if failed.is_empty() {
                return;
            }
            let _ = store.update(&id, |r| {
                if !r["journal"].is_array() {
                    r["journal"] = json!([]);
                }
                if let Some(j) = r["journal"].as_array_mut() {
                    j.extend(failed);
                }
            });
        });
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
        let mut view = classify(&self.requests()?, &self.projects_all()?);
        // A task for later is in the Backlog, or Ready at its place in line.
        let fleet: Vec<String> = crate::fleet::queue(self)?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        for c in view["later"].as_array_mut().into_iter().flatten() {
            let rank = fleet.iter().position(|id| c["id"] == id.as_str());
            c["fleet"] = json!(rank.is_some());
            c["rank"] = json!(rank);
        }
        // The list starts and stops the Fleet too, and sees why it stopped.
        view["fleet"] = json!({"on": crate::fleet::running(self)?, "queued": fleet.len(),
                               "stopped": crate::fleet::stopped(self)?});
        Ok(view)
    }
}

/// The lines of a text that attach an image (see `Store::save_image`).
pub fn image_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .filter(|l| l.trim().starts_with("![image](file://"))
}

/// The plugin event a status change is, if any, with what a message needs:
/// the task's first line and, in French, what happened to it.
fn event(was: &str, id: &str, r: &Value) -> Option<Value> {
    let now = r["status"].as_str().unwrap_or("");
    let title = words(r["text"].as_str().unwrap_or(""))
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let (name, message) = match now {
        "running" => ("started", format!("fennec starts \"{title}\"")),
        "talk" | "question" | "blocked" => {
            // What is asked: a stopped step's question, the conversation's
            // last word, or the "done when" to accept.
            let last = r["thread"]
                .as_array()
                .and_then(|t| t.iter().rev().find(|t| t["who"] == "fennec"))
                .map(|t| &t["text"]);
            let ask = match now {
                "blocked" => &r["question"],
                "question" => &r["done_when"],
                _ => last.unwrap_or(&Value::Null),
            };
            let ask = ask.as_str().unwrap_or("");
            (
                "question",
                format!("fennec has a question on \"{title}\": {ask}"),
            )
        }
        // Back from a commit tried: the human is the one who just clicked.
        "ready" if was == "landing" => return None,
        "ready" => ("ready", format!("\"{title}\" is ready to validate")),
        "failed" => (
            "failed",
            format!("\"{title}\" failed: {}", r["reason"].as_str().unwrap_or("")),
        ),
        "landed" => ("landed", format!("\"{title}\" is committed")),
        _ => return None,
    };
    let project = r["project"].as_str().unwrap_or("");
    // The page's address for the task, so a message can open it.
    let addr = std::env::var("FENNEC_LEAD_ADDR").unwrap_or_else(|_| "127.0.0.1:7777".into());
    let path: String = project
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    Some(json!({
        "url": format!("http://{addr}/#{path}/{id}"),
        "event": name, "task": id, "title": title, "message": message,
        "project": project,
        "project_name": Path::new(project).file_name().map(|n| n.to_string_lossy()),
        "status": now,
    }))
}

/// What the human wrote, without its image lines: a title.
pub(crate) fn words(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim().starts_with("![image](file://"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn unbase64(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=').as_bytes();
    // A base64 body is a multiple of 4: a length of 4n+1 is not a padding of
    // a valid one, so it is refused, not decoded.
    if text.len() % 4 == 1 {
        return None;
    }
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    for chunk in text.chunks(4) {
        let n = chunk.iter().enumerate().try_fold(0u32, |n, (i, c)| {
            Some(n | (value(*c)? as u32) << (18 - 6 * i))
        })?;
        out.extend_from_slice(&n.to_be_bytes()[1..chunk.len()]);
    }
    Some(out)
}

pub(crate) fn write_json(path: &Path, value: &Value) -> io::Result<()> {
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
            // fennec's own bookkeeping commit, signed by fennec: the human's
            // identity is not required for a project to start.
            .args(["-c", "user.name=fennec", "-c", "user.email=fennec@fennec"])
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
                "this folder holds files but is not a git repository",
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

/// A step fennec is in right now: the project may not be taken out of view.
pub(crate) fn is_working(request: &Value) -> bool {
    matches!(
        request["status"].as_str(),
        Some("thinking" | "framing" | "waiting" | "running" | "reviewing" | "proving" | "landing")
    )
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
                "fennec could not look at {}: no browser is installed",
                s["url"].as_str().unwrap_or("the page")
            )));
        }
        if s["unchanged"] == true {
            lines.push(json!(format!(
                "nothing changed on screen at {} between before and after: the capture does not show the work",
                s["url"].as_str().unwrap_or("the page")
            )));
        }
    }
    for c in proof["checks"].as_array().into_iter().flatten() {
        if c["missing"] == true {
            lines.push(json!(format!(
                "{} could not run `{}`: the tool is not installed",
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
fn classify(requests: &[(String, Value)], projects_all: &[Value]) -> Value {
    let projects: Vec<String> = projects_all
        .iter()
        .filter(|p| p["archived"] != true)
        .map(|p| p["path"].as_str().unwrap_or_default().to_string())
        .collect();
    // Only a registered project's archiving hides its requests; a request
    // from an unregistered folder is shown as before.
    let hidden: Vec<String> = projects_all
        .iter()
        .filter(|p| p["archived"] == true)
        .map(|p| p["path"].as_str().unwrap_or_default().to_string())
        .collect();
    let (mut todo, mut running, mut history, mut later, mut archived) =
        (vec![], vec![], vec![], vec![], vec![]);
    for (id, r) in requests {
        // An archived project's requests are out of view with it; they come
        // back when the project does.
        if hidden
            .iter()
            .any(|p| *p == r["project"].as_str().unwrap_or(""))
        {
            continue;
        }
        // The task's content, and its short label of its own: older tasks
        // have no label, so the content stands in for it.
        let body = words(r["text"].as_str().unwrap_or(""));
        let label = r["title"]
            .as_str()
            .filter(|t| !t.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| body.clone());
        let card = |kind: &str| {
            json!({"id": id, "kind": kind, "project": name(r["project"].as_str().unwrap_or("")), "path": r["project"],
                   "title": label, "description": body,
                   "images": image_lines(r["text"].as_str().unwrap_or("")).collect::<Vec<_>>(), "rules": r["rules"], "checks": r["verify"], "team": r["team"],
                   "thread": r["thread"], "chat": r["chat"], "stats": r["stats"], "since": r["since"], "picked": r["picked"],
                   // As the work reads it: said in the conversation, else
                   // the request as fennec restated it.
                   "done_when": ([&r["done_when"], &r["brief"], &r["ahead"]].into_iter().find(|v| v.is_string()))})
        };
        // Scenario 4: what the human said may become a rule, at one word.
        if let Some(rule) = r["offer"]["rule"].as_str() {
            let mut c = card("rule");
            c["rule"] = json!(rule);
            c["heard"] = r["offer"]["heard"].clone();
            todo.push((0, c));
        }
        match r["status"].as_str() {
            Some("later") => {
                if r["archived"] == true {
                    archived.push(card("later"));
                } else if r["asked"] == true {
                    // Framed ahead, it asked: answered now, it stays for later.
                    let mut c = card("talk");
                    c["later"] = json!(true);
                    todo.push((0, c));
                } else {
                    let mut c = card("later");
                    c["framed"] = json!(r["ahead"].is_string());
                    c["ahead_failed"] = json!(r["ahead_failed"] == true);
                    c["splitting"] = json!(r["splitting"] == true);
                    c["split_error"] = r["split_error"].clone();
                    // What it builds on, not committed yet: the Fleet waits.
                    let after: Vec<String> = crate::fleet::blockers(r, requests)
                        .into_iter()
                        .map(crate::fleet::short)
                        .collect();
                    c["after"] = json!(after);
                    later.push(c);
                }
            }
            Some("thinking") => running.push(card_step(card("running"), "thinks about your task")),
            Some("talk") => todo.push((0, card("talk"))),
            Some("framing") => running.push(card_step(card("running"), "gets to work")),
            Some("waiting") => {
                let step = match r["behind"].as_str() {
                    Some(by) if !by.is_empty() => {
                        format!("waits its turn: {by} holds the folder")
                    }
                    _ => "waits its turn: the folder has changes not committed yet".to_string(),
                };
                // Queued, not working: the UI reads it as idle, not active.
                let mut c = card_step(card("running"), &step);
                c["waiting"] = json!(true);
                running.push(c)
            }
            Some("running") => running.push(card_step(card("running"), "works")),
            Some("reviewing") => running.push(card_step(card("running"), "reviews the work")),
            Some("proving") => running.push(card_step(card("running"), "tries the product")),
            Some("landing") => running.push(card_step(card("running"), "makes the commit")),
            Some("question") => {
                let c = card("validate");
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
                c["unverified"] = unverified(&r["proof"]);
                c["tries"] = r["proof"]["tries"].clone();
                c["role_checks"] = r["proof"]["checks"].clone();
                c["shots"] = r["proof"]["shots"].clone();
                c["land_error"] = r["land_error"].clone();
                c["here"] = json!(in_folder(r));
                c["page"] = json!(r["proof"]["serve"].is_string());
                c["amend_error"] = r["amend_error"].clone();
                c["summary"] = r["summary"].clone();
                c["files"] = r["files"].clone();
                c["skills"] = r["skills"].clone();
                c["disagreements"] = r["disagreements"].clone();
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
                for k in ["summary", "files", "skills"] {
                    c[k] = r[k].clone();
                }
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
                for k in ["summary", "files", "skills", "question", "reason"] {
                    c[k] = r[k].clone();
                }
                c["tries"] = r["proof"]["tries"].clone();
                c["role_checks"] = r["proof"]["checks"].clone();
                c["shots"] = r["proof"]["shots"].clone();
                c["unverified"] = unverified(&r["proof"]);
                if r["archived"] == true {
                    archived.push(c);
                } else {
                    history.push(c);
                }
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
    archived.reverse();
    // Whether a worker still owns it: "Réessayer" is only for a dead one.
    for c in &mut running {
        c["alive"] = json!(crate::work::working(c["id"].as_str().unwrap_or("")));
    }
    let projects: Vec<Value> = projects
        .iter()
        .map(|p| json!({"path": p, "name": name(p)}))
        .collect();
    let projects_archived: Vec<Value> = projects_all
        .iter()
        .filter(|p| p["archived"] == true)
        .map(|p| {
            let path = p["path"].as_str().unwrap_or_default();
            json!({"path": path, "name": name(path)})
        })
        .collect();
    json!({"todo": todo, "running": running, "history": history, "later": later,
           "archived": archived, "projects": projects, "projectsArchived": projects_archived})
}

fn card_step(mut card: Value, step: &str) -> Value {
    card["step"] = json!(step);
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_changes_become_plugin_events() {
        let r = |status: &str| {
            json!({"project": "/p/my app", "text": "Add filters\nmore", "status": status,
                   "question": "Which port?", "reason": "timeout",
                   "thread": [{"who": "fennec", "text": "Hi?"}]})
        };
        let name = |was: &str, now: &str| event(was, "7", &r(now)).map(|e| e["event"].clone());
        assert_eq!(name("framing", "running"), Some(json!("started")));
        assert_eq!(name("running", "blocked"), Some(json!("question")));
        assert_eq!(name("proving", "ready"), Some(json!("ready")));
        assert_eq!(name("landing", "ready"), None, "the human just clicked");
        assert_eq!(name("running", "proving"), None);
        let e = event("running", "7", &r("blocked")).unwrap();
        assert_eq!(
            e["message"],
            "fennec has a question on \"Add filters\": Which port?"
        );
        assert_eq!(e["project_name"], "my app");
        assert!(e["url"].as_str().unwrap().ends_with("/#%2Fp%2Fmy%20app/7"));
        let e = event("thinking", "7", &r("talk")).unwrap();
        assert_eq!(
            e["message"],
            "fennec has a question on \"Add filters\": Hi?"
        );
        let e = event("proving", "7", &r("failed")).unwrap();
        assert_eq!(e["message"], "\"Add filters\" failed: timeout");
    }

    #[test]
    fn an_attached_image_is_kept_and_named_by_a_line() {
        let root = std::env::temp_dir().join(format!("fennec-lead-image-{}", std::process::id()));
        let store = Store { root: root.clone() };
        assert_eq!(unbase64("TWFu").unwrap(), b"Man");
        assert_eq!(unbase64("TWE=").unwrap(), b"Ma");
        assert!(
            unbase64("TWFuX").is_none(),
            "a length of 4n+1 is not base64"
        );
        let line = store.save_image("data:image/png;base64,TWFu").unwrap();
        let name = line.rsplit('/').next().unwrap().trim_end_matches(')');
        assert_eq!(store.image(name).unwrap(), b"Man");
        assert!(store.image("../requests.png").is_err());
        assert!(store.save_image("data:text/html;base64,TWFu").is_err());
        let text = format!("vois ça\n{line}");
        assert_eq!(words(&text), "vois ça");
        assert_eq!(image_lines(&text).collect::<Vec<_>>(), [line.as_str()]);
        std::fs::remove_dir_all(root).unwrap();
    }

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
    fn a_request_kept_for_later_waits_until_launched_or_forgotten() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-later-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        store
            .add_project(project.to_str().unwrap(), "here")
            .unwrap();
        let p = store.projects().unwrap()[0].clone();
        let a = store.add_request(&p, "dark mode", "", &[], true).unwrap();
        let b = store.add_request(&p, "export", "", &[], true).unwrap();
        let view = store.view().unwrap();
        assert_eq!(view["later"].as_array().unwrap().len(), 2);
        assert!(view["running"].as_array().unwrap().is_empty());

        store.rewrite(&a, "  dark mode, auto  ", "").unwrap();
        assert_eq!(store.load(&a).unwrap()["text"], "dark mode, auto");
        assert!(store.rewrite(&a, " ", "").is_err());
        store.launch(&a).unwrap();
        assert_eq!(store.load(&a).unwrap()["status"], "thinking");
        // Started, it is no longer the backlog's to change.
        assert!(store.rewrite(&a, "x", "").is_err() && store.forget(&a).is_err());
        store.forget(&b).unwrap();
        assert!(store.load(&b).is_err());
        assert!(
            store.view().unwrap()["later"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn the_list_sees_the_fleet_and_why_it_stopped() {
        let tmp =
            std::env::temp_dir().join(format!("fennec-lead-fleet-stopped-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        store
            .add_project(project.to_str().unwrap(), "here")
            .unwrap();
        let view = store.view().unwrap();
        assert_eq!(view["fleet"]["on"], true);
        assert_eq!(view["fleet"]["queued"], 0);
        assert!(view["fleet"]["stopped"].is_null());
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn a_finished_request_can_be_forgotten_a_running_one_cannot() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-forget-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        store
            .add_project(project.to_str().unwrap(), "here")
            .unwrap();
        let p = store.projects().unwrap()[0].clone();
        let landed = store.add_request(&p, "dark mode", "", &[], true).unwrap();
        let running = store.add_request(&p, "export", "", &[], true).unwrap();
        store
            .update(&landed, |r| r["status"] = json!("landed"))
            .unwrap();
        store
            .update(&running, |r| r["status"] = json!("thinking"))
            .unwrap();
        // Work in progress is not forgotten: it would write its result over the decision.
        assert!(store.forget(&running).is_err());
        store.forget(&landed).unwrap();
        assert!(store.load(&landed).is_err());
        assert_eq!(store.load(&running).unwrap()["status"], "thinking");
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn a_tool_a_role_could_not_run_is_said_unverified() {
        let proof = json!({"unverified": ["le GUI"], "shots": [{"url": "http://127.0.0.1:8000/", "missing": true}, {"url": "http://x/", "unchanged": true}], "checks": [
            {"role": "DevSecOps", "command": "gitleaks detect", "exit": 127, "missing": true},
            {"role": "DevSecOps", "command": "ok", "exit": 0, "missing": false}]});
        assert_eq!(
            unverified(&proof),
            json!([
                "le GUI",
                "fennec could not look at http://127.0.0.1:8000/: no browser is installed",
                "nothing changed on screen at http://x/ between before and after: the capture does not show the work",
                "DevSecOps could not run `gitleaks detect`: the tool is not installed"
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
                json!({"project": "/p/a", "text": "f", "status": "failed", "reason": "timeout", "brief": "B", "summary": "S"}),
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
        let v = classify(&requests, &[json!({"path": "/p/a", "archived": false})]);
        let kinds: Vec<&str> = v["todo"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["rule", "validate", "ready", "failed"]);
        assert_eq!(v["todo"][1]["done_when"], "Q");
        // Without "done when" said, it is the request as fennec restated it.
        assert_eq!(v["todo"][3]["done_when"], "B");
        assert_eq!(v["todo"][3]["summary"], "S");
        let ready = &v["todo"][2];
        assert_eq!(ready["untried"], true);
        assert_eq!(ready["land_error"], "conflit");
        assert_eq!(ready["rules"][0], "use uv");
        assert_eq!(ready["land"]["branch"], Value::Null);
        assert_eq!(v["running"][0]["step"], "gets to work");
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

    #[test]
    fn a_waiting_request_says_what_it_waits_for() {
        let requests = vec![
            (
                "1".into(),
                json!({"status": "waiting", "behind": "\"filtres\""}),
            ),
            ("2".into(), json!({"status": "waiting", "behind": ""})),
        ];
        let v = classify(&requests, &[]);
        assert_eq!(
            v["running"][0]["step"],
            "waits its turn: \"filtres\" holds the folder"
        );
        assert!(
            v["running"][1]["step"]
                .as_str()
                .unwrap()
                .contains("not committed yet")
        );
    }

    #[test]
    fn a_task_that_works_again_drops_why_it_failed() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-reason-{}", std::process::id()));
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        let id = "abc1";
        write_json(
            &store.request_path(id).unwrap(),
            &json!({"status": "running", "project": "/p", "text": "t"}),
        )
        .unwrap();
        let fail = |r: &mut Value| {
            r["status"] = json!("failed");
            r["reason"] = json!("git add failed");
        };
        store.update(id, fail).unwrap();
        assert_eq!(store.load(id).unwrap()["reason"], "git add failed");
        store.update(id, |r| r["status"] = json!("ready")).unwrap();
        store.update(id, |r| r["status"] = json!("landed")).unwrap();
        assert!(store.load(id).unwrap()["reason"].is_null());
        // A dropped task keeps why it had failed, for its history.
        store.update(id, fail).unwrap();
        store
            .update(id, |r| r["status"] = json!("dropped"))
            .unwrap();
        assert_eq!(store.load(id).unwrap()["reason"], "git add failed");
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn an_archived_project_hides_its_requests_and_comes_back() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-parch-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        store
            .add_project(project.to_str().unwrap(), "here")
            .unwrap();
        let p = store.projects().unwrap()[0].clone();
        let a = store.add_request(&p, "dark mode", "", &[], true).unwrap();
        store.update(&a, |r| r["status"] = json!("landed")).unwrap();
        assert_eq!(
            store.view().unwrap()["history"].as_array().unwrap().len(),
            1
        );
        store.set_project_archived(&p, true).unwrap();
        assert!(store.projects().unwrap().is_empty());
        assert_eq!(store.projects_all().unwrap().len(), 1);
        let v = store.view().unwrap();
        assert!(v["history"].as_array().unwrap().is_empty());
        assert!(v["later"].as_array().unwrap().is_empty());
        assert_eq!(v["projectsArchived"][0]["name"], "p");
        store.set_project_archived(&p, false).unwrap();
        assert_eq!(store.projects().unwrap(), vec![p]);
        assert_eq!(
            store.view().unwrap()["history"].as_array().unwrap().len(),
            1
        );
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_project_is_removed_only_when_it_has_no_requests() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-prem-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        store
            .add_project(project.to_str().unwrap(), "here")
            .unwrap();
        let p = store.projects().unwrap()[0].clone();
        // With a request, even a forgotten one, the project is not removed.
        let a = store.add_request(&p, "dark mode", "", &[], true).unwrap();
        let err = store.remove_project(&p).unwrap_err().to_string();
        assert!(err.contains("tasks"), "{err}");
        store.forget(&a).unwrap();
        store.remove_project(&p).unwrap();
        assert!(store.projects_all().unwrap().is_empty());
        // The folder stays a repository: untouched, only unregistered.
        assert!(is_repository(&project));
        assert!(store.remove_project("/inconnu").is_err());
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_working_project_cannot_be_archived() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-pwork-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        store
            .add_project(project.to_str().unwrap(), "here")
            .unwrap();
        let p = store.projects().unwrap()[0].clone();
        let a = store.add_request(&p, "dark mode", "", &[], false).unwrap();
        assert_eq!(store.load(&a).unwrap()["status"], "thinking");
        let err = store
            .set_project_archived(&p, true)
            .unwrap_err()
            .to_string();
        assert!(err.contains("works"), "{err}");
        assert_eq!(store.projects().unwrap(), vec![p]);
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn an_archived_request_leaves_the_lists_and_comes_back() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-rarch-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        store
            .add_project(project.to_str().unwrap(), "here")
            .unwrap();
        let p = store.projects().unwrap()[0].clone();
        let a = store.add_request(&p, "dark mode", "", &[], true).unwrap();
        let b = store.add_request(&p, "export", "", &[], true).unwrap();
        store.update(&a, |r| r["status"] = json!("landed")).unwrap();
        store
            .update(&b, |r| r["status"] = json!("thinking"))
            .unwrap();
        // A running request is not archived: fennec would work in the dark.
        assert!(store.archive_request(&b, true).is_err());
        store.archive_request(&a, true).unwrap();
        let v = store.view().unwrap();
        assert!(v["history"].as_array().unwrap().is_empty());
        let arch = v["archived"].as_array().unwrap();
        assert_eq!(arch.len(), 1);
        assert_eq!(arch[0]["kind"], "landed");
        assert_eq!(arch[0]["id"], a);
        store.archive_request(&a, false).unwrap();
        assert_eq!(
            store.view().unwrap()["history"].as_array().unwrap().len(),
            1
        );
        assert!(
            store.view().unwrap()["archived"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
