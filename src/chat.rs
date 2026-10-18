//! Chat: a plain conversation with an agent in a folder the person chose,
//! git or not. No world, no request, no proof: the agent reads, runs
//! commands and writes in that folder directly, in the same sandbox as a
//! world's (no network, hidden home, private /tmp), and its turn ends when
//! it answers without a tool call. Chats live in `<store>/chats/<id>.json`:
//! the folder, the chosen skills, the messages without the system prompt,
//! and one entry per model call for the cost.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::agent;
use crate::outside;
use crate::provider::Model;
use crate::skill;
use crate::world::Worlds;

const SYSTEM: &str = "You are fennec, talking with a person about the folder they chose; \
it is the current directory. Answer in their language, like a colleague, in Markdown. Use the \
tools whenever the answer is in the files or needs a command: read before you answer, and do \
what they ask. Your changes are made directly in their folder and cannot be undone by the \
harness, so ask before deleting or overwriting what you did not write. The sandbox has no \
network and hides their home except this folder. Your turn ends when you reply without a tool \
call: that reply is what they read.";

/// Every write of a chat file goes through this lock: the turn running in
/// the background and the page (stop, skills) change the same file.
/// ponytail: one process; a second fennec on the same store would race.
static LOCK: Mutex<()> = Mutex::new(());

fn path(worlds: &Worlds, id: &str) -> io::Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(io::Error::other(format!("no chat {id}")));
    }
    Ok(worlds.root.join("chats").join(format!("{id}.json")))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn load(worlds: &Worlds, id: &str) -> io::Result<Value> {
    let text = std::fs::read_to_string(path(worlds, id)?)
        .map_err(|_| io::Error::other(format!("no chat {id}")))?;
    serde_json::from_str(&text).map_err(io::Error::other)
}

fn save(worlds: &Worlds, id: &str, chat: &Value) -> io::Result<()> {
    let p = path(worlds, id)?;
    std::fs::create_dir_all(p.parent().unwrap_or(&worlds.root))?;
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, chat.to_string())?;
    std::fs::rename(tmp, p)
}

fn update(worlds: &Worlds, id: &str, change: impl FnOnce(&mut Value)) -> io::Result<Value> {
    let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut chat = load(worlds, id)?;
    change(&mut chat);
    save(worlds, id, &chat)?;
    Ok(chat)
}

/// Why `dir` cannot be opened read-write, as a chat's folder or a grant:
/// it must sit strictly inside the home and never over the store.
fn unopenable(worlds: &Worlds, dir: &Path) -> Option<String> {
    let home = std::fs::canonicalize(&worlds.home).unwrap_or(worlds.home.clone());
    let store = std::fs::canonicalize(&worlds.root).unwrap_or(worlds.root.clone());
    if !dir.starts_with(&home) || dir == home {
        return Some(format!(
            "{} is not a folder strictly inside {}",
            dir.display(),
            home.display()
        ));
    }
    if dir.starts_with(&store) || store.starts_with(dir) {
        return Some(format!("{} overlaps the world store", dir.display()));
    }
    None
}

/// A new chat over `folder`: strictly inside the home, as a grant, and never
/// over the store. It starts with the skills the folder's project chose.
pub fn start(worlds: &Worlds, folder: &Path) -> io::Result<Value> {
    let folder = std::fs::canonicalize(folder)?;
    if !folder.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a folder",
            folder.display()
        )));
    }
    if let Some(why) = unopenable(worlds, &folder) {
        return Err(io::Error::other(why));
    }
    let skills: Vec<String> = skill::chosen(Some(&folder))?
        .into_iter()
        .map(|s| s.name)
        .collect();
    let id = crate::world::new_id();
    let chat = json!({"id": id, "folder": folder.display().to_string(), "created": now(),
                      "status": "idle", "skills": skills, "grants": [],
                      "messages": [], "turns": []});
    save(worlds, &id, &chat)?;
    Ok(chat)
}

/// Every chat, the latest first, without its messages.
pub fn list(worlds: &Worlds) -> io::Result<Vec<Value>> {
    let Ok(entries) = std::fs::read_dir(worlds.root.join("chats")) else {
        return Ok(Vec::new());
    };
    let mut ids: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".json")
                .map(str::to_string)
        })
        .collect();
    ids.sort();
    ids.reverse();
    Ok(ids
        .iter()
        .filter_map(|id| load(worlds, id).ok())
        .map(|c| {
            json!({"id": c["id"], "folder": c["folder"], "status": c["status"],
                   "title": title(&c), "since": c["since"]})
        })
        .collect())
}

/// The chat's name: its first message, cut.
pub fn title(chat: &Value) -> String {
    let first = chat["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("Nouvelle discussion");
    let line = first.lines().next().unwrap_or("");
    match line.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

/// Adds or takes back a skill for this chat's next turns.
pub fn choose(worlds: &Worlds, id: &str, name: &str, on: bool) -> io::Result<Value> {
    update(worlds, id, |c| {
        let mut names: Vec<Value> = c["skills"].as_array().cloned().unwrap_or_default();
        names.retain(|n| n != name);
        if on {
            names.push(json!(name));
        }
        c["skills"] = json!(names);
    })
}

/// Asks the turn under way to end after its current call.
pub fn stop(worlds: &Worlds, id: &str) -> io::Result<Value> {
    update(worlds, id, |c| c["stop"] = json!(true))
}

/// Marks the person's message as said and the chat as thinking; the caller
/// then runs [`answer`], usually in the background. Fails while a turn runs.
pub fn say(worlds: &Worlds, id: &str, text: &str) -> io::Result<()> {
    if text.trim().is_empty() {
        return Err(io::Error::other("empty message"));
    }
    let mut busy = false;
    update(worlds, id, |c| {
        busy = c["status"] == "thinking" || c["status"] == "waiting";
        if busy {
            return;
        }
        if let Some(m) = c["messages"].as_array_mut() {
            m.push(json!({"role": "user", "content": text.trim()}));
        }
        c["status"] = json!("thinking");
        c["since"] = json!(now());
        c["stop"] = json!(false);
        c["error"] = Value::Null;
    })?;
    if busy {
        return Err(io::Error::other("fennec is still answering"));
    }
    Ok(())
}

/// fennec's turn: model calls and tools until a reply without a tool call,
/// a stop asked by the person, or `max_steps`. Each call is saved as it
/// comes, so the page follows the turn while it runs.
pub fn answer(worlds: &Worlds, id: &str, model: &dyn Model, max_steps: usize) -> io::Result<()> {
    let ran = turn(worlds, id, model, max_steps);
    update(worlds, id, |c| {
        c["status"] = json!("idle");
        c["stop"] = json!(false);
        c["pending"] = Value::Null;
        c["decision"] = Value::Null;
        if let Err(e) = &ran {
            c["error"] = json!(e.to_string());
        }
    })?;
    Ok(())
}

/// The turn never started: its settings gave no model. Say so, as [`answer`]
/// does when its turn fails, so the page does not stay on "répond…".
pub fn fail(worlds: &Worlds, id: &str, why: &str) -> io::Result<()> {
    update(worlds, id, |c| {
        c["status"] = json!("idle");
        c["stop"] = json!(false);
        c["error"] = json!(why);
    })
    .map(drop)
}

/// The person's answer to the pending question: `true` opens the folder for
/// the whole chat (a grant), `false` covers this call only, so the next
/// attempt asks again.
pub fn decide(worlds: &Worlds, id: &str, allow: bool) -> io::Result<()> {
    let mut answered = false;
    update(worlds, id, |c| {
        if c["status"] != "waiting" {
            return;
        }
        answered = true;
        c["status"] = json!("thinking");
        c["decision"] = json!(allow);
        if allow {
            let dir = c["pending"]["directory"].as_str().map(String::from);
            if let Some(d) = dir {
                push_grant(c, &d);
            }
        }
        c["pending"] = Value::Null;
    })?;
    if !answered {
        return Err(io::Error::other("fennec n'attend plus de réponse"));
    }
    Ok(())
}

/// Takes back a folder the person had opened for this chat.
pub fn ungrant(worlds: &Worlds, id: &str, dir: &str) -> io::Result<()> {
    update(worlds, id, |c| {
        if let Some(l) = c["grants"].as_array_mut() {
            l.retain(|g| *g != json!(dir));
        }
    })
    .map(drop)
}

/// Records `dir` among the chat's grants, once.
fn push_grant(c: &mut Value, dir: &str) {
    if !c["grants"].is_array() {
        c["grants"] = json!([]);
    }
    let Some(l) = c["grants"].as_array_mut() else {
        return;
    };
    if !l.iter().any(|g| *g == json!(dir)) {
        l.push(json!(dir));
    }
}

/// A turn that was running when fennec stopped will not resume: it goes back
/// to idle with the reason, so the person can speak again.
pub fn recover(worlds: &Worlds) -> io::Result<()> {
    for c in list(worlds)? {
        if c["status"] == "thinking" || c["status"] == "waiting" {
            update(worlds, c["id"].as_str().unwrap_or(""), |c| {
                c["status"] = json!("idle");
                c["stop"] = json!(false);
                c["pending"] = Value::Null;
                c["decision"] = Value::Null;
                c["error"] = json!("interrompu : fennec a été arrêté pendant la réponse");
            })?;
        }
    }
    Ok(())
}

/// Shows the question on the page and waits for the person's decision, the
/// chat file being what the page writes. `None` when a stop asked for the
/// turn to end.
fn ask(
    worlds: &Worlds,
    id: &str,
    tool: &str,
    path: &Path,
    directory: &Path,
) -> io::Result<Option<bool>> {
    update(worlds, id, |c| {
        c["status"] = json!("waiting");
        c["pending"] = json!({"tool": tool, "path": path.display().to_string(),
                              "directory": directory.display().to_string()});
    })?;
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let c = load(worlds, id)?;
        if c["stop"] == true {
            update(worlds, id, |c| c["pending"] = Value::Null)?;
            return Ok(None);
        }
        if c["decision"].is_boolean() {
            let allow = c["decision"] == true;
            update(worlds, id, |c| c["decision"] = Value::Null)?;
            return Ok(Some(allow));
        }
    }
}

fn turn(worlds: &Worlds, id: &str, model: &dyn Model, max_steps: usize) -> io::Result<()> {
    let chat = load(worlds, id)?;
    let folder = PathBuf::from(chat["folder"].as_str().unwrap_or(""));
    let tools: Value = agent::tools()
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["function"]["name"] != "done")
        .cloned()
        .collect();
    let mut watch = agent::Watch::default();
    // Blank: the model spent its whole budget reasoning; the next call goes
    // without reasoning, as in `agent::run`.
    let mut blank = false;
    for _ in 0..max_steps {
        // Skills are read again at each call: the person may change them.
        let chat = load(worlds, id)?;
        if chat["stop"] == true {
            return Ok(());
        }
        let names: Vec<&str> = chat["skills"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let skills: Vec<skill::Skill> = skill::catalog(Some(&folder))?
            .into_iter()
            .filter(|s| names.contains(&s.name.as_str()))
            .collect();
        let mounts: Vec<(String, PathBuf)> = skills
            .iter()
            .map(|s| (s.name.clone(), s.dir.clone()))
            .collect();
        let system = format!("{SYSTEM}{}", skill::brief(&skills));
        let mut messages = vec![json!({"role": "system", "content": system})];
        messages.extend(chat["messages"].as_array().cloned().unwrap_or_default());
        agent::bound(&mut messages);
        let asked = Instant::now();
        let mut reply = if blank {
            model.chat_quick(&messages, &tools)?
        } else {
            model.chat(&messages, &tools)?
        };
        let mut record = json!({"at": now(), "model_ms": asked.elapsed().as_millis() as u64,
                                "think": !blank});
        if let Some(obj) = reply.as_object_mut() {
            obj.remove("reasoning_content");
            obj.remove("reasoning");
            if let Some(usage) = obj.remove("usage") {
                record["usage"] = usage;
            }
        }
        reply["role"] = json!("assistant");
        let calls = reply["tool_calls"].as_array().cloned().unwrap_or_default();
        let text = reply["content"].as_str().unwrap_or("");
        // Some servers leave the reasoning inline.
        let text = text
            .rsplit("</think>")
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if calls.is_empty() && text.is_empty() {
            if blank {
                return Err(io::Error::other("the model gave an empty reply twice"));
            }
            blank = true;
            update(worlds, id, |c| push(&mut c["turns"], record))?;
            continue;
        }
        blank = false;
        if calls.is_empty() {
            reply["content"] = json!(text);
        }
        let mut said = vec![reply];
        let mut stuck = None;
        let mut times = Vec::new();
        for call in &calls {
            let name = call["function"]["name"].as_str().unwrap_or("");
            // Folders the person opened for this chat, read again before each
            // call: one taken back meanwhile is closed for it.
            let mut grants = grants(worlds, id)?;
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            let started = Instant::now();
            // A literal path outside the folder the person chose: ask before
            // the call touches it; opening a folder stays open for the chat.
            if let Some(p) = outside::outside(&folder, &worlds.home, &worlds.ro_paths, name, &args)
                .into_iter()
                .find(|p| !grants.iter().any(|g| outside::covers(g, p)))
            {
                let directory = outside::grant_dir(&p);
                // The home itself, `/` or the store is never offered: one
                // click there would undo the sandbox.
                if let Some(why) = unopenable(worlds, &directory) {
                    times.push(json!({"name": name, "ms": started.elapsed().as_millis() as u64}));
                    said.push(json!({"role": "tool", "tool_call_id": call["id"],
                        "content": format!("refused without asking: {why}; it is untouched")}));
                    continue;
                }
                match ask(worlds, id, name, &p, &directory)? {
                    Some(true) => {
                        grants.push(directory.clone());
                        update(worlds, id, |c| {
                            push_grant(c, &directory.to_string_lossy());
                        })?;
                    }
                    Some(false) => {
                        times.push(
                            json!({"name": name, "ms": started.elapsed().as_millis() as u64}),
                        );
                        said.push(json!({"role": "tool", "tool_call_id": call["id"],
                        "content": format!(
                            "the person refused access to {}; it is untouched, do not retry it",
                            directory.display()
                        )}));
                        continue;
                    }
                    None => return Ok(()),
                }
            }
            let sandbox = |argv: &[String]| worlds.sandbox_folder(&folder, &mounts, &grants, argv);
            let result =
                agent::execute(&sandbox, name, &args).unwrap_or_else(|e| format!("error: {e}"));
            times.push(json!({"name": name, "ms": started.elapsed().as_millis() as u64}));
            if watch.call(&call["function"]["arguments"].to_string(), &result) == Some(true) {
                stuck = Some(format!(
                    "stopped: the same call gave the same result {} times in a row",
                    agent::REPEAT_STOP
                ));
            }
            said.push(json!({"role": "tool", "tool_call_id": call["id"],
                             "content": agent::truncate(&result)}));
        }
        record["tools"] = json!(times);
        // What was cut to bound the context stays cut: cutting again at each
        // call would change the prefix and cost a fresh prefill every time.
        let kept: Vec<Value> = messages.into_iter().skip(1).chain(said).collect();
        update(worlds, id, |c| {
            c["messages"] = json!(kept);
            push(&mut c["turns"], record);
        })?;
        if let Some(why) = stuck {
            return Err(io::Error::other(why));
        }
        if calls.is_empty() {
            return Ok(());
        }
    }
    Err(io::Error::other(format!(
        "stopped after {max_steps} steps without an answer"
    )))
}

/// The folders the person opened for chat `id`; a granted call runs read-write.
fn grants(worlds: &Worlds, id: &str) -> io::Result<Vec<PathBuf>> {
    Ok(load(worlds, id)?["grants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect())
}

fn push(list: &mut Value, item: Value) {
    if !list.is_array() {
        *list = json!([]);
    }
    if let Some(l) = list.as_array_mut() {
        l.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worlds(dir: &std::path::Path) -> Worlds {
        Worlds {
            root: dir.join("store"),
            home: dir.to_path_buf(),
            ro_paths: Vec::new(),
            memory_mb: None,
        }
    }

    #[test]
    fn a_turn_that_never_started_says_so_and_moves_on() {
        let dir = std::env::temp_dir().join(format!("fennec-os-chat-{}", std::process::id()));
        let folder = dir.join("proj");
        std::fs::create_dir_all(&folder).unwrap();
        let w = worlds(&dir);
        let chat = start(&w, &folder).unwrap();
        let id = chat["id"].as_str().unwrap().to_string();
        say(&w, &id, "bonjour").unwrap();
        assert_eq!(load(&w, &id).unwrap()["status"], "thinking");
        fail(&w, &id, "no model: the server did not answer").unwrap();
        let c = load(&w, &id).unwrap();
        assert_eq!(c["status"], "idle");
        assert_eq!(c["stop"], false);
        assert_eq!(c["error"], "no model: the server did not answer");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
