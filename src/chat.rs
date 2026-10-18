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
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::agent;
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

/// A new chat over `folder`: strictly inside the home, as a grant, and never
/// over the store. It starts with the skills the folder's project chose.
pub fn start(worlds: &Worlds, folder: &Path) -> io::Result<Value> {
    let folder = std::fs::canonicalize(folder)?;
    let home = std::fs::canonicalize(&worlds.home)?;
    let store = std::fs::canonicalize(&worlds.root).unwrap_or(worlds.root.clone());
    if !folder.is_dir() || !folder.starts_with(&home) || folder == home {
        return Err(io::Error::other(format!(
            "{} is not a folder strictly inside {}",
            folder.display(),
            home.display()
        )));
    }
    if folder.starts_with(&store) || store.starts_with(&folder) {
        return Err(io::Error::other(format!(
            "{} overlaps the world store",
            folder.display()
        )));
    }
    let skills: Vec<String> = skill::chosen(&folder)?
        .into_iter()
        .map(|s| s.name)
        .collect();
    let id = crate::world::new_id();
    let chat = json!({"id": id, "folder": folder.display().to_string(), "created": now(),
                      "status": "idle", "skills": skills, "messages": [], "turns": []});
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
        busy = c["status"] == "thinking";
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
        if let Err(e) = &ran {
            c["error"] = json!(e.to_string());
        }
    })?;
    Ok(())
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
        let skills: Vec<skill::Skill> = skill::catalog(&folder)?
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
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            let started = Instant::now();
            let sandbox = |argv: &[String]| worlds.sandbox_folder(&folder, &mounts, argv);
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

fn push(list: &mut Value, item: Value) {
    if !list.is_array() {
        *list = json!([]);
    }
    if let Some(l) = list.as_array_mut() {
        l.push(item);
    }
}
