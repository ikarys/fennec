//! The Fleet: the Ready tasks fennec starts on its own, in one queue across
//! projects. A task enters it only when the human validated what fennec
//! understood of it (framed ahead while in the Backlog). The decision is the task's; the project is only a
//! place. Its work always goes apart, whatever the project chose: a task
//! ready and waiting for the human never holds the folder from the next.
//! `fleet.json` in the store holds the queue's order, run or stop, how many
//! tasks may work at once, and whether a failure stops it. A task the Fleet
//! started carries `fleet: true`, for its summary.

use std::io;
use std::path::Path;

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
    // A failure stops the Fleet by default: a model server down would
    // otherwise fail the whole queue, one task after the other.
    if !v["on_fail"].is_string() {
        v["on_fail"] = json!("stop");
    }
    // Proven work waits for the human by default; "branch" commits it on
    // the project's `fleet` branch, where the next task starts from.
    if !v["on_success"].is_string() {
        v["on_success"] = json!("wait");
    }
    Ok(v)
}

fn save(store: &Store, v: &Value) -> io::Result<()> {
    crate::state::write_json(&path(store), v)
}

/// The queue as it stands: a task launched, forgotten, archived or reworded
/// (its framing dropped) since has left it, whatever the file still says.
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
                && r["ahead"].is_string()
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
            let r = store.load(id)?;
            if r["status"] != "later" {
                return Err(io::Error::other("this task is no longer for later"));
            }
            // Ready means the human read what fennec understood.
            if !r["ahead"].is_string() {
                return Err(io::Error::other(
                    "fennec has not restated this task yet: validate it once prepared",
                ));
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
        (_, None) => return Err(io::Error::other("this task is not in the Fleet")),
        _ => return Err(io::Error::other("action inconnue")),
    }
    let mut v = load(store)?;
    v["queue"] = json!(ids);
    save(store, &v)
}

/// The branch the Fleet commits on, in each project, when told to.
pub const BRANCH: &str = "fleet";

/// Run or stop, how many tasks at once (1 to 10), what a failure does
/// (`stop` or `go on`) and what proven work does (`wait` or `branch`).
pub fn set(
    store: &Store,
    on: Option<bool>,
    at_once: Option<u64>,
    on_fail: Option<&str>,
    on_success: Option<&str>,
) -> io::Result<()> {
    let mut v = load(store)?;
    if let Some(on) = on {
        v["on"] = json!(on);
        // Started again: the reason it stopped is the past.
        if let Some(m) = v.as_object_mut() {
            m.remove("stopped");
        }
    }
    if let Some(f) = on_fail {
        if !matches!(f, "stop" | "go on") {
            return Err(io::Error::other("unknown setting"));
        }
        v["on_fail"] = json!(f);
    }
    if let Some(f) = on_success {
        if !matches!(f, "wait" | "branch") {
            return Err(io::Error::other("unknown setting"));
        }
        v["on_success"] = json!(f);
    }
    if let Some(n) = at_once {
        v["at_once"] = json!(n.clamp(1, 10));
    }
    save(store, &v)
}

/// A task named in a line: its first words.
pub(crate) fn short(r: &Value) -> String {
    let title = crate::state::words(r["text"].as_str().unwrap_or(""));
    let first = title.lines().next().unwrap_or("");
    match first.char_indices().nth(40) {
        Some((at, _)) => format!("{}…", &first[..at]),
        None => first.to_string(),
    }
}

/// The tasks `r` builds on (its `after`) not committed yet. A task deleted
/// since holds nothing back.
pub(crate) fn blockers<'a>(r: &Value, requests: &'a [(String, Value)]) -> Vec<&'a Value> {
    r["after"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| requests.iter().find(|(id, _)| a == id.as_str()))
        .map(|(_, d)| d)
        .filter(|d| d["status"] != "landed")
        .collect()
}

/// Whether the Fleet runs.
pub fn running(store: &Store) -> io::Result<bool> {
    Ok(load(store)?["on"] == true)
}

/// Why the Fleet stopped on its own, if it did; nothing once it runs again.
pub fn stopped(store: &Store) -> io::Result<Value> {
    Ok(load(store)?["stopped"].clone())
}

/// Each queued task with what it waits for (`None`: it starts next), in order.
/// The folder is not one of them: the Fleet's work goes apart (`work`).
fn states(
    queue: &[(String, Value)],
    requests: &[(String, Value)],
    at_once: usize,
) -> Vec<Option<String>> {
    let working = requests.iter().filter(|(_, r)| is_working(r)).count();
    let mut next = false;
    queue
        .iter()
        .map(|(_, r)| {
            // Started before what it builds on is committed, it would work
            // on code that is not there and conflict with it later.
            if let Some(d) = blockers(r, requests).first() {
                return Some(format!("waits for \"{}\"", short(d)));
            }
            if working >= at_once {
                return Some("waits for a slot".into());
            }
            // Only the first free one starts at the next look.
            if next {
                return Some("waits its turn in the Fleet".into());
            }
            next = true;
            None
        })
        .collect()
}

/// One look: when running, proven work meant for the `fleet` branch is
/// committed there first (`land`, `work::land`), so the next task starts
/// from it; else, when a place is free, the first queued task starts
/// (`start`, `work::talk`). One step per look. When nothing works, even
/// stopped, the first queued task not framed yet is framed ahead
/// (`prepare`, `work::prepare`).
pub fn tick(
    store: &Store,
    start: fn(String),
    land: fn(&Store, &str) -> io::Result<()>,
    prepare: fn(String),
) -> io::Result<()> {
    let mut v = load(store)?;
    let requests = store.requests()?;
    if v["on"] != true {
        return ahead(store, &requests, prepare);
    }
    // A failure is seen once; it stops the Fleet unless told to go on. A
    // commit on the branch that failed (a conflict, a check) is one too.
    for (id, r) in &requests {
        let failed = r["status"] == "failed"
            || (r["fleet_land"] == true && r["status"] == "ready" && r["land_error"].is_string());
        if r["fleet"] == true && failed && r["fleet_seen"] != true {
            store.update(id, |r| r["fleet_seen"] = json!(true))?;
            if v["on_fail"] == "stop" {
                v["on"] = json!(false);
                v["stopped"] = json!(match r["status"].as_str() {
                    Some("failed") => format!("\"{}\" failed", short(r)),
                    _ => format!("\"{}\" could not be committed on {BRANCH}", short(r)),
                });
                return save(store, &v);
            }
        }
    }
    if let Some((id, r)) = requests.iter().find(|(_, r)| {
        r["fleet_branch"].is_string() && r["status"] == "ready" && r["fleet_land"] != true
    }) {
        let branch = r["fleet_branch"].clone();
        store.update(id, |r| {
            r["fleet_land"] = json!(true);
            r["target"] = json!({"branch": branch});
        })?;
        return land(store, id);
    }
    let queue = queue(store)?;
    let at_once = v["at_once"].as_u64().unwrap_or(1) as usize;
    let states = states(&queue, &requests, at_once);
    if let Some(((id, _), _)) = queue.iter().zip(&states).find(|(_, s)| s.is_none()) {
        store.launch(id)?;
        let branch = (v["on_success"] == "branch").then_some(BRANCH);
        store.update(id, |r| {
            r["fleet"] = json!(true);
            // A failure seen on an earlier run must not hide this one's.
            if let Some(m) = r.as_object_mut() {
                m.remove("fleet_seen");
            }
            if let Some(b) = branch {
                r["fleet_branch"] = json!(b);
            }
        })?;
        start(id.clone());
        return Ok(());
    }
    ahead(store, &requests, prepare)
}

/// Framing ahead takes the model only when no task needs it: the Backlog's
/// first task not framed yet, so the human finds it ready to validate.
fn ahead(store: &Store, requests: &[(String, Value)], prepare: fn(String)) -> io::Result<()> {
    if requests.iter().any(|(_, r)| is_working(r)) || crate::work::busy() {
        return Ok(());
    }
    let projects = store.projects()?;
    let framed = |r: &Value| {
        ["ahead", "asked", "ahead_failed"]
            .iter()
            .any(|k| !r[k].is_null())
    };
    if let Some((id, _)) = requests.iter().find(|(_, r)| {
        r["status"] == "later"
            && r["archived"] != true
            && !framed(r)
            && projects.iter().any(|p| r["project"] == p.as_str())
    }) {
        prepare(id.clone());
    }
    Ok(())
}

/// The Fleet, looked at every 3 s for as long as fennec runs.
/// ponytail: polls; a task finishing could wake it instead.
pub fn watch() {
    std::thread::spawn(|| {
        loop {
            if let Err(e) = Store::from_env().and_then(|s| {
                tick(
                    &s,
                    crate::work::talk,
                    crate::work::land,
                    crate::work::prepare,
                )
            }) {
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
    let states = states(&queue, &requests, at_once);
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
    // What the Fleet started: where each stands, what it cost, the latest first.
    let started: Vec<&(String, Value)> = requests
        .iter()
        .filter(|(_, r)| r["fleet"] == true)
        .collect();
    let count = |states: &[&str]| {
        started
            .iter()
            .filter(|(_, r)| states.contains(&r["status"].as_str().unwrap_or("")))
            .count()
    };
    let mut cost = json!({"ms": 0, "sent": 0, "received": 0});
    for (_, r) in &started {
        for k in ["ms", "sent", "received"] {
            cost[k] = json!(cost[k].as_u64().unwrap_or(0) + r["stats"][k].as_u64().unwrap_or(0));
        }
    }
    let mut done: Vec<&&(String, Value)> = started.iter().filter(|(_, r)| !is_working(r)).collect();
    done.sort_by_key(|(_, r)| std::cmp::Reverse(r["since"].as_u64().unwrap_or(0)));
    let recent: Vec<Value> = done
        .iter()
        .take(10)
        .map(|(id, r)| {
            let mut c = card(id, r);
            c["outcome"] = json!(match r["status"].as_str() {
                Some("landed") if r["fleet_branch"].is_string() => "committed on fleet",
                Some("landed") => "committed",
                Some("passed") => "set aside",
                Some("dropped") => "dropped",
                Some("failed") => "failed",
                Some("ready") => "ready, waits for you",
                Some("later") => "put off for later",
                _ => "a question for you",
            });
            c
        })
        .collect();
    let summary = json!({
        "queued": queue.len(), "working": count(&["thinking", "framing", "waiting", "running", "reviewing", "proving", "landing"]),
        "yours": count(&["ready", "question", "talk", "blocked"]), "landed": count(&["landed"]),
        "failed": count(&["failed"]), "cost": cost,
    });
    Ok(
        json!({"on": v["on"], "at_once": v["at_once"], "on_fail": v["on_fail"], "on_success": v["on_success"], "stopped": v["stopped"],
              "queue": queued, "working": working, "summary": summary, "recent": recent}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(project: &str, status: &str) -> Value {
        json!({"project": project, "status": status, "text": "t"})
    }

    /// What framing ahead leaves: a brief the human can validate.
    fn framed(store: &Store, id: &str) {
        store.update(id, |r| r["ahead"] = json!("brief")).unwrap();
    }

    #[test]
    fn the_first_task_starts_and_the_others_say_why_they_wait() {
        let queue = vec![
            ("a".to_string(), task("/p", "later")),
            ("b".to_string(), task("/p", "later")),
        ];
        let s = states(&queue, &[], 1);
        assert_eq!(s[0], None, "the first goes first, whatever its folder");
        assert_eq!(s[1].as_deref(), Some("waits its turn in the Fleet"));
        // Every place taken: nothing starts.
        // b builds on a: it waits for a's commit, and c may go first.
        let mut b = task("/p", "later");
        b["after"] = json!(["a"]);
        let queue2 = vec![("b".to_string(), b), ("c".to_string(), task("/p", "later"))];
        let mut a = task("/p", "running");
        a["text"] = json!("the api");
        let s = states(&queue2, &[("a".to_string(), a.clone())], 2);
        assert_eq!(s[0].as_deref(), Some("waits for \"the api\""));
        assert_eq!(s[1], None);
        a["status"] = json!("landed");
        assert_eq!(states(&queue2, &[("a".to_string(), a)], 2)[0], None);
        let working = vec![("w".to_string(), task("/z", "running"))];
        let s = states(&queue, &working, 1);
        assert_eq!(s[0].as_deref(), Some("waits for a slot"));
        assert_eq!(states(&queue, &working, 2)[0], None);
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
        let a = store.add_request(&p, "a", "", &[], true).unwrap();
        let b = store.add_request(&q, "b", "", &[], true).unwrap();
        let kept = store.add_request(&p, "kept", "", &[], true).unwrap();
        assert!(place(&store, &a, "add").is_err(), "not framed yet");
        for id in [&a, &b, &kept] {
            framed(&store, id);
        }
        place(&store, &a, "add").unwrap();
        place(&store, &b, "add").unwrap();
        // The human moves b first; "kept" stays for later, out of the Fleet.
        place(&store, &b, "up").unwrap();
        set(&store, None, Some(2), None, None).unwrap();
        tick(&store, |_| {}, |_, _| Ok(()), |_| {}).unwrap();
        assert_eq!(store.load(&b).unwrap()["status"], "thinking");
        assert_eq!(store.load(&a).unwrap()["status"], "later");
        set(&store, Some(false), None, None, None).unwrap();
        tick(&store, |_| {}, |_, _| Ok(()), |_| {}).unwrap();
        assert_eq!(store.load(&a).unwrap()["status"], "later", "paused");
        set(&store, Some(true), None, None, None).unwrap();
        tick(&store, |_| {}, |_, _| Ok(()), |_| {}).unwrap();
        assert_eq!(store.load(&a).unwrap()["status"], "thinking");
        // Two work, two places: the queue is empty and "kept" never moved.
        assert!(queue(&store).unwrap().is_empty());
        tick(&store, |_| {}, |_, _| Ok(()), |_| {}).unwrap();
        assert_eq!(store.load(&kept).unwrap()["status"], "later");
        assert!(place(&store, &a, "add").is_err(), "no longer for later");
        // A task the Fleet started fails: the Fleet stops and says why.
        place(&store, &kept, "add").unwrap();
        store.update(&a, |r| r["status"] = json!("failed")).unwrap();
        store.update(&b, |r| r["status"] = json!("landed")).unwrap();
        tick(&store, |_| {}, |_, _| Ok(()), |_| {}).unwrap();
        assert_eq!(store.load(&kept).unwrap()["status"], "later", "stopped");
        let v = view(&store).unwrap();
        assert_eq!(v["on"], false);
        assert_eq!(v["stopped"], "\"a\" failed");
        assert_eq!(v["summary"]["landed"], 1);
        // Told to go on, the same failure, already seen, stops nothing.
        set(&store, Some(true), None, Some("go on"), None).unwrap();
        tick(&store, |_| {}, |_, _| Ok(()), |_| {}).unwrap();
        assert_eq!(store.load(&kept).unwrap()["status"], "thinking");
        std::fs::remove_dir_all(tmp).unwrap();
    }
    static PREPARED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    #[test]
    fn idle_backlog_tasks_are_framed_ahead_and_only_validated_ones_start() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-ahead-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        let p = tmp.join("p");
        std::fs::create_dir_all(&p).unwrap();
        store.add_project(p.to_str().unwrap(), "apart").unwrap();
        let p = std::fs::canonicalize(p).unwrap().display().to_string();
        let a = store.add_request(&p, "a", "", &[], true).unwrap();
        let b = store.add_request(&p, "b", "", &[], true).unwrap();
        let prepare = |id: String| PREPARED.lock().unwrap().push(id);
        let prepared = || std::mem::take(&mut *PREPARED.lock().unwrap());
        // A model turn this binary's other tests run in flight keeps
        // ahead() quiet: the Fleet looks again, so the test ticks again.
        let settle = |want: &Vec<String>| {
            for _ in 0..2000 {
                tick(&store, |_| {}, |_, _| Ok(()), prepare).unwrap();
                if prepared() == *want {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("the Fleet did not frame the Backlog in time");
        };
        // Stopped, nothing works: the Backlog's first task is framed ahead.
        set(&store, Some(false), None, None, None).unwrap();
        settle(&vec![a.clone()]);
        // It asked: it cannot be validated, and the next one is framed.
        store.update(&a, |r| r["asked"] = json!(true)).unwrap();
        assert!(place(&store, &a, "add").is_err());
        settle(&vec![b.clone()]);
        framed(&store, &b);
        settle(&Vec::new());
        // Running, nothing validated: nothing starts.
        set(&store, Some(true), None, None, None).unwrap();
        tick(&store, |_| {}, |_, _| Ok(()), prepare).unwrap();
        assert_eq!(store.load(&b).unwrap()["status"], "later");
        place(&store, &b, "add").unwrap();
        settle(&Vec::new());
        assert_eq!(store.load(&a).unwrap()["status"], "later");
        assert_eq!(store.load(&b).unwrap()["status"], "thinking");
        // Reworded, a task is framed again on its new words.
        store.rewrite(&a, "a2", "").unwrap();
        store.update(&b, |r| r["status"] = json!("landed")).unwrap();
        settle(&vec![a.clone()]);
        // A Ready task reworded leaves Ready until validated again.
        framed(&store, &a);
        set(&store, Some(false), None, None, None).unwrap();
        place(&store, &a, "add").unwrap();
        store.rewrite(&a, "a3", "").unwrap();
        assert!(queue(&store).unwrap().is_empty());
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn on_the_branch_proven_work_is_committed_before_the_next_starts() {
        let tmp =
            std::env::temp_dir().join(format!("fennec-lead-fleet-branch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        let p = tmp.join("p");
        std::fs::create_dir_all(&p).unwrap();
        store.add_project(p.to_str().unwrap(), "apart").unwrap();
        let p = std::fs::canonicalize(p).unwrap().display().to_string();
        let a = store.add_request(&p, "a", "", &[], true).unwrap();
        let b = store.add_request(&p, "b", "", &[], true).unwrap();
        framed(&store, &a);
        framed(&store, &b);
        place(&store, &a, "add").unwrap();
        place(&store, &b, "add").unwrap();
        set(&store, None, Some(2), None, Some("branch")).unwrap();
        assert!(set(&store, None, None, None, Some("main")).is_err());
        let landing = |s: &Store, id: &str| s.update(id, |r| r["status"] = json!("landing"));
        tick(&store, |_| {}, landing, |_| {}).unwrap();
        assert_eq!(store.load(&a).unwrap()["fleet_branch"], BRANCH);
        // Proven: the next look commits it rather than starting b.
        store.update(&a, |r| r["status"] = json!("ready")).unwrap();
        tick(&store, |_| {}, landing, |_| {}).unwrap();
        let r = store.load(&a).unwrap();
        assert_eq!(
            (r["status"].as_str(), &r["target"]["branch"]),
            (Some("landing"), &json!(BRANCH))
        );
        assert_eq!(store.load(&b).unwrap()["status"], "later");
        // The commit failed (a conflict): the Fleet stops and says so.
        store
            .update(&a, |r| {
                r["status"] = json!("ready");
                r["land_error"] = json!("conflit");
            })
            .unwrap();
        tick(&store, |_| {}, landing, |_| {}).unwrap();
        assert_eq!(
            view(&store).unwrap()["stopped"],
            "\"a\" could not be committed on fleet"
        );
        assert_eq!(store.load(&b).unwrap()["status"], "later");
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
