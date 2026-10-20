//! Chat: a plain conversation with an agent in a folder the person chose,
//! git or not. No world, no request, no proof: the agent reads and runs
//! commands in that folder, and writes there directly once the person
//! allows changes (a chat starts read-only), in the same sandbox as a
//! world's (no network, hidden home, private /tmp), and its turn ends when
//! it answers without a tool call. Chats live in `<store>/chats/<id>.json`:
//! the folder, the messages without the system prompt,
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
use crate::tools;
use crate::world::Worlds;

const SYSTEM: &str = "You are fennec, talking with a person about the folder they chose; \
it is the current directory. Answer in their language, like a colleague, in Markdown. Use the \
tools whenever the answer is in the files or needs a command: read before you answer. The \
sandbox reaches the network and hides their home except this folder. Dependencies go in the \
folder with its own tools, never in the system or the home. Your turn ends when you reply \
without a tool call: that reply is what they read.";

/// A chat starts read-only: talking about a project is not asking to build it.
const READ: &str = " The folder is read-only for now: read, discuss and plan. When they \
want something built or changed, propose a short plan (files, steps, open questions) and tell \
them with `propose_plan`: if they approve, changes are allowed and you apply it in the same \
turn; if not, keep discussing. Do not try to write before.";

const WRITE: &str = " They allowed changes: do what they ask. Your changes are made directly \
in their folder and cannot be undone by the harness, so ask before deleting or overwriting \
what you did not write. Read a file before changing it (read_file), change it with edit_file \
(write_file for a new file), never with sed or a script; build and test after each change.";

/// The reminder's line while an approved plan's steps are open.
const PLAN: &str = "Work through the approved plan, step by step: build and test what you \
change, then mark the step with `criteria` (action done). Answer the person once every step \
is done.";

/// Every write of a chat file goes through this lock: the turn running in
/// the background and the page (stop) change the same file.
/// ponytail: one process; a second fennec on the same store would race.
static LOCK: Mutex<()> = Mutex::new(());

fn path(worlds: &Worlds, id: &str) -> io::Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(io::Error::other(format!("discussion not found: {id}")));
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
        .map_err(|_| io::Error::other(format!("discussion not found: {id}")))?;
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
/// over the store. Skills come from the person's global list, read at each
/// turn; the chat keeps none of its own.
pub fn start(worlds: &Worlds, folder: &Path) -> io::Result<Value> {
    let folder = std::fs::canonicalize(folder)?;
    if !folder.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a folder",
            folder.display()
        )));
    }
    if let Some(why) = worlds.unopenable(&folder) {
        return Err(io::Error::other(why));
    }
    let id = crate::world::new_id();
    let chat = json!({"id": id, "folder": folder.display().to_string(), "created": now(),
                      "status": "idle", "grants": [],
                      "messages": [], "turns": []});
    save(worlds, &id, &chat)?;
    Ok(chat)
}

/// Every chat, the latest first, without its messages.
pub fn list(worlds: &Worlds) -> io::Result<Vec<Value>> {
    listed(worlds, Some(false))
}

/// The archived ones, the latest first: hidden from the list, kept on disk.
pub fn list_archived(worlds: &Worlds) -> io::Result<Vec<Value>> {
    listed(worlds, Some(true))
}

/// `None`: archived or not.
fn listed(worlds: &Worlds, archived: Option<bool>) -> io::Result<Vec<Value>> {
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
        // A chat without the key is a legacy one: not archived.
        .filter(|c| archived.is_none_or(|a| c["archived"].as_bool().unwrap_or(false) == a))
        .map(|c| {
            json!({"id": c["id"], "folder": c["folder"], "status": c["status"],
                   "title": title(&c), "since": c["since"]})
        })
        .collect())
}

/// Out of the list, or back in it: the exchange stays on disk either way.
pub fn archive(worlds: &Worlds, id: &str, on: bool) -> io::Result<Value> {
    update(worlds, id, |c| c["archived"] = json!(on))
}

/// Gone for good. A turn under way would rewrite the file from under it, so
/// it is refused while the chat thinks or waits; stop it first.
pub fn forget(worlds: &Worlds, id: &str) -> io::Result<()> {
    // Checked under the lock: a turn started in between would lose its file.
    let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if busy(&load(worlds, id)?) {
        return Err(io::Error::other("the discussion is running: stop it first"));
    }
    std::fs::remove_file(path(worlds, id)?)
}

/// The chat's name: its first message, cut.
pub fn title(chat: &Value) -> String {
    let first = chat["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("New discussion");
    let line = first.lines().next().unwrap_or("");
    match line.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

/// Allows the chat to change its folder (and the folders opened for it), or
/// takes it back to read-only; the next call follows.
pub fn write(worlds: &Worlds, id: &str, on: bool) -> io::Result<Value> {
    update(worlds, id, |c| c["write"] = json!(on))
}

/// Asks the turn under way to end after its current call.
pub fn stop(worlds: &Worlds, id: &str) -> io::Result<Value> {
    update(worlds, id, |c| c["stop"] = json!(true))
}

/// Marks the person's message as said and the chat as thinking; the caller
/// then runs [`answer`], usually in the background. Fails while a turn runs.
pub fn say(worlds: &Worlds, id: &str, text: &str) -> io::Result<()> {
    speak(worlds, id, None, text)
}

/// Says `text` again in place of the person's message at `index`: the
/// exchange from there on is dropped, then the turn runs as after [`say`].
/// Files changed since stay as they are; the cost already spent stays counted.
pub fn replay(worlds: &Worlds, id: &str, index: usize, text: &str) -> io::Result<()> {
    speak(worlds, id, Some(index), text)
}

fn speak(worlds: &Worlds, id: &str, at: Option<usize>, text: &str) -> io::Result<()> {
    if text.trim().is_empty() {
        return Err(io::Error::other("empty message"));
    }
    let mut refused = None;
    update(worlds, id, |c| {
        if busy(c) {
            refused = Some("fennec is still answering");
            return;
        }
        if let Some(i) = at {
            if c["messages"][i]["role"] != "user" {
                refused = Some("no message of yours at this place");
                return;
            }
            if let Some(m) = c["messages"].as_array_mut() {
                m.truncate(i);
            }
            // Notes taken past the cut summarize what is gone: the whole
            // exchange is sent again, compacted anew if it must be.
            if c["window"].as_u64().is_some_and(|w| w as usize > i) {
                c["window"] = json!(0);
            }
        }
        push(
            &mut c["messages"],
            json!({"role": "user", "content": text.trim()}),
        );
        c["status"] = json!("thinking");
        c["since"] = json!(now());
        c["stop"] = json!(false);
        c["error"] = Value::Null;
    })?;
    refused.map_or(Ok(()), |why| Err(io::Error::other(why)))
}

fn busy(c: &Value) -> bool {
    c["status"] == "thinking" || c["status"] == "waiting"
}

/// Back to idle, `why` said if the turn did not end well: the person may
/// speak again.
fn settle(c: &mut Value, why: Option<String>) {
    c["status"] = json!("idle");
    c["stop"] = json!(false);
    c["pending"] = Value::Null;
    c["decision"] = Value::Null;
    if let Some(why) = why {
        c["error"] = json!(why);
    }
}

/// fennec's turn: model calls and tools until a reply without a tool call,
/// a stop asked by the person, or `max_steps`. Each call is saved as it
/// comes, so the page follows the turn while it runs.
pub fn answer(worlds: &Worlds, id: &str, model: &dyn Model, max_steps: usize) -> io::Result<()> {
    let ran = turn(worlds, id, model, max_steps);
    update(worlds, id, |c| settle(c, ran.err().map(|e| e.to_string())))?;
    Ok(())
}

/// The turn never started: its settings gave no model. Say so, as [`answer`]
/// does when its turn fails, so the page does not stay on "répond…".
pub fn fail(worlds: &Worlds, id: &str, why: &str) -> io::Result<()> {
    update(worlds, id, |c| settle(c, Some(why.to_string()))).map(drop)
}

/// The person's answer to the pending question: `true` opens the folder for
/// the whole chat (a grant), `false` covers this call only, so the next
/// attempt asks again.
pub fn decide(worlds: &Worlds, id: &str, allow: bool) -> io::Result<()> {
    decide_for(worlds, id, allow, false)
}

/// [`decide`]; `for_chat`: a rule's question answered for the rest of the
/// chat, not this call only.
pub fn decide_for(worlds: &Worlds, id: &str, allow: bool, for_chat: bool) -> io::Result<()> {
    let mut answered = false;
    update(worlds, id, |c| {
        if c["status"] != "waiting" {
            return;
        }
        answered = true;
        c["status"] = json!("thinking");
        c["decision"] = json!(allow);
        c["for_chat"] = json!(for_chat);
        // An approved plan allows changes, as the button on the page does.
        if allow && c["pending"]["tool"] == "propose_plan" {
            c["write"] = json!(true);
        } else if allow {
            let dir = c["pending"]["directory"].as_str().map(String::from);
            if let Some(d) = dir {
                push_grant(c, &d);
            }
        }
        c["pending"] = Value::Null;
    })?;
    if !answered {
        return Err(io::Error::other("fennec no longer waits for an answer"));
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
/// to idle with the reason, so the person can speak again. Archived chats
/// are recovered too: hidden from the list, their turn must not freeze.
pub fn recover(worlds: &Worlds) -> io::Result<()> {
    for c in listed(worlds, None)? {
        if busy(&c) {
            update(worlds, c["id"].as_str().unwrap_or(""), |c| {
                settle(
                    c,
                    Some("interrupted: fennec was stopped during the answer".into()),
                )
            })?;
        }
    }
    Ok(())
}

/// Shows the question on the page and waits for the person's decision, the
/// chat file being what the page writes. `None` when a stop asked for the
/// turn to end.
fn ask(worlds: &Worlds, id: &str, pending: Value) -> io::Result<Option<bool>> {
    update(worlds, id, |c| {
        c["status"] = json!("waiting");
        c["pending"] = pending;
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
    // The plan's steps, kept across turns until they are all done.
    let criteria = chat["criteria"].as_array().cloned().unwrap_or_default();
    let mut engine = agent::Engine::new(model, criteria);
    engine.guard = Some(crate::guard::Guard::load(&folder, &folder, &worlds.home)?);
    engine.allowed = chat["allowed"].as_array().cloned().unwrap_or_default();
    // A rule's question goes to the page as the others do.
    let asker = |question: Value| {
        Ok(
            ask(worlds, id, question)?.map(|yes| match (yes, load(worlds, id)) {
                (false, _) => agent::Answer::No,
                (true, Ok(c)) if c["for_chat"] == true => agent::Answer::Chat,
                (true, _) => agent::Answer::Once,
            }),
        )
    };
    engine.asker = Some(&asker);
    // Replies without a tool call while the plan has open steps: the model
    // is sent back to them, twice at most, then the turn ends.
    let mut pushed = 0;
    for step in 1..=max_steps {
        // Skills are read again at each call: the person may change them.
        let chat = load(worlds, id)?;
        if chat["stop"] == true {
            return Ok(());
        }
        let skills: Vec<skill::Skill> = skill::chosen(Some(&folder))?;
        let mounts: Vec<(String, PathBuf)> = skills
            .iter()
            .map(|s| (s.name.clone(), s.dir.clone()))
            .collect();
        // Read at each call: the person may allow changes while the turn runs.
        let write = chat["write"] == true;
        let mut tools = tools::definitions(write);
        tools.push(if write {
            tools::criteria_tool()
        } else {
            propose_plan()
        });
        let tools = Value::Array(tools);
        let mode = if write { WRITE } else { READ };
        let system = json!({"role": "system",
                            "content": format!("{SYSTEM}{}{mode}{}", agent::tools_line(worlds), skill::brief(&skills))});
        let mut log = chat["messages"].as_array().cloned().unwrap_or_default();
        // The model sees the exchange from `window` on: past the last
        // compaction, whose notes open it.
        let mut from = (chat["window"].as_u64().unwrap_or(0) as usize).min(log.len());
        if engine.needs_compaction() {
            let window: Vec<Value> = std::iter::once(system.clone())
                .chain(log[from..].iter().cloned())
                .collect();
            if let Ok(notes) = engine.compact(&window) {
                let last = log
                    .iter()
                    .rev()
                    .find(|m| m["role"] == "user" && m["harness"] != true)
                    .and_then(|m| m["content"].as_str())
                    .unwrap_or("");
                log.push(agent::harness(&format!(
                    "The conversation was shortened; your own notes on it:\n{notes}\n\n\
                     The person's latest message:\n{last}"
                )));
                from = log.len() - 1;
                update(worlds, id, |c| {
                    c["messages"] = json!(log);
                    c["window"] = json!(from);
                })?;
            }
        }
        let mut window: Vec<Value> = std::iter::once(system)
            .chain(log[from..].iter().cloned())
            .collect();
        // Framed at each step while a plan is under way.
        if write && engine.open() > 0 {
            window.push(engine.reminder(step, max_steps, PLAN));
        }
        let was_blank = engine.blank;
        let (mut reply, cut) = engine.ask(&window, &tools)?;
        let mut record = engine.turns.last().cloned().unwrap_or_default();
        record["at"] = json!(now());
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
            if was_blank {
                return Err(io::Error::other("the model gave an empty reply twice"));
            }
            update(worlds, id, |c| push(&mut c["turns"], record))?;
            continue;
        }
        if calls.is_empty() {
            reply["content"] = json!(text);
        }
        let mut said = vec![reply];
        let mut stuck = None;
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
            let timed =
                |name: &str| json!({"name": name, "ms": started.elapsed().as_millis() as u64});
            // The plan is the question: approved, the rest of the turn writes.
            if name == "propose_plan" {
                // Steps, when given, let the page split the plan into tasks.
                let steps = args["steps"].as_array().cloned().unwrap_or_default();
                let pending = json!({"tool": name, "plan": args["plan"].as_str().unwrap_or(""),
                                     "steps": steps});
                let answer = match ask(worlds, id, pending)? {
                    Some(true) => {
                        // Its steps are what the turn works through, one by one.
                        engine.criteria = steps
                            .iter()
                            .filter_map(Value::as_str)
                            .map(|s| agent::step_criterion(s.lines().next().unwrap_or(s)))
                            .collect();
                        "approved: changes are allowed now, apply the plan"
                    }
                    Some(false) => {
                        "not approved: the folder stays read-only; ask what to change in the plan"
                    }
                    None => return Ok(()),
                };
                push(&mut record["tools"], timed(name));
                said.push(json!({"role": "tool", "tool_call_id": call["id"], "content": answer}));
                continue;
            }
            // A literal path outside the folder the person chose: ask before
            // the call touches it; opening a folder stays open for the chat.
            if let Some(p) = outside::outside(&folder, &worlds.home, &worlds.ro_paths, name, &args)
                .into_iter()
                .find(|p| !grants.iter().any(|g| outside::covers(g, p)))
            {
                let directory = outside::grant_dir(&p);
                // The home itself, `/` or the store is never offered: one
                // click there would undo the sandbox.
                if let Some(why) = worlds.unopenable(&directory) {
                    push(&mut record["tools"], timed(name));
                    said.push(json!({"role": "tool", "tool_call_id": call["id"],
                        "content": format!("refused without asking: {why}; it is untouched")}));
                    continue;
                }
                let pending = json!({"tool": name, "path": p.display().to_string(),
                                     "directory": directory.display().to_string()});
                match ask(worlds, id, pending)? {
                    Some(true) => {
                        grants.push(directory.clone());
                        update(worlds, id, |c| {
                            push_grant(c, &directory.to_string_lossy());
                        })?;
                    }
                    Some(false) => {
                        push(&mut record["tools"], timed(name));
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
            let sandbox =
                |argv: &[String]| worlds.sandbox_folder(&folder, &mounts, &grants, write, argv);
            let result = engine.tool(&sandbox, call);
            push(&mut record["tools"], timed(name));
            if engine
                .watch
                .call(&call["function"]["arguments"].to_string(), &result)
                == Some(true)
            {
                stuck = Some(format!(
                    "stopped: the same call gave the same result {} times in a row",
                    agent::REPEAT_STOP
                ));
            }
            said.push(json!({"role": "tool", "tool_call_id": call["id"], "content": result}));
        }
        let open = engine.open();
        // A reply that leaves the plan's steps open, or one cut by the
        // length limit: sent back to work.
        let more = if cut {
            engine.continue_cut()
        } else if calls.is_empty() && write && open > 0 && pushed < 2 {
            pushed += 1;
            Some(agent::harness(&format!(
                "<system-reminder>{open} step(s) of the approved plan are not done. Go on with \
                 the next one now, and mark each with `criteria` when it is done. If one cannot \
                 be done, say why.</system-reminder>"
            )))
        } else {
            None
        };
        let going = more.is_some();
        said.extend(more);
        let criteria = engine.criteria.clone();
        let allowed = engine.allowed.clone();
        update(worlds, id, |c| {
            push_all(&mut c["messages"], said);
            c["criteria"] = json!(criteria);
            c["allowed"] = json!(allowed);
            push(&mut c["turns"], record);
        })?;
        if let Some(why) = stuck {
            return Err(io::Error::other(why));
        }
        if calls.is_empty() && !going {
            return Ok(());
        }
    }
    Err(io::Error::other(format!(
        "stopped after {max_steps} steps without an answer"
    )))
}

/// The tool a read-only chat ends its planning with, as a coding harness's
/// plan mode does: the person approves on the page, or not.
fn propose_plan() -> Value {
    json!({"type": "function", "function": {
        "name": "propose_plan",
        "description": "Show the person your plan and ask them to allow changes. Their answer is the result.",
        "parameters": {"type": "object", "required": ["plan"], "properties": {
            "plan": {"type": "string", "description": "the plan in Markdown, in their language"},
            "steps": {"type": "array", "items": {"type": "string"}, "description":
                "when the plan has several independent steps: each one alone, its title on the first line, \
                 then in a few lines what to do, the context it needs (it is done without the plan) \
                 and how to know it is done; \
                 the person may make each a task of its own"}}}
    }})
}

/// The folders the person opened for chat `id`; writable as the folder is.
fn grants(worlds: &Worlds, id: &str) -> io::Result<Vec<PathBuf>> {
    Ok(load(worlds, id)?["grants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect())
}

fn push_all(list: &mut Value, items: Vec<Value>) {
    for item in items {
        push(list, item);
    }
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
            apart: false,
            base: None,
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

    #[test]
    fn an_archived_chat_leaves_the_list_and_comes_back() {
        let dir = std::env::temp_dir().join(format!("fennec-os-chat-arch-{}", std::process::id()));
        let folder = dir.join("proj");
        std::fs::create_dir_all(&folder).unwrap();
        let w = worlds(&dir);
        let chat = start(&w, &folder).unwrap();
        let id = chat["id"].as_str().unwrap().to_string();
        assert_eq!(list(&w).unwrap().len(), 1);
        archive(&w, &id, true).unwrap();
        assert!(list(&w).unwrap().is_empty());
        assert_eq!(list_archived(&w).unwrap().len(), 1);
        // The exchange is untouched by archiving.
        assert_eq!(load(&w, &id).unwrap()["archived"], true);
        archive(&w, &id, false).unwrap();
        assert_eq!(list(&w).unwrap().len(), 1);
        assert!(list_archived(&w).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_chat_is_forgotten_only_when_its_turn_is_done() {
        let dir =
            std::env::temp_dir().join(format!("fennec-os-chat-forget-{}", std::process::id()));
        let folder = dir.join("proj");
        std::fs::create_dir_all(&folder).unwrap();
        let w = worlds(&dir);
        let chat = start(&w, &folder).unwrap();
        let id = chat["id"].as_str().unwrap().to_string();
        say(&w, &id, "bonjour").unwrap();
        assert_eq!(load(&w, &id).unwrap()["status"], "thinking");
        let err = forget(&w, &id).unwrap_err().to_string();
        assert!(err.contains("running"), "{err}");
        fail(&w, &id, "no model").unwrap();
        forget(&w, &id).unwrap();
        assert!(load(&w, &id).is_err());
        assert!(list(&w).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
