//! The Fleet: tasks kept for later that fennec starts on its own, in one
//! queue across projects. The decision is the task's; the project is only a
//! place: where fennec works in the folder itself, one task at a time.
//! `fleet.json` in the store holds the queue's order, run or pause, and how
//! many tasks may work at once.

use std::io;
use std::path::Path;

use fennec_os::world::{self, Held};
use serde_json::{Value, json};

use crate::state::{Store, is_working};

fn path(store: &Store) -> std::path::PathBuf {
    store.root.join("fleet.json")
}

/// Run by default: only tasks the human put in the queue start. One at a
/// time by default: a local model server answers one request well.
fn load(store: &Store) -> io::Result<Value> {
    let mut v = match std::fs::read_to_string(path(store)) {
        Ok(text) => serde_json::from_str(&text).map_err(io::Error::other)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e),
    };
    if !v["on"].is_boolean() {
        v["on"] = json!(true);
    }
    if !v["at_once"].is_u64() {
        v["at_once"] = json!(1);
    }
    if !v["queue"].is_array() {
        v["queue"] = json!([]);
    }
    Ok(v)
}

fn save(store: &Store, v: &Value) -> io::Result<()> {
    crate::state::write_json(&path(store), v)
}

/// The queue as it stands: a task launched, forgotten or archived since has
/// left it, whatever the file still says.
pub fn queue(store: &Store) -> io::Result<Vec<(String, Value)>> {
    let v = load(store)?;
    let projects = store.projects()?;
    Ok(v["queue"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| {
            let id = id.as_str()?;
            let r = store.load(id).ok()?;
            (r["status"] == "later"
                && r["archived"] != true
                && projects.iter().any(|p| r["project"] == p.as_str()))
            .then(|| (id.to_string(), r))
        })
        .collect())
}

/// Put in the Fleet (at the end), taken out, or moved one place.
pub fn place(store: &Store, id: &str, action: &str) -> io::Result<()> {
    let mut ids: Vec<String> = queue(store)?.into_iter().map(|(id, _)| id).collect();
    let at = ids.iter().position(|x| x == id);
    match (action, at) {
        ("add", None) => {
            if store.load(id)?["status"] != "later" {
                return Err(io::Error::other("cette tâche n'est plus pour plus tard"));
            }
            ids.push(id.to_string());
        }
        ("add", Some(_)) => {}
        ("remove", Some(i)) => {
            ids.remove(i);
        }
        ("up", Some(i)) if i > 0 => ids.swap(i, i - 1),
        ("down", Some(i)) if i + 1 < ids.len() => ids.swap(i, i + 1),
        ("up" | "down", Some(_)) => {}
        (_, None) => return Err(io::Error::other("cette tâche n'est pas dans la Fleet")),
        _ => return Err(io::Error::other("action inconnue")),
    }
    let mut v = load(store)?;
    v["queue"] = json!(ids);
    save(store, &v)
}

/// Run or pause, and how many tasks at once (1 to 10).
pub fn set(store: &Store, on: Option<bool>, at_once: Option<u64>) -> io::Result<()> {
    let mut v = load(store)?;
    if let Some(on) = on {
        v["on"] = json!(on);
    }
    if let Some(n) = at_once {
        v["at_once"] = json!(n.clamp(1, 10));
    }
    save(store, &v)
}

/// Why `project`'s folder cannot take a task now, or `None` when it can.
fn held(project: &str, requests: &[(String, Value)]) -> Option<String> {
    let p = Path::new(project);
    match world::held(p) {
        Err(e) => return Some(format!("dossier illisible ({e})")),
        Ok(Some(Held::World(w))) => {
            let owner = requests.iter().find(|(_, r)| r["run"] == w.as_str());
            return Some(match owner {
                Some((_, r)) => {
                    // Its first words: the reason is a tag, not the task.
                    let title = crate::state::words(r["text"].as_str().unwrap_or(""));
                    let first = title.lines().next().unwrap_or("");
                    match first.char_indices().nth(40) {
                        Some((at, _)) => format!("« {}… » occupe le dossier", &first[..at]),
                        None => format!("« {first} » occupe le dossier"),
                    }
                }
                None => "une autre tâche occupe le dossier".into(),
            });
        }
        Ok(Some(Held::Changes)) => {
            return Some("le dossier a des changements pas encore commités".into());
        }
        Ok(None) => {}
    }
    // Still in conversation, the task has not checked the folder out yet.
    let here = world::workspace(p) == "here";
    let busy = requests
        .iter()
        .any(|(_, r)| r["project"] == project && is_working(r));
    (here && busy).then(|| "une autre tâche travaille dans ce projet".into())
}

/// Each queued task with what it waits for (`None`: it starts next), in order.
/// Pure, given what holds each folder, so the choice is tested here.
fn states(
    queue: &[(String, Value)],
    requests: &[(String, Value)],
    at_once: usize,
    held: impl Fn(&str) -> Option<String>,
) -> Vec<Option<String>> {
    let working = requests.iter().filter(|(_, r)| is_working(r)).count();
    let mut next = false;
    queue
        .iter()
        .map(|(_, r)| {
            if let Some(why) = held(r["project"].as_str().unwrap_or("")) {
                return Some(why);
            }
            if working >= at_once {
                return Some("attend une place".into());
            }
            // Only the first free one starts at the next look.
            if next {
                return Some("attend son tour dans la Fleet".into());
            }
            next = true;
            None
        })
        .collect()
}

/// One look: when running and a place is free, the first queued task whose
/// folder is free starts. `start` begins its work (`work::talk`).
pub fn tick(store: &Store, start: fn(String)) -> io::Result<()> {
    let v = load(store)?;
    if v["on"] != true {
        return Ok(());
    }
    let queue = queue(store)?;
    let requests = store.requests()?;
    let at_once = v["at_once"].as_u64().unwrap_or(1) as usize;
    let states = states(&queue, &requests, at_once, |p| held(p, &requests));
    if let Some(((id, _), _)) = queue.iter().zip(&states).find(|(_, s)| s.is_none()) {
        store.launch(id)?;
        start(id.clone());
    }
    Ok(())
}

/// The Fleet, looked at every 3 s for as long as fennec runs.
/// ponytail: polls; a task leaving its folder could wake it instead.
pub fn watch() {
    std::thread::spawn(|| {
        loop {
            if let Err(e) = Store::from_env().and_then(|s| tick(&s, crate::work::talk)) {
                eprintln!("fennec: la Fleet : {e}");
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}

/// What the Fleet page shows: run or pause, how many at once, the queue in
/// order with what each task waits for, and what works now.
pub fn view(store: &Store) -> io::Result<Value> {
    let v = load(store)?;
    let queue = queue(store)?;
    let requests = store.requests()?;
    let at_once = v["at_once"].as_u64().unwrap_or(1) as usize;
    let states = states(&queue, &requests, at_once, |p| held(p, &requests));
    let card = |id: &str, r: &Value| {
        let project = r["project"].as_str().unwrap_or("");
        json!({"id": id, "path": project, "title": crate::state::words(r["text"].as_str().unwrap_or("")),
               "project": Path::new(project).file_name().and_then(|n| n.to_str()).unwrap_or(project)})
    };
    let queued: Vec<Value> = queue
        .iter()
        .zip(&states)
        .map(|((id, r), s)| {
            let mut c = card(id, r);
            c["waits"] = json!(s);
            c
        })
        .collect();
    let working: Vec<Value> = requests
        .iter()
        .filter(|(_, r)| is_working(r))
        .map(|(id, r)| card(id, r))
        .collect();
    Ok(json!({"on": v["on"], "at_once": v["at_once"], "queue": queued, "working": working}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(project: &str, status: &str) -> Value {
        json!({"project": project, "status": status, "text": "t"})
    }

    #[test]
    fn the_first_free_task_starts_and_the_others_say_why_they_wait() {
        let queue = vec![
            ("a".to_string(), task("/p", "later")),
            ("b".to_string(), task("/q", "later")),
            ("c".to_string(), task("/r", "later")),
        ];
        let held = |p: &str| (p == "/p").then(|| "occupé".to_string());
        let s = states(&queue, &[], 1, held);
        assert_eq!(s[0].as_deref(), Some("occupé"));
        assert_eq!(s[1], None, "the first free folder goes first");
        assert_eq!(s[2].as_deref(), Some("attend son tour dans la Fleet"));
        // Every place taken: nothing starts, whatever the folders.
        let working = vec![("w".to_string(), task("/z", "running"))];
        let s = states(&queue, &working, 1, held);
        assert_eq!(s[1].as_deref(), Some("attend une place"));
        assert_eq!(states(&queue, &working, 2, held)[1], None);
    }

    #[test]
    fn fleet_tasks_start_in_order_one_per_look_and_pause_stops_them() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-fleet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        let (p, q) = (tmp.join("p"), tmp.join("q"));
        for dir in [&p, &q] {
            std::fs::create_dir_all(dir).unwrap();
            store.add_project(dir.to_str().unwrap(), "apart").unwrap();
        }
        let p = std::fs::canonicalize(p).unwrap().display().to_string();
        let q = std::fs::canonicalize(q).unwrap().display().to_string();
        let a = store.add_request(&p, "a", &[], true).unwrap();
        let b = store.add_request(&q, "b", &[], true).unwrap();
        let kept = store.add_request(&p, "kept", &[], true).unwrap();
        place(&store, &a, "add").unwrap();
        place(&store, &b, "add").unwrap();
        // The human moves b first; "kept" stays for later, out of the Fleet.
        place(&store, &b, "up").unwrap();
        set(&store, None, Some(2)).unwrap();
        tick(&store, |_| {}).unwrap();
        assert_eq!(store.load(&b).unwrap()["status"], "thinking");
        assert_eq!(store.load(&a).unwrap()["status"], "later");
        set(&store, Some(false), None).unwrap();
        tick(&store, |_| {}).unwrap();
        assert_eq!(store.load(&a).unwrap()["status"], "later", "paused");
        set(&store, Some(true), None).unwrap();
        tick(&store, |_| {}).unwrap();
        assert_eq!(store.load(&a).unwrap()["status"], "thinking");
        // Two work, two places: the queue is empty and "kept" never moved.
        assert!(queue(&store).unwrap().is_empty());
        tick(&store, |_| {}).unwrap();
        assert_eq!(store.load(&kept).unwrap()["status"], "later");
        assert!(place(&store, &a, "add").is_err(), "no longer for later");
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
